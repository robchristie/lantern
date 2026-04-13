use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{
    cdp::{CdpError, CdpEvent, CdpWebSocket, TargetInfo},
    redaction::{
        CONSOLE_MESSAGE_LIMIT, RedactionMode, sanitize_console_message, sanitize_title,
        sanitize_url,
    },
};

pub const CONSOLE_SCHEMA_VERSION: u8 = 1;
pub const CONSOLE_MAX_ENTRIES: usize = 20;
const CONSOLE_DRAIN_TIMEOUT: Duration = Duration::from_millis(300);
const CONSOLE_READ_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: ConsolePageSummary,
    pub console: ConsoleSummary,
}

impl ConsoleCommandOutput {
    pub fn success(page: ConsolePageSummary, console: ConsoleSummary) -> Self {
        Self {
            schema_version: CONSOLE_SCHEMA_VERSION,
            command: "console",
            ok: true,
            page,
            console,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleSummary {
    pub message_count: usize,
    pub exception_count: usize,
    pub max_entries: usize,
    pub message_text_limit: usize,
    pub truncated: bool,
    pub entries: Vec<ConsoleEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleEntry {
    pub sequence: usize,
    pub kind: ConsoleEntryKind,
    pub severity: String,
    pub message: String,
    pub source_url_shape: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub argument_count: usize,
    pub stack_frame_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleEntryKind {
    Console,
    Exception,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleReadError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for ConsoleReadError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn read_console_errors(
    target: &TargetInfo,
    mode: RedactionMode,
) -> Result<ConsoleCommandOutput, ConsoleReadError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(ConsoleReadError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    socket.call("Runtime.enable", None)?;
    socket.call("Log.enable", None)?;

    let mut collector = ConsoleCollector::new(mode);
    let deadline = Instant::now() + CONSOLE_DRAIN_TIMEOUT;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(CONSOLE_READ_SLICE);
        let Some(event) = socket.read_event(timeout)? else {
            break;
        };
        collector.push_event(event);
    }

    let page = ConsolePageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    };

    Ok(ConsoleCommandOutput::success(page, collector.finish()))
}

struct ConsoleCollector {
    mode: RedactionMode,
    next_sequence: usize,
    message_count: usize,
    exception_count: usize,
    truncated: bool,
    entries: Vec<ConsoleEntry>,
}

impl ConsoleCollector {
    fn new(mode: RedactionMode) -> Self {
        Self {
            mode,
            next_sequence: 1,
            message_count: 0,
            exception_count: 0,
            truncated: false,
            entries: Vec::new(),
        }
    }

    fn push_event(&mut self, event: CdpEvent) {
        let candidate = match event.method.as_str() {
            "Runtime.consoleAPICalled" => runtime_console_entry(event.params, self.mode),
            "Runtime.exceptionThrown" => runtime_exception_entry(event.params, self.mode),
            "Log.entryAdded" => log_entry(event.params, self.mode),
            _ => None,
        };

        let Some(mut candidate) = candidate else {
            return;
        };

        if self.entries.len() >= CONSOLE_MAX_ENTRIES {
            self.truncated = true;
            return;
        }

        candidate.entry.sequence = self.next_sequence;
        self.next_sequence += 1;
        match candidate.entry.kind {
            ConsoleEntryKind::Console => self.message_count += 1,
            ConsoleEntryKind::Exception => self.exception_count += 1,
        }
        if candidate.message_truncated {
            self.truncated = true;
        }
        self.entries.push(candidate.entry);
    }

    fn finish(self) -> ConsoleSummary {
        ConsoleSummary {
            message_count: self.message_count,
            exception_count: self.exception_count,
            max_entries: CONSOLE_MAX_ENTRIES,
            message_text_limit: CONSOLE_MESSAGE_LIMIT,
            truncated: self.truncated,
            entries: self.entries,
        }
    }
}

struct ConsoleEntryCandidate {
    entry: ConsoleEntry,
    message_truncated: bool,
}

fn runtime_console_entry(
    params: serde_json::Value,
    mode: RedactionMode,
) -> Option<ConsoleEntryCandidate> {
    let event: RuntimeConsoleApiCalled = serde_json::from_value(params).ok()?;
    if !matches!(event.kind.as_str(), "error" | "assert") {
        return None;
    }

    let argument_count = event.args.len();
    let message = event
        .args
        .iter()
        .map(runtime_argument_preview)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let stack = stack_summary(event.stack_trace.as_ref());

    Some(ConsoleEntryCandidate {
        message_truncated: message_was_truncated(&message, mode),
        entry: ConsoleEntry {
            sequence: 0,
            kind: ConsoleEntryKind::Console,
            severity: "error".to_owned(),
            message: sanitize_console_message(&message, mode),
            source_url_shape: stack.url.and_then(|url| sanitize_event_url(&url, mode)),
            line: stack.line,
            column: stack.column,
            argument_count,
            stack_frame_count: stack.frame_count,
        },
    })
}

fn runtime_exception_entry(
    params: serde_json::Value,
    mode: RedactionMode,
) -> Option<ConsoleEntryCandidate> {
    let event: RuntimeExceptionThrown = serde_json::from_value(params).ok()?;
    let details = event.exception_details;
    let stack = stack_summary(details.stack_trace.as_ref());
    let exception_preview = details.exception.as_ref().map(runtime_argument_preview);
    let message = exception_preview
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&details.text);

    Some(ConsoleEntryCandidate {
        message_truncated: message_was_truncated(message, mode),
        entry: ConsoleEntry {
            sequence: 0,
            kind: ConsoleEntryKind::Exception,
            severity: "error".to_owned(),
            message: sanitize_console_message(message, mode),
            source_url_shape: details
                .url
                .or(stack.url)
                .and_then(|url| sanitize_event_url(&url, mode)),
            line: details.line_number.or(stack.line),
            column: details.column_number.or(stack.column),
            argument_count: 0,
            stack_frame_count: stack.frame_count,
        },
    })
}

fn log_entry(params: serde_json::Value, mode: RedactionMode) -> Option<ConsoleEntryCandidate> {
    let event: LogEntryAdded = serde_json::from_value(params).ok()?;
    if event.entry.level != "error" {
        return None;
    }

    let stack = stack_summary(event.entry.stack_trace.as_ref());

    Some(ConsoleEntryCandidate {
        message_truncated: message_was_truncated(&event.entry.text, mode),
        entry: ConsoleEntry {
            sequence: 0,
            kind: ConsoleEntryKind::Console,
            severity: event.entry.level,
            message: sanitize_console_message(&event.entry.text, mode),
            source_url_shape: event
                .entry
                .url
                .or(stack.url)
                .and_then(|url| sanitize_event_url(&url, mode)),
            line: event.entry.line_number.or(stack.line),
            column: stack.column,
            argument_count: 0,
            stack_frame_count: stack.frame_count,
        },
    })
}

fn runtime_argument_preview(argument: &RuntimeRemoteObject) -> String {
    if let Some(value) = argument.value.as_ref() {
        return runtime_value_preview(value);
    }

    if let Some(description) = argument.description.as_ref() {
        return description.clone();
    }

    match argument.subtype.as_deref() {
        Some("array") => "array".to_owned(),
        Some("error") => "error".to_owned(),
        Some("node") => "node".to_owned(),
        Some(subtype) => subtype.to_owned(),
        None => argument.kind.clone(),
    }
}

fn runtime_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(values) => format!("array({})", values.len()),
        serde_json::Value::Object(_) => "object".to_owned(),
    }
}

fn stack_summary(stack: Option<&CdpStackTrace>) -> StackSummary {
    let Some(stack) = stack else {
        return StackSummary::default();
    };

    let first = stack.call_frames.first();
    StackSummary {
        url: first
            .map(|frame| frame.url.clone())
            .filter(|url| !url.is_empty()),
        line: first.map(|frame| frame.line_number),
        column: first.map(|frame| frame.column_number),
        frame_count: stack.call_frames.len(),
    }
}

fn sanitize_event_url(url: &str, mode: RedactionMode) -> Option<String> {
    sanitize_url(url, mode)
}

fn message_was_truncated(message: &str, mode: RedactionMode) -> bool {
    mode == RedactionMode::Redacted && message.chars().count() > CONSOLE_MESSAGE_LIMIT
}

#[derive(Debug, Default)]
struct StackSummary {
    url: Option<String>,
    line: Option<i64>,
    column: Option<i64>,
    frame_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeConsoleApiCalled {
    #[serde(rename = "type")]
    kind: String,
    args: Vec<RuntimeRemoteObject>,
    stack_trace: Option<CdpStackTrace>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeExceptionThrown {
    exception_details: RuntimeExceptionDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeExceptionDetails {
    text: String,
    url: Option<String>,
    line_number: Option<i64>,
    column_number: Option<i64>,
    stack_trace: Option<CdpStackTrace>,
    exception: Option<RuntimeRemoteObject>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRemoteObject {
    #[serde(rename = "type")]
    kind: String,
    subtype: Option<String>,
    value: Option<serde_json::Value>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogEntryAdded {
    entry: CdpLogEntry,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpLogEntry {
    level: String,
    text: String,
    url: Option<String>,
    line_number: Option<i64>,
    stack_trace: Option<CdpStackTrace>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpStackTrace {
    call_frames: Vec<CdpCallFrame>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpCallFrame {
    url: String,
    line_number: i64,
    column_number: i64,
}
