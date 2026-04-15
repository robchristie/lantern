use std::{
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    cdp::{CdpError, CdpEvent, CdpWebSocket, TargetInfo},
    console::{CONSOLE_COLLECTION_GAP_REASON, ConsoleCollector, ConsoleSummary},
    network::{NETWORK_COLLECTION_GAP_REASON, NetworkCollector, NetworkSummary},
    redaction::{RedactionMode, sanitize_title, sanitize_url},
    wait::{ReadyState, WaitConditionName, WaitObservedState, WaitSummary},
};

pub const FLOW_SCHEMA_VERSION: u8 = 1;
pub const FLOW_COLLECTION_GAP_NONE: &str = "collection_started_before_observed_flow";
const FLOW_POLL_INTERVAL: Duration = Duration::from_millis(100);
const FLOW_EVENT_READ_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowOptions {
    pub open_url: Option<String>,
    pub timeout: Duration,
    pub quiet_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: FlowPageSummary,
    pub flow: FlowSummary,
    pub console: ConsoleSummary,
    pub network: NetworkSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
    pub ready_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlowSummary {
    pub single_attachment: bool,
    pub opened_url: bool,
    pub observation_started_before_navigation: bool,
    pub wait: WaitSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowError {
    TargetWebSocketMissing,
    NavigationFailed(String),
    Cdp(CdpError),
}

impl From<CdpError> for FlowError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn run_observation_flow(
    target: &TargetInfo,
    options: FlowOptions,
    mode: RedactionMode,
) -> Result<FlowCommandOutput, FlowError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(FlowError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    socket.call("Runtime.enable", None)?;
    socket.call("Log.enable", None)?;
    socket.call("Network.enable", None)?;
    socket.call("Page.enable", None)?;

    let mut console = ConsoleCollector::new(mode);
    let mut network = NetworkCollector::new(mode);
    drain_available_events(&mut socket, &mut console, &mut network)?;

    let opened_url = options.open_url.is_some();
    if let Some(url) = options.open_url.as_deref() {
        let result = socket
            .call("Page.navigate", Some(json!({ "url": url })))
            .map_err(|error| match error {
                CdpError::Command { message, .. } => FlowError::NavigationFailed(message),
                error => FlowError::Cdp(error),
            })?;
        if let Some(error_text) = result.get("errorText").and_then(Value::as_str) {
            return Err(FlowError::NavigationFailed(error_text.to_owned()));
        }
    }

    let started = Instant::now();
    let wait = if let Some(quiet_ms) = options.quiet_ms {
        wait_for_flow_quiet(
            &mut socket,
            &mut console,
            &mut network,
            started,
            options.timeout,
            quiet_ms,
        )?
    } else {
        wait_for_flow_ready(
            &mut socket,
            &mut console,
            &mut network,
            started,
            options.timeout,
        )?
    };

    drain_available_events(&mut socket, &mut console, &mut network)?;
    let page = flow_page_summary(target, &mut socket, mode)?;
    let collection_gap = !opened_url;
    let collection_gap_reason = if collection_gap {
        CONSOLE_COLLECTION_GAP_REASON
    } else {
        FLOW_COLLECTION_GAP_NONE
    };
    let network_gap_reason = if collection_gap {
        NETWORK_COLLECTION_GAP_REASON
    } else {
        FLOW_COLLECTION_GAP_NONE
    };

    Ok(FlowCommandOutput {
        schema_version: FLOW_SCHEMA_VERSION,
        command: "flow",
        ok: true,
        page,
        flow: FlowSummary {
            single_attachment: true,
            opened_url,
            observation_started_before_navigation: opened_url,
            wait,
        },
        console: console.finish_with_collection_gap(collection_gap, collection_gap_reason),
        network: network.finish_with_collection_gap(collection_gap, network_gap_reason),
    })
}

fn wait_for_flow_ready(
    socket: &mut CdpWebSocket,
    console: &mut ConsoleCollector,
    network: &mut NetworkCollector,
    started: Instant,
    timeout: Duration,
) -> Result<WaitSummary, FlowError> {
    let deadline = started + timeout;

    loop {
        drain_available_events(socket, console, network)?;
        let ready_state = document_ready_state(socket)?;
        if ready_state
            .as_deref()
            .is_some_and(|state| ReadyState::Complete.matches_public(state))
        {
            return Ok(WaitSummary {
                condition: WaitConditionName::Ready,
                matched: true,
                timed_out: false,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed: WaitObservedState::Ready { ready_state },
            });
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(WaitSummary {
                condition: WaitConditionName::Ready,
                matched: false,
                timed_out: true,
                elapsed_ms: duration_millis(started.elapsed()),
                timeout_ms: duration_millis(timeout),
                observed: WaitObservedState::Ready { ready_state },
            });
        }

        thread::sleep((deadline - now).min(FLOW_POLL_INTERVAL));
    }
}

fn wait_for_flow_quiet(
    socket: &mut CdpWebSocket,
    console: &mut ConsoleCollector,
    network: &mut NetworkCollector,
    started: Instant,
    timeout: Duration,
    quiet_ms: u64,
) -> Result<WaitSummary, FlowError> {
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
            .min(FLOW_EVENT_READ_SLICE);
        if let Some(event) = socket.read_event(read_timeout)? {
            if counts_for_flow_quiet(&event) {
                event_count += 1;
                last_event = Some(event.method.clone());
                quiet_deadline = Instant::now() + quiet;
            }
            push_observation_event(event, console, network);
        }
    }
}

fn drain_available_events(
    socket: &mut CdpWebSocket,
    console: &mut ConsoleCollector,
    network: &mut NetworkCollector,
) -> Result<(), FlowError> {
    while let Some(event) = socket.read_event(Duration::from_millis(1))? {
        push_observation_event(event, console, network);
    }
    Ok(())
}

fn push_observation_event(
    event: CdpEvent,
    console: &mut ConsoleCollector,
    network: &mut NetworkCollector,
) {
    console.push_event(event.clone());
    network.push_event(event);
}

fn counts_for_flow_quiet(event: &CdpEvent) -> bool {
    event.method.starts_with("Runtime.")
        || event.method.starts_with("Log.")
        || event.method.starts_with("Network.")
        || event.method.starts_with("Page.loadEvent")
        || event.method.starts_with("Page.domContentEvent")
}

fn flow_page_summary(
    target: &TargetInfo,
    socket: &mut CdpWebSocket,
    mode: RedactionMode,
) -> Result<FlowPageSummary, FlowError> {
    let title = runtime_evaluate_string(socket, "document.title")?
        .or_else(|| target.title.clone())
        .map(|title| sanitize_title(&title, mode));
    let ready_state = document_ready_state(socket)?;
    let url_shape = navigation_history_current_url(socket, mode)?
        .or_else(|| target.url.as_ref().and_then(|url| sanitize_url(url, mode)));

    Ok(FlowPageSummary {
        target_id: target.id.clone(),
        title,
        url_shape,
        ready_state,
    })
}

fn document_ready_state(socket: &mut CdpWebSocket) -> Result<Option<String>, FlowError> {
    runtime_evaluate_string(socket, "document.readyState")
}

fn runtime_evaluate_string(
    socket: &mut CdpWebSocket,
    expression: &str,
) -> Result<Option<String>, FlowError> {
    let result = socket.call(
        "Runtime.evaluate",
        Some(json!({
            "expression": expression,
            "returnByValue": true,
            "timeout": 1000
        })),
    )?;
    let value = result
        .get("result")
        .and_then(|result| result.get("value"))
        .cloned();
    Ok(match value {
        Some(Value::String(value)) => Some(value),
        Some(Value::Null) | None => None,
        Some(value) => Some(value.to_string()),
    })
}

fn navigation_history_current_url(
    socket: &mut CdpWebSocket,
    mode: RedactionMode,
) -> Result<Option<String>, FlowError> {
    let result = socket.call("Page.getNavigationHistory", None)?;
    let Some(entries) = result.get("entries").and_then(Value::as_array) else {
        return Ok(None);
    };
    let current_index = result
        .get("currentIndex")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let Some(entry) = entries.get(current_index as usize) else {
        return Ok(None);
    };
    let Some(url) = entry.get("url").and_then(Value::as_str) else {
        return Ok(None);
    };
    if url == "about:blank" {
        return Ok(Some(url.to_owned()));
    }
    Ok(sanitize_url(url, mode))
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

trait ReadyStateMatchPublic {
    fn matches_public(self, observed: &str) -> bool;
}

impl ReadyStateMatchPublic for ReadyState {
    fn matches_public(self, observed: &str) -> bool {
        match self {
            Self::Loading => matches!(observed, "loading" | "interactive" | "complete"),
            Self::Interactive => matches!(observed, "interactive" | "complete"),
            Self::Complete => observed == "complete",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_wait_timeout_carries_timeout() {
        let summary = WaitSummary {
            condition: WaitConditionName::Ready,
            matched: false,
            timed_out: true,
            elapsed_ms: 10,
            timeout_ms: 123,
            observed: WaitObservedState::Ready { ready_state: None },
        };

        assert_eq!(summary.timeout_ms, 123);
    }
}
