use std::{
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    cdp::{
        CdpError, CdpEvent, CdpWebSocket, MAX_DRAIN_EVENTS, MAX_DRAIN_TIME, OperationDeadline,
        TargetInfo,
    },
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
    let budget = OperationDeadline::from(options.timeout);
    run_observation_flow_until(target, options, mode, budget)
}

pub fn run_observation_flow_until(
    target: &TargetInfo,
    options: FlowOptions,
    mode: RedactionMode,
    budget: OperationDeadline,
) -> Result<FlowCommandOutput, FlowError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(FlowError::TargetWebSocketMissing)?;

    let started = budget.started;
    let mut socket = CdpWebSocket::connect_until(web_socket_debugger_url, budget.end())?;
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

    let mut wait = if let Some(quiet_ms) = options.quiet_ms {
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
    let page = if Instant::now() < budget.end() {
        flow_page_summary(target, &mut socket, mode)?
    } else {
        socket.mark_collection_deadline();
        FlowPageSummary {
            target_id: target.id.clone(),
            title: target.title.as_ref().map(|t| sanitize_title(t, mode)),
            url_shape: target.url.as_ref().and_then(|u| sanitize_url(u, mode)),
            ready_state: None,
        }
    };
    // Finalisation calls may themselves queue events. The queue is bounded.
    while let Some(event) = socket.pop_pending_event() {
        if Instant::now() >= budget.end() {
            socket.mark_collection_deadline();
            break;
        }
        push_observation_event(event, &mut console, &mut network);
    }
    let loss = socket.evidence_loss();
    console.record_evidence_loss(loss.clone());
    network.record_evidence_loss(loss);
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

    let console = console.finish_with_collection_gap(collection_gap, collection_gap_reason);
    let network = network.finish_with_collection_gap(collection_gap, network_gap_reason);
    if wait.condition == WaitConditionName::Quiet
        && (console.evidence_loss.incomplete() || network.evidence_loss.incomplete())
    {
        wait.matched = false;
    }

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
        console,
        network,
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
        let ready_state = if Instant::now() < deadline {
            document_ready_state(socket)?
        } else {
            None
        };
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

pub(crate) fn drain_available_events(
    socket: &mut CdpWebSocket,
    console: &mut ConsoleCollector,
    network: &mut NetworkCollector,
) -> Result<(), FlowError> {
    let until = Instant::now() + MAX_DRAIN_TIME;
    for _ in 0..MAX_DRAIN_EVENTS {
        if socket.deadline().is_some_and(|d| Instant::now() >= d) {
            socket.mark_collection_deadline();
            return Ok(());
        }
        if Instant::now() >= until {
            socket.mark_drain_limit();
            return Ok(());
        }
        let Some(event) = socket.read_event(Duration::from_millis(1))? else {
            return Ok(());
        };
        push_observation_event(event, console, network);
    }
    socket.mark_drain_limit();
    Ok(())
}

pub(crate) fn push_observation_event(
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

    fn flow_fixture(withhold: Option<&'static str>) -> (TargetInfo, std::thread::JoinHandle<()>) {
        use std::{io::ErrorKind, net::TcpListener};
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_nodelay(true).unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            socket.get_mut().set_nonblocking(true).unwrap();
            loop {
                match socket.read() {
                    Ok(Message::Text(text)) => {
                        let command: Value = serde_json::from_str(&text).unwrap();
                        let method = command["method"].as_str().unwrap();
                        let title = command["params"]["expression"] == "document.title";
                        if withhold != Some(method) && !(withhold == Some("title") && title) {
                            let result = if method == "Runtime.evaluate" {
                                json!({"result":{"value":"complete"}})
                            } else {
                                json!({})
                            };
                            if socket
                                .send(Message::Text(
                                    json!({"id":command["id"],"result":result})
                                        .to_string()
                                        .into(),
                                ))
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
                if socket
                    .send(Message::Text(
                        json!({"method":"Runtime.executionContextCreated","params":{}})
                            .to_string()
                            .into(),
                    ))
                    .is_err()
                {
                    break;
                }
                std::thread::sleep(Duration::from_micros(100));
            }
        });
        (
            TargetInfo {
                id: "fixture".into(),
                kind: "page".into(),
                title: None,
                url: Some("about:blank".into()),
                attached: None,
                browser_context_id: None,
                web_socket_debugger_url: Some(format!("ws://{address}/page")),
            },
            handle,
        )
    }

    fn flow_with_collector_events(batches: Vec<(&'static str, Vec<Value>)>) -> FlowCommandOutput {
        use std::net::TcpListener;
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_nodelay(true).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            while let Ok(Message::Text(text)) = socket.read() {
                let command: Value = serde_json::from_str(&text).unwrap();
                let method = command["method"].as_str().unwrap();
                for (_, events) in batches.iter().filter(|(when, _)| *when == method) {
                    for event in events {
                        socket
                            .send(Message::Text(event.to_string().into()))
                            .unwrap();
                    }
                }
                let result = if method == "Runtime.evaluate" {
                    json!({"result":{"value":"complete"}})
                } else {
                    json!({})
                };
                socket
                    .send(Message::Text(
                        json!({"id":command["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .unwrap();
            }
        });
        let target = TargetInfo {
            id: "collector-fixture".into(),
            kind: "page".into(),
            title: None,
            url: Some("about:blank".into()),
            attached: None,
            browser_context_id: None,
            web_socket_debugger_url: Some(format!("ws://{address}/page")),
        };
        let output = run_observation_flow(
            &target,
            FlowOptions {
                open_url: Some("http://example.test".into()),
                timeout: Duration::from_secs(1),
                quiet_ms: Some(30),
            },
            RedactionMode::Redacted,
        )
        .unwrap();
        handle.join().unwrap();
        output
    }

    #[test]
    fn quiet_flow_rejects_collector_payload_loss_without_transport_loss() {
        let output = flow_with_collector_events(vec![(
            "Page.navigate",
            vec![json!({
                "method":"Runtime.exceptionThrown", "params":{"exceptionDetails":{"text":"x".repeat(270_000)}}
            })],
        )]);
        assert!(output.ok, "preserve completed-command exit semantics");
        assert!(!output.flow.wait.timed_out);
        assert!(!output.flow.wait.matched);
        for loss in [&output.console.evidence_loss, &output.network.evidence_loss] {
            assert_eq!(
                loss.dropped_events, 1,
                "only the collector rejects this payload"
            );
            assert!(!loss.drain_limit_reached);
            assert!(!loss.collection_deadline_reached);
        }
        assert!(!output.console.observed_clean);
        assert!(!output.network.observed_clean);
    }

    #[test]
    fn quiet_flow_rejects_correlation_loss_from_finalisation_without_queue_overflow() {
        let requests = |start, end| {
            (start..end).map(|id| json!({
            "method":"Network.requestWillBeSent", "params":{"requestId":format!("request-{id}"), "request":{"method":"GET", "url":"http://example.test"}}
        })).collect::<Vec<_>>()
        };
        // Each response buffers fewer than 1024 small events; only the network
        // collector's retained correlation map fills. The final batch arrives
        // during finalisation, after the provisional quiet match.
        let output = flow_with_collector_events(vec![
            ("Page.navigate", requests(0, 512)),
            ("Page.getNavigationHistory", requests(512, 1025)),
        ]);
        assert!(output.ok);
        assert!(!output.flow.wait.timed_out);
        assert!(!output.flow.wait.matched);
        assert!(
            output.console.evidence_loss.is_complete(),
            "no transport or console loss"
        );
        assert_eq!(output.network.evidence_loss.dropped_events, 1);
        assert!(!output.network.evidence_loss.drain_limit_reached);
        assert!(!output.network.evidence_loss.collection_deadline_reached);
        assert!(!output.network.observed_clean);
    }

    #[test]
    fn sustained_flow_events_finish_at_deadline_without_claiming_clean() {
        let (target, handle) = flow_fixture(None);
        let started = Instant::now();
        let output = run_observation_flow(
            &target,
            FlowOptions {
                open_url: Some("http://example.test".into()),
                timeout: Duration::from_millis(150),
                quiet_ms: Some(30),
            },
            RedactionMode::Redacted,
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!output.flow.wait.matched);
        assert!(output.flow.wait.timed_out);
        assert!(!output.console.observed_clean);
        assert!(!output.network.observed_clean);
        assert!(output.console.evidence_loss.incomplete());
        handle.join().unwrap();
    }

    #[test]
    fn flow_setup_and_finalisation_share_the_operation_deadline() {
        for withheld in ["Runtime.enable", "title"] {
            let (target, handle) = flow_fixture(Some(withheld));
            let started = Instant::now();
            assert!(
                run_observation_flow(
                    &target,
                    FlowOptions {
                        open_url: None,
                        timeout: Duration::from_millis(100),
                        quiet_ms: None
                    },
                    RedactionMode::Redacted
                )
                .is_err()
            );
            assert!(started.elapsed() < Duration::from_millis(600));
            handle.join().unwrap();
        }
    }

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
