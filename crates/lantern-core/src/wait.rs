use std::{
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    cdp::{CdpError, CdpEvent, CdpWebSocket, TargetInfo},
    redaction::{RedactionMode, sanitize_dom_text, sanitize_title, sanitize_url},
};

pub const WAIT_SCHEMA_VERSION: u8 = 1;
pub const WAIT_MAX_TIMEOUT_MS: u64 = 30_000;
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WAIT_EVENT_READ_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: WaitPageSummary,
    pub wait: WaitSummary,
}

impl WaitCommandOutput {
    pub fn success(page: WaitPageSummary, wait: WaitSummary) -> Self {
        Self {
            schema_version: WAIT_SCHEMA_VERSION,
            command: "wait",
            ok: true,
            page,
            wait,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaitSummary {
    pub condition: WaitConditionName,
    pub matched: bool,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub observed: WaitObservedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitConditionName {
    Ready,
    Url,
    Selector,
    Text,
    Quiet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum WaitObservedState {
    Ready {
        ready_state: Option<String>,
    },
    Url {
        expected_url_shape: String,
        current_url_shape: Option<String>,
    },
    Selector {
        selector: String,
        present: bool,
    },
    Text {
        selector: String,
        expected_text: String,
        current_text: Option<String>,
    },
    Quiet {
        quiet_ms: u64,
        event_count: usize,
        last_event: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitCondition {
    Ready { state: ReadyState },
    Url { expected_url_shape: String },
    Selector { selector: String },
    Text { selector: String, text: String },
    Quiet { quiet_ms: u64 },
}

impl WaitCondition {
    pub fn name(&self) -> WaitConditionName {
        match self {
            Self::Ready { .. } => WaitConditionName::Ready,
            Self::Url { .. } => WaitConditionName::Url,
            Self::Selector { .. } => WaitConditionName::Selector,
            Self::Text { .. } => WaitConditionName::Text,
            Self::Quiet { .. } => WaitConditionName::Quiet,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyState {
    Loading,
    Interactive,
    Complete,
}

impl ReadyState {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "loading" => Some(Self::Loading),
            "interactive" => Some(Self::Interactive),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }

    fn matches(self, observed: &str) -> bool {
        match self {
            Self::Loading => matches!(observed, "loading" | "interactive" | "complete"),
            Self::Interactive => matches!(observed, "interactive" | "complete"),
            Self::Complete => observed == "complete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for WaitError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn wait_for_condition(
    target: &TargetInfo,
    condition: WaitCondition,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<WaitCommandOutput, WaitError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(WaitError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let wait = match condition {
        WaitCondition::Quiet { quiet_ms } => {
            wait_for_quiet(&mut socket, started, timeout, quiet_ms)?
        }
        WaitCondition::Ready { state } => {
            wait_by_polling(started, timeout, WaitConditionName::Ready, || {
                let ready_state = document_ready_state(&mut socket)?;
                let matched = ready_state
                    .as_deref()
                    .is_some_and(|observed| state.matches(observed));
                Ok((matched, WaitObservedState::Ready { ready_state }))
            })?
        }
        WaitCondition::Url { expected_url_shape } => {
            wait_by_polling(started, timeout, WaitConditionName::Url, || {
                let current_url_shape = navigation_history_current_url(&mut socket, mode)?;
                let matched = current_url_shape.as_deref() == Some(expected_url_shape.as_str());
                Ok((
                    matched,
                    WaitObservedState::Url {
                        expected_url_shape: expected_url_shape.clone(),
                        current_url_shape,
                    },
                ))
            })?
        }
        WaitCondition::Selector { selector } => {
            wait_by_polling(started, timeout, WaitConditionName::Selector, || {
                let present = selector_present(&mut socket, &selector)?;
                Ok((
                    present,
                    WaitObservedState::Selector {
                        selector: selector.clone(),
                        present,
                    },
                ))
            })?
        }
        WaitCondition::Text { selector, text } => {
            wait_by_polling(started, timeout, WaitConditionName::Text, || {
                let current_text = selector_text(&mut socket, &selector, mode)?;
                let matched = current_text
                    .as_deref()
                    .is_some_and(|value| value.contains(&text));
                Ok((
                    matched,
                    WaitObservedState::Text {
                        selector: selector.clone(),
                        expected_text: sanitize_dom_text(&text, mode),
                        current_text,
                    },
                ))
            })?
        }
    };

    let page = WaitPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target
            .url
            .as_ref()
            .and_then(|url| sanitize_wait_url(url, mode)),
    };

    Ok(WaitCommandOutput::success(page, wait))
}

fn wait_by_polling(
    started: Instant,
    timeout: Duration,
    condition: WaitConditionName,
    mut observe: impl FnMut() -> Result<(bool, WaitObservedState), WaitError>,
) -> Result<WaitSummary, WaitError> {
    let deadline = started + timeout;
    let mut last_observed = None;

    loop {
        if Instant::now() >= deadline {
            if let Some(observed) = last_observed {
                return Ok(WaitSummary {
                    condition,
                    matched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed,
                });
            }
        }

        let (matched, observed) = observe()?;
        if matched {
            return Ok(WaitSummary {
                condition,
                matched: true,
                timed_out: false,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed,
            });
        }
        last_observed = Some(observed);

        let now = Instant::now();
        if now >= deadline {
            let observed = last_observed.expect("polling observes before timeout check");
            return Ok(WaitSummary {
                condition,
                matched: false,
                timed_out: true,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed,
            });
        }

        thread::sleep((deadline - now).min(WAIT_POLL_INTERVAL));
    }
}

fn wait_for_quiet(
    socket: &mut CdpWebSocket,
    started: Instant,
    timeout: Duration,
    quiet_ms: u64,
) -> Result<WaitSummary, WaitError> {
    socket.call("Runtime.enable", None)?;
    socket.call("Log.enable", None)?;
    socket.call("Page.enable", None)?;
    let _ = socket.call(
        "Page.setLifecycleEventsEnabled",
        Some(json!({ "enabled": true })),
    );

    let deadline = started + timeout;
    let quiet = Duration::from_millis(quiet_ms);
    let mut quiet_deadline = Instant::now() + quiet;
    let mut event_count = 0;
    let mut last_event = None;

    loop {
        let now = Instant::now();
        if now >= quiet_deadline {
            return Ok(WaitSummary {
                condition: WaitConditionName::Quiet,
                matched: true,
                timed_out: false,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed: WaitObservedState::Quiet {
                    quiet_ms,
                    event_count,
                    last_event,
                },
            });
        }
        if now >= deadline {
            return Ok(WaitSummary {
                condition: WaitConditionName::Quiet,
                matched: false,
                timed_out: true,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed: WaitObservedState::Quiet {
                    quiet_ms,
                    event_count,
                    last_event,
                },
            });
        }

        let read_timeout = (deadline - now)
            .min(quiet_deadline - now)
            .min(WAIT_EVENT_READ_SLICE);
        if let Some(event) = socket.read_event(read_timeout)? {
            if counts_for_quiet(&event) {
                event_count += 1;
                last_event = Some(event.method);
                quiet_deadline = Instant::now() + quiet;
            }
        }
    }
}

fn counts_for_quiet(event: &CdpEvent) -> bool {
    event.method.starts_with("Runtime.")
        || event.method.starts_with("Log.")
        || event.method.starts_with("Page.lifecycleEvent")
        || event.method.starts_with("Page.loadEvent")
        || event.method.starts_with("Page.domContentEvent")
}

fn document_ready_state(socket: &mut CdpWebSocket) -> Result<Option<String>, WaitError> {
    runtime_evaluate_string(socket, "document.readyState")
}

fn navigation_history_current_url(
    socket: &mut CdpWebSocket,
    mode: RedactionMode,
) -> Result<Option<String>, WaitError> {
    let result = socket.call("Page.getNavigationHistory", None)?;
    let response: CdpNavigationHistoryResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Page.getNavigationHistory response",
            source: source.to_string(),
        })?;

    let Some(entry) = response.entries.get(response.current_index as usize) else {
        return Ok(None);
    };

    Ok(sanitize_wait_url(&entry.url, mode))
}

fn selector_present(socket: &mut CdpWebSocket, selector: &str) -> Result<bool, WaitError> {
    let selector = js_string_literal(selector)?;
    let expression = format!("Boolean(document.querySelector({selector}))");
    Ok(runtime_evaluate_bool(socket, &expression)?.unwrap_or(false))
}

fn selector_text(
    socket: &mut CdpWebSocket,
    selector: &str,
    mode: RedactionMode,
) -> Result<Option<String>, WaitError> {
    let selector = js_string_literal(selector)?;
    let expression = format!(
        "(function() {{ const el = document.querySelector({selector}); return el ? (el.textContent || '') : null; }})()"
    );
    Ok(runtime_evaluate_string(socket, &expression)?.map(|text| sanitize_dom_text(&text, mode)))
}

fn runtime_evaluate_string(
    socket: &mut CdpWebSocket,
    expression: &str,
) -> Result<Option<String>, WaitError> {
    match runtime_evaluate_value(socket, expression)? {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(value) => Ok(Some(value.to_string())),
    }
}

fn runtime_evaluate_bool(
    socket: &mut CdpWebSocket,
    expression: &str,
) -> Result<Option<bool>, WaitError> {
    match runtime_evaluate_value(socket, expression)? {
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Ok(None),
    }
}

fn runtime_evaluate_value(
    socket: &mut CdpWebSocket,
    expression: &str,
) -> Result<Option<Value>, WaitError> {
    let result = socket.call(
        "Runtime.evaluate",
        Some(json!({
            "expression": expression,
            "returnByValue": true,
            "timeout": 1000
        })),
    )?;
    let response: CdpRuntimeEvaluateResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Runtime.evaluate response",
            source: source.to_string(),
        })?;

    Ok(response.result.value)
}

fn js_string_literal(value: &str) -> Result<String, WaitError> {
    serde_json::to_string(value).map_err(|source| {
        WaitError::Cdp(CdpError::ResponseInvalid {
            context: "failed to serialize wait selector",
            source: source.to_string(),
        })
    })
}

fn sanitize_wait_url(value: &str, mode: RedactionMode) -> Option<String> {
    if value == "about:blank" {
        return Some(value.to_owned());
    }

    sanitize_url(value, mode)
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpNavigationHistoryResponse {
    current_index: i64,
    entries: Vec<CdpNavigationHistoryEntry>,
}

#[derive(Debug, Deserialize)]
struct CdpNavigationHistoryEntry {
    url: String,
}

#[derive(Debug, Deserialize)]
struct CdpRuntimeEvaluateResponse {
    result: CdpRuntimeRemoteObject,
}

#[derive(Debug, Deserialize)]
struct CdpRuntimeRemoteObject {
    value: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_state_complete_satisfies_interactive_and_complete() {
        assert!(ReadyState::Interactive.matches("complete"));
        assert!(ReadyState::Complete.matches("complete"));
        assert!(!ReadyState::Complete.matches("interactive"));
    }

    #[test]
    fn parses_supported_ready_states() {
        assert_eq!(ReadyState::parse("loading"), Some(ReadyState::Loading));
        assert_eq!(
            ReadyState::parse("interactive"),
            Some(ReadyState::Interactive)
        );
        assert_eq!(ReadyState::parse("complete"), Some(ReadyState::Complete));
        assert_eq!(ReadyState::parse("idle"), None);
    }

    #[test]
    fn polling_wait_summary_carries_timeout_without_outer_patch() {
        let timeout = Duration::from_millis(1234);
        let summary = wait_by_polling(Instant::now(), timeout, WaitConditionName::Selector, || {
            Ok((
                true,
                WaitObservedState::Selector {
                    selector: "main".to_owned(),
                    present: true,
                },
            ))
        })
        .expect("wait summary");

        assert_eq!(summary.timeout_ms, 1234);
        assert!(summary.matched);
        assert!(!summary.timed_out);
    }

    #[test]
    fn polling_timeout_summary_carries_timeout_without_outer_patch() {
        let timeout = Duration::from_millis(7);
        let summary = wait_by_polling(
            Instant::now() - Duration::from_millis(10),
            timeout,
            WaitConditionName::Selector,
            || {
                Ok((
                    false,
                    WaitObservedState::Selector {
                        selector: "main".to_owned(),
                        present: false,
                    },
                ))
            },
        )
        .expect("wait summary");

        assert_eq!(summary.timeout_ms, 7);
        assert!(!summary.matched);
        assert!(summary.timed_out);
    }
}
