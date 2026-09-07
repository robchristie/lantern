//! One observed click and an explicit postcondition; never replay uncertain input.
use crate::{
    cdp::{CdpError, CdpWebSocket, OperationDeadline, TargetInfo},
    console::{ConsoleCollector, ConsoleSummary},
    flow::{FlowError, drain_available_events, push_observation_event},
    interaction::{ActionRequest, DispatchState, InteractionSummary, interact_on_socket},
    network::{NetworkCollector, NetworkSummary},
    redaction::{RedactionMode, sanitize_dom_text, sanitize_url},
    screenshot::{CapturedScreenshot, ScreenshotSummary, capture_on_socket},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    thread,
    time::{Duration, Instant},
};

pub const ACTION_FLOW_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub enum Postcondition {
    Selector { selector: String },
    Text { selector: String, text: String },
    Url { url: String },
}

#[derive(Debug, Serialize)]
pub struct PostconditionSummary {
    pub matched_before_action: Option<bool>,
    pub observed_before_action: Value,
    pub matched: bool,
    pub timed_out: bool,
    pub observed: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct CaptureSummary {
    pub requested: bool,
    pub status: &'static str,
    pub screenshot: Option<ScreenshotSummary>,
    pub error: Option<&'static str>,
    pub correlation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Passed,
    Failed,
    Incomplete,
}

#[derive(Debug, Serialize)]
pub struct ActionFlowOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub target_id: String,
    pub single_attachment: bool,
    pub observation_started_before_action: bool,
    pub interaction: InteractionSummary,
    pub postcondition: PostconditionSummary,
    pub console: ConsoleSummary,
    pub network: NetworkSummary,
    pub capture: CaptureSummary,
    pub verdict: Verdict,
    pub error: Option<&'static str>,
    pub elapsed_ms: u64,
}

/// `persist` runs only for an explicitly requested capture. Any persistence
/// error is retained alongside the already dispatched action. The caller must
/// use a local regular-file sink; it must not introduce an unbounded stream.
pub fn run_action_flow_until(
    target: &TargetInfo,
    selector: &str,
    condition: Postcondition,
    mode: RedactionMode,
    budget: OperationDeadline,
    capture_requested: bool,
    mut persist: impl FnMut(CapturedScreenshot) -> Result<ScreenshotSummary, &'static str>,
) -> Result<ActionFlowOutput, FlowError> {
    let url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(FlowError::TargetWebSocketMissing)?;
    let mut socket = CdpWebSocket::connect_until(url, budget.end())?;
    for method in [
        "Runtime.enable",
        "Log.enable",
        "Network.enable",
        "Page.enable",
    ] {
        socket.call(method, None)?;
    }
    let mut console = ConsoleCollector::new(mode);
    let mut network = NetworkCollector::new(mode);
    drain_available_events(&mut socket, &mut console, &mut network)?;
    // A malformed or unavailable baseline must fail before any input.
    let (before, observed) = condition.probe(&mut socket, mode)?;
    let mut postcondition = PostconditionSummary {
        matched_before_action: Some(before),
        observed_before_action: observed,
        matched: false,
        timed_out: false,
        observed: None,
    };
    let interaction = interact_on_socket(&mut socket, selector, ActionRequest::default(), budget);
    // Best-effort input release may have used the separate cleanup allowance.
    socket.restore_deadline(budget.end());
    let mut error = None;
    if interaction.dispatch_state != DispatchState::NotDispatched {
        loop {
            if Instant::now() >= budget.end() {
                postcondition.timed_out = true;
                break;
            }
            if drain_available_events(&mut socket, &mut console, &mut network).is_err() {
                error = Some("observation_failed");
                break;
            }
            match condition.probe(&mut socket, mode) {
                Ok((matched, observed)) => {
                    postcondition.matched = matched;
                    postcondition.observed = Some(observed);
                    if matched {
                        break;
                    }
                }
                Err(_) => {
                    error = Some("postcondition_observation_failed");
                    break;
                }
            }
            thread::sleep(
                budget
                    .end()
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(50)),
            );
        }
    }
    let mut capture = CaptureSummary {
        requested: capture_requested,
        status: if capture_requested {
            "failed"
        } else {
            "not_requested"
        },
        screenshot: None,
        error: None,
        correlation: "sequenced_after_postcondition_not_atomic_frame",
    };
    if capture_requested {
        match capture_on_socket(target, mode, None, &mut socket) {
            Ok(_) if Instant::now() >= budget.end() => {
                capture.error = Some("screenshot_deadline_reached");
            }
            Ok(image) => match persist(image) {
                Ok(summary) => {
                    capture.status = "captured";
                    capture.screenshot = Some(summary);
                }
                Err(code) => capture.error = Some(code),
            },
            Err(_) => capture.error = Some("screenshot_capture_failed"),
        }
    }
    if drain_available_events(&mut socket, &mut console, &mut network).is_err() {
        error.get_or_insert("observation_failed");
    }
    // No reconnect or metadata call after this final bounded queue drain.
    while let Some(event) = socket.pop_pending_event() {
        if Instant::now() >= budget.end() {
            socket.mark_collection_deadline();
            break;
        }
        push_observation_event(event, &mut console, &mut network);
    }
    if Instant::now() >= budget.end() {
        socket.mark_collection_deadline();
        if !postcondition.matched && interaction.dispatch_state != DispatchState::NotDispatched {
            postcondition.timed_out = true;
        }
    }
    console.record_evidence_loss(socket.evidence_loss());
    network.record_evidence_loss(socket.evidence_loss());
    let console =
        console.finish_with_collection_gap(false, "collection_started_before_observed_action");
    let network =
        network.finish_with_collection_gap(false, "collection_started_before_observed_action");
    let incomplete = before
        || error.is_some()
        || console.evidence_loss.incomplete()
        || network.evidence_loss.incomplete()
        || interaction.dispatch_state == DispatchState::Uncertain
        || capture.error.is_some();
    let failed = console.message_count > 0
        || console.exception_count > 0
        || network.failed_count > 0
        || network.http_error_count > 0
        || interaction.dispatch_state == DispatchState::NotDispatched;
    let verdict = if incomplete {
        Verdict::Incomplete
    } else if failed {
        Verdict::Failed
    } else if postcondition.matched {
        Verdict::Passed
    } else {
        Verdict::Failed
    };
    if before {
        error.get_or_insert("postcondition_already_matched");
    }
    Ok(ActionFlowOutput {
        schema_version: ACTION_FLOW_SCHEMA_VERSION,
        command: "action-flow",
        ok: true,
        target_id: target.id.clone(),
        single_attachment: true,
        observation_started_before_action: true,
        interaction,
        postcondition,
        console,
        network,
        capture,
        verdict,
        error,
        elapsed_ms: budget
            .started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64,
    })
}

impl Postcondition {
    fn probe(
        &self,
        socket: &mut CdpWebSocket,
        mode: RedactionMode,
    ) -> Result<(bool, Value), CdpError> {
        // Match in the page against original text/URL, redact only reported
        // metadata. Distinct secrets must never become equal via redaction.
        let expression = match self {
            Self::Selector { selector } => format!(
                "(() => {{ const nodes = document.querySelectorAll({}); return {{matched:nodes.length > 0, count:nodes.length}}; }})()",
                json!(selector)
            ),
            Self::Text { selector, text } => format!(
                "(() => {{ const nodes = document.querySelectorAll({}); const text = nodes.length === 1 ? nodes[0].textContent : null; return {{matched:text !== null && text.includes({}), count:nodes.length, text:text === null ? null : text.slice(0,2000)}}; }})()",
                json!(selector),
                json!(text)
            ),
            Self::Url { url } => format!(
                "({{matched:location.href === {}, url:location.href}})",
                json!(url)
            ),
        };
        let result = socket.call(
            "Runtime.evaluate",
            Some(json!({"expression":expression,"returnByValue":true,"timeout":1000})),
        )?;
        if result.get("exceptionDetails").is_some() {
            return Err(CdpError::ResponseInvalid {
                context: "postcondition evaluation failed",
                source: "runtime exception".into(),
            });
        }
        let value = &result["result"]["value"];
        let matched = value["matched"]
            .as_bool()
            .ok_or_else(|| CdpError::ResponseInvalid {
                context: "postcondition response invalid",
                source: "missing match boolean".into(),
            })?;
        let observed = match self {
            Self::Selector { selector } => {
                json!({"kind":"selector","selector":selector,"count":value["count"]})
            }
            Self::Text { selector, text } => {
                json!({"kind":"text","selector":selector,"expected_text":sanitize_dom_text(text,mode),"current_text":value["text"].as_str().map(|s|sanitize_dom_text(s,mode)),"count":value["count"]})
            }
            Self::Url { url } => {
                json!({"kind":"url","expected_url":sanitize_url(url,mode),"current_url":value["url"].as_str().and_then(|s|sanitize_url(s,mode))})
            }
        };
        Ok((matched, observed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tungstenite::Message;

    fn fixture(scenario: &'static str, capture: bool) -> (ActionFlowOutput, Vec<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_nodelay(true).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let mut inputs = Vec::new();
            let mut probes = 0;
            while let Ok(Message::Text(text)) = ws.read() {
                let c: Value = serde_json::from_str(&text).unwrap();
                let method = c["method"].as_str().unwrap();
                let mut result = json!({});
                if method == "Runtime.evaluate" {
                    if c["params"].get("objectGroup").is_some() {
                        result = json!({"result":{"objectId":"target"}});
                    } else {
                        probes += 1;
                        if scenario == "probe_stall" && probes > 1 {
                            continue;
                        }
                        if scenario == "probe_error" && probes > 1 {
                            result = json!({"exceptionDetails":{}});
                        } else {
                            let matched =
                                scenario == "preexisting" || (probes > 1 && scenario != "timeout");
                            result = json!({"result":{"value":{"matched":matched,"count":1,"text":"saved"}}});
                        }
                    }
                } else if method == "Runtime.callFunctionOn" {
                    result = if scenario == "blocked" {
                        json!({"result":{"value":{"error":"ambiguous_selector"}}})
                    } else {
                        json!({"result":{"value":{"node_name":"BUTTON","rect":[0,0,50,50],"point":{"x":25,"y":25}}}})
                    };
                } else if method.starts_with("Input.") {
                    let kind = c["params"]["type"].as_str().unwrap().to_owned();
                    inputs.push(kind.clone());
                    if scenario == "uncertain" && kind == "mousePressed" {
                        continue;
                    }
                    if kind == "mouseReleased" && matches!(scenario, "failure" | "loss") {
                        let events = if scenario == "loss" { 1100 } else { 1 };
                        for n in 0..events {
                            let event = json!({"method":"Runtime.exceptionThrown","params":{"exceptionDetails":{"text":format!("failure {n}"),"lineNumber":1,"columnNumber":1}}});
                            if ws.send(Message::Text(event.to_string().into())).is_err() {
                                return inputs;
                            }
                        }
                    }
                } else if method == "Page.captureScreenshot" {
                    if scenario == "capture_stall" {
                        continue;
                    }
                    result = json!({"data":"aW1hZ2U="});
                }
                if ws
                    .send(Message::Text(
                        json!({"id":c["id"],"result":result}).to_string().into(),
                    ))
                    .is_err()
                {
                    break;
                }
                if scenario == "final_error" && probes > 1 {
                    break;
                }
            }
            inputs
        });
        let target = TargetInfo {
            id: "fixture".into(),
            kind: "page".into(),
            title: None,
            url: None,
            attached: None,
            browser_context_id: None,
            web_socket_debugger_url: Some(format!("ws://{address}/page")),
        };
        let budget = OperationDeadline::from(Duration::from_millis(350));
        let output = run_action_flow_until(
            &target,
            "#target",
            Postcondition::Text {
                selector: "#state".into(),
                text: "saved".into(),
            },
            RedactionMode::Redacted,
            budget,
            capture,
            |image| {
                if scenario == "write_failure" {
                    return Err("screenshot_write_failed");
                }
                Ok(ScreenshotSummary {
                    format: image.format,
                    width: image.width,
                    height: image.height,
                    region: None,
                    byte_count: image.bytes.len(),
                    path: "test.png".into(),
                    overwritten: false,
                    redaction_caveat: crate::screenshot::SCREENSHOT_REDACTION_CAVEAT,
                })
            },
        )
        .unwrap();
        assert!(
            budget.started.elapsed() < Duration::from_millis(800),
            "operation overran budget and cleanup allowance"
        );
        (output, server.join().unwrap())
    }

    #[test]
    fn explicit_transition_passes_and_preexisting_match_does_not() {
        for (scenario, verdict) in [
            ("saved", Verdict::Passed),
            ("preexisting", Verdict::Incomplete),
        ] {
            let (o, input) = fixture(scenario, false);
            assert_eq!(o.verdict, verdict);
            assert_eq!(o.interaction.dispatch_state, DispatchState::Acknowledged);
            assert!(o.postcondition.matched);
            assert_eq!(input, ["mouseMoved", "mousePressed", "mouseReleased"]);
        }
    }

    #[test]
    fn failures_and_loss_survive_a_matching_postcondition() {
        for scenario in ["failure", "loss"] {
            let (o, _) = fixture(scenario, false);
            assert!(o.postcondition.matched);
            assert!(o.console.exception_count > 0);
            assert_ne!(o.verdict, Verdict::Passed);
            if scenario == "loss" {
                assert!(o.console.evidence_loss.incomplete());
            }
        }
    }

    #[test]
    fn later_observation_and_capture_failures_retain_dispatch() {
        for scenario in [
            "timeout",
            "probe_stall",
            "probe_error",
            "final_error",
            "capture_stall",
            "write_failure",
        ] {
            let (o, input) = fixture(
                scenario,
                scenario.contains("capture") || scenario == "write_failure",
            );
            assert_eq!(o.interaction.dispatch_state, DispatchState::Acknowledged);
            assert_eq!(o.verdict, Verdict::Incomplete);
            assert_eq!(input.iter().filter(|s| *s == "mousePressed").count(), 1);
            if scenario == "write_failure" {
                assert!(o.postcondition.matched);
                assert_eq!(o.capture.error, Some("screenshot_write_failed"));
            }
        }
    }

    #[test]
    fn uncertain_input_is_never_replayed_and_original_deadline_is_restored() {
        let (o, input) = fixture("uncertain", true);
        assert_eq!(o.interaction.dispatch_state, DispatchState::Uncertain);
        assert_eq!(o.verdict, Verdict::Incomplete);
        assert_eq!(o.capture.status, "failed");
        assert!(o.postcondition.timed_out);
        assert_eq!(input, ["mouseMoved", "mousePressed", "mouseReleased"]);
    }

    #[test]
    fn blocked_action_emits_no_input() {
        let (o, input) = fixture("blocked", false);
        assert_eq!(o.verdict, Verdict::Failed);
        assert_eq!(o.interaction.dispatch_state, DispatchState::NotDispatched);
        assert!(input.is_empty());
    }

    #[test]
    fn setup_and_baseline_deadlines_prevent_input() {
        for withheld in ["Runtime.enable", "Runtime.evaluate"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut ws = tungstenite::accept(stream).unwrap();
                while let Ok(Message::Text(text)) = ws.read() {
                    let c: Value = serde_json::from_str(&text).unwrap();
                    let method = c["method"].as_str().unwrap();
                    assert!(!method.starts_with("Input."));
                    assert_ne!(method, "Runtime.callFunctionOn");
                    if method == withheld {
                        continue;
                    }
                    if ws
                        .send(Message::Text(
                            json!({"id":c["id"],"result":{}}).to_string().into(),
                        ))
                        .is_err()
                    {
                        break;
                    }
                }
            });
            let target = TargetInfo {
                id: "fixture".into(),
                kind: "page".into(),
                title: None,
                url: None,
                attached: None,
                browser_context_id: None,
                web_socket_debugger_url: Some(format!("ws://{address}/page")),
            };
            let budget = OperationDeadline::from(Duration::from_millis(80));
            let result = run_action_flow_until(
                &target,
                "#target",
                Postcondition::Selector {
                    selector: "#saved".into(),
                },
                RedactionMode::Redacted,
                budget,
                false,
                |_| panic!("unexpected capture"),
            );
            assert!(result.is_err());
            assert!(budget.started.elapsed() < Duration::from_millis(400));
            server.join().unwrap();
        }
    }
}
