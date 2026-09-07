use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{
    cdp::{
        CdpError, CdpEvent, CdpWebSocket, DEFAULT_OPERATION_TIMEOUT, EvidenceLoss,
        MAX_DRAIN_EVENTS, TargetInfo,
    },
    redaction::{
        CONSOLE_MESSAGE_LIMIT, RedactionMode, sanitize_console_message, sanitize_title,
        sanitize_url,
    },
};

pub const CONSOLE_SCHEMA_VERSION: u8 = 1;
pub const CONSOLE_MAX_ENTRIES: usize = 20;
pub const CONSOLE_COLLECTION_GAP_REASON: &str = "events_before_runtime_log_enable_not_observed";
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
    pub observed_clean: bool,
    #[serde(skip_serializing_if = "crate::cdp::EvidenceLoss::is_complete")]
    pub evidence_loss: EvidenceLoss,
    pub collection_gap: bool,
    pub collection_gap_reason: &'static str,
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

    let operation_deadline = Instant::now() + DEFAULT_OPERATION_TIMEOUT;
    let mut socket = CdpWebSocket::connect_until(web_socket_debugger_url, operation_deadline)?;
    socket.call("Runtime.enable", None)?;
    socket.call("Log.enable", None)?;

    let mut collector = ConsoleCollector::new(mode);
    let deadline = Instant::now() + CONSOLE_DRAIN_TIMEOUT;

    let mut drained = 0;
    while Instant::now() < deadline && drained < MAX_DRAIN_EVENTS {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(CONSOLE_READ_SLICE);
        let Some(event) = socket.read_event(timeout)? else {
            break;
        };
        drained += 1;
        collector.push_event(event);
    }

    if drained == MAX_DRAIN_EVENTS {
        socket.mark_drain_limit();
    }
    if Instant::now() >= deadline || Instant::now() >= operation_deadline {
        socket.mark_collection_deadline();
    }
    collector.record_evidence_loss(socket.evidence_loss());

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

pub(crate) struct ConsoleCollector {
    mode: RedactionMode,
    next_sequence: usize,
    message_count: usize,
    exception_count: usize,
    truncated: bool,
    evidence_loss: EvidenceLoss,
    entries: Vec<ConsoleEntry>,
}

impl ConsoleCollector {
    pub(crate) fn new(mode: RedactionMode) -> Self {
        Self {
            mode,
            next_sequence: 1,
            message_count: 0,
            exception_count: 0,
            truncated: false,
            evidence_loss: EvidenceLoss::default(),
            entries: Vec::new(),
        }
    }

    pub(crate) fn record_evidence_loss(&mut self, loss: EvidenceLoss) {
        self.evidence_loss.dropped_events = self
            .evidence_loss
            .dropped_events
            .saturating_add(loss.dropped_events);
        self.evidence_loss.dropped_event_bytes = self
            .evidence_loss
            .dropped_event_bytes
            .saturating_add(loss.dropped_event_bytes);
        self.evidence_loss.drain_limit_reached |= loss.drain_limit_reached;
        self.evidence_loss.collection_deadline_reached |= loss.collection_deadline_reached;
    }

    pub(crate) fn push_event(&mut self, event: CdpEvent) {
        // Bound retained derived fields even with redaction disabled.
        let bytes = event.params.to_string().len() + event.method.len();
        if bytes > 256 * 1024 {
            self.evidence_loss.dropped_events = self.evidence_loss.dropped_events.saturating_add(1);
            self.evidence_loss.dropped_event_bytes = self
                .evidence_loss
                .dropped_event_bytes
                .saturating_add(bytes as u64);
            return;
        }
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

    pub(crate) fn finish(self) -> ConsoleSummary {
        self.finish_with_collection_gap(true, CONSOLE_COLLECTION_GAP_REASON)
    }

    pub(crate) fn finish_with_collection_gap(
        self,
        collection_gap: bool,
        collection_gap_reason: &'static str,
    ) -> ConsoleSummary {
        let observed_clean =
            self.entries.is_empty() && !self.truncated && !self.evidence_loss.incomplete();
        ConsoleSummary {
            message_count: self.message_count,
            exception_count: self.exception_count,
            max_entries: CONSOLE_MAX_ENTRIES,
            message_text_limit: CONSOLE_MESSAGE_LIMIT,
            truncated: self.truncated || self.evidence_loss.incomplete(),
            evidence_loss: self.evidence_loss,
            observed_clean,
            collection_gap,
            collection_gap_reason,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redaction::RedactionMode;

    #[test]
    fn lost_transport_or_oversized_evidence_cannot_be_observed_clean() {
        let mut collector = ConsoleCollector::new(RedactionMode::Redacted);
        collector.record_evidence_loss(EvidenceLoss {
            dropped_events: 1,
            dropped_event_bytes: 100,
            ..Default::default()
        });
        let summary = collector.finish();
        assert!(!summary.observed_clean);
        assert!(summary.truncated);
        assert_eq!(summary.evidence_loss.dropped_events, 1);
        let mut collector = ConsoleCollector::new(RedactionMode::Unredacted);
        collector.push_event(CdpEvent {
            method: "Runtime.exceptionThrown".into(),
            params: serde_json::json!({"large":"x".repeat(256 * 1024)}),
        });
        let summary = collector.finish();
        assert!(!summary.observed_clean);
        assert!(summary.evidence_loss.dropped_event_bytes > 256 * 1024);
    }

    #[test]
    fn clean_window_still_reports_collection_gap() {
        let summary = ConsoleCollector::new(RedactionMode::Redacted).finish();

        assert_eq!(summary.message_count, 0);
        assert_eq!(summary.exception_count, 0);
        assert!(summary.observed_clean);
        assert!(summary.collection_gap);
        assert_eq!(
            summary.collection_gap_reason,
            "events_before_runtime_log_enable_not_observed"
        );
        assert!(summary.entries.is_empty());
    }
}
