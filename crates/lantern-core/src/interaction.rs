use std::{
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    cdp::{CdpError, CdpWebSocket, OperationDeadline, TargetInfo},
    redaction::{RedactionMode, sanitize_title, sanitize_url},
};

pub const INTERACTION_SCHEMA_VERSION: u8 = 1;
pub const INTERACTION_MAX_TIMEOUT_MS: u64 = 30_000;
pub const INTERACTION_MAX_DURATION_MS: u64 = 30_000;
const INTERACTION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_STEP_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InteractionCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: InteractionPageSummary,
    pub interaction: InteractionSummary,
}

impl InteractionCommandOutput {
    fn completed(
        command: &'static str,
        page: InteractionPageSummary,
        interaction: InteractionSummary,
    ) -> Self {
        Self {
            schema_version: INTERACTION_SCHEMA_VERSION,
            command,
            ok: !interaction
                .immediate_error
                .is_some_and(|code| code.starts_with("cdp_") || code == "actionability_incomplete"),
            page,
            interaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InteractionPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InteractionSummary {
    pub action: InteractionAction,
    pub selector: String,
    pub dispatched: bool,
    pub dispatch_state: DispatchState,
    pub application_outcome: &'static str,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub observed: InteractionObservedState,
    pub immediate_error: Option<&'static str>,
    #[serde(skip_serializing_if = "crate::cdp::EvidenceLoss::is_complete")]
    pub evidence_loss: crate::cdp::EvidenceLoss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchState {
    NotDispatched,
    Acknowledged,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionAction {
    Click,
    Type,
    Key,
    Hover,
    Wheel,
    Drag,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InteractionObservedState {
    pub node_name: Option<String>,
    pub clickable_point: Option<InteractionPoint>,
    pub inserted_text_length: Option<usize>,
    pub key: Option<String>,
    pub key_event_count: Option<usize>,
    pub pointer_start: Option<InteractionPoint>,
    pub pointer_end: Option<InteractionPoint>,
    pub delta_x: Option<f64>,
    pub delta_y: Option<f64>,
    pub duration_ms: Option<u64>,
    pub input_event_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InteractionPoint {
    pub x: f64,
    pub y: f64,
}

impl InteractionObservedState {
    fn click(node_name: Option<String>, clickable_point: Option<InteractionPoint>) -> Self {
        Self {
            node_name,
            clickable_point,
            inserted_text_length: None,
            key: None,
            key_event_count: None,
            pointer_start: None,
            pointer_end: None,
            delta_x: None,
            delta_y: None,
            duration_ms: None,
            input_event_count: None,
        }
    }

    fn type_text(node_name: Option<String>, inserted_text_length: Option<usize>) -> Self {
        Self {
            node_name,
            clickable_point: None,
            inserted_text_length,
            key: None,
            key_event_count: None,
            pointer_start: None,
            pointer_end: None,
            delta_x: None,
            delta_y: None,
            duration_ms: None,
            input_event_count: None,
        }
    }

    fn key(node_name: Option<String>, key: &str, key_event_count: usize) -> Self {
        Self {
            node_name,
            clickable_point: None,
            inserted_text_length: None,
            key: Some(key.to_owned()),
            key_event_count: Some(key_event_count),
            pointer_start: None,
            pointer_end: None,
            delta_x: None,
            delta_y: None,
            duration_ms: None,
            input_event_count: None,
        }
    }

    fn pointer(
        node_name: Option<String>,
        point: Option<InteractionPoint>,
        delta_x: Option<f64>,
        delta_y: Option<f64>,
        duration_ms: Option<u64>,
        input_event_count: Option<usize>,
    ) -> Self {
        Self {
            node_name,
            clickable_point: point,
            inserted_text_length: None,
            key: None,
            key_event_count: None,
            pointer_start: point,
            pointer_end: None,
            delta_x,
            delta_y,
            duration_ms,
            input_event_count,
        }
    }

    fn drag(
        node_name: Option<String>,
        start: Option<InteractionPoint>,
        end: Option<InteractionPoint>,
        delta_x: f64,
        delta_y: f64,
        duration_ms: u64,
        input_event_count: usize,
    ) -> Self {
        Self {
            node_name,
            clickable_point: start,
            inserted_text_length: None,
            key: None,
            key_event_count: None,
            pointer_start: start,
            pointer_end: end,
            delta_x: Some(delta_x),
            delta_y: Some(delta_y),
            duration_ms: Some(duration_ms),
            input_event_count: Some(input_event_count),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for InteractionError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn click_element(
    target: &TargetInfo,
    selector: &str,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Click,
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

pub fn type_text(
    target: &TargetInfo,
    selector: &str,
    text: &str,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Type,
            text: Some(text),
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

pub fn press_key(
    target: &TargetInfo,
    selector: &str,
    key: &str,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Key,
            key: Some(key),
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

pub fn hover_element(
    target: &TargetInfo,
    selector: &str,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Hover,
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

pub fn wheel_element(
    target: &TargetInfo,
    selector: &str,
    delta_x: f64,
    delta_y: f64,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Wheel,
            delta_x,
            delta_y,
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

pub fn drag_element(
    target: &TargetInfo,
    selector: &str,
    delta_x: f64,
    delta_y: f64,
    duration: Duration,
    timeout: impl Into<OperationDeadline>,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    run_interaction(
        target,
        selector,
        ActionRequest {
            action: InteractionAction::Drag,
            delta_x,
            delta_y,
            duration,
            ..ActionRequest::default()
        },
        timeout.into(),
        mode,
    )
}

/// Internal same-socket request, reusable by observed actions without reconnecting.
#[derive(Clone, Copy)]
pub(crate) struct ActionRequest<'a> {
    pub action: InteractionAction,
    pub text: Option<&'a str>,
    pub key: Option<&'a str>,
    pub delta_x: f64,
    pub delta_y: f64,
    pub duration: Duration,
}

impl Default for ActionRequest<'_> {
    fn default() -> Self {
        Self {
            action: InteractionAction::Click,
            text: None,
            key: None,
            delta_x: 0.,
            delta_y: 0.,
            duration: Duration::ZERO,
        }
    }
}

impl ActionRequest<'_> {
    fn command(self) -> &'static str {
        match self.action {
            InteractionAction::Click => "click",
            InteractionAction::Type => "type",
            InteractionAction::Key => "key",
            InteractionAction::Hover => "hover",
            InteractionAction::Wheel => "wheel",
            InteractionAction::Drag => "drag",
        }
    }
    fn observed(
        self,
        node: Option<String>,
        point: Option<InteractionPoint>,
        count: usize,
    ) -> InteractionObservedState {
        match self.action {
            InteractionAction::Click => InteractionObservedState::click(node, point),
            InteractionAction::Type => InteractionObservedState::type_text(
                node,
                (count > 0).then(|| self.text.unwrap_or("").chars().count()),
            ),
            InteractionAction::Key => {
                InteractionObservedState::key(node, self.key.unwrap_or(""), count)
            }
            InteractionAction::Hover => {
                InteractionObservedState::pointer(node, point, None, None, None, Some(count))
            }
            InteractionAction::Wheel => InteractionObservedState::pointer(
                node,
                point,
                Some(self.delta_x),
                Some(self.delta_y),
                None,
                Some(count),
            ),
            InteractionAction::Drag => InteractionObservedState::drag(
                node,
                point,
                point.map(|p| self.end(p)),
                self.delta_x,
                self.delta_y,
                duration_millis(self.duration),
                count,
            ),
        }
    }
    fn end(self, start: InteractionPoint) -> InteractionPoint {
        InteractionPoint {
            x: (start.x + self.delta_x).round(),
            y: (start.y + self.delta_y).round(),
        }
    }
}

fn run_interaction(
    target: &TargetInfo,
    selector: &str,
    request: ActionRequest<'_>,
    budget: OperationDeadline,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;
    let mut socket = CdpWebSocket::connect_until(url, budget.end())?;
    let summary = interact_on_socket(&mut socket, selector, request, budget);
    Ok(InteractionCommandOutput::completed(
        request.command(),
        page_summary(target, mode),
        summary,
    ))
}

pub(crate) fn interact_on_socket(
    socket: &mut CdpWebSocket,
    selector: &str,
    request: ActionRequest<'_>,
    budget: OperationDeadline,
) -> InteractionSummary {
    let mut summary = InteractionSummary {
        action: request.action,
        selector: selector.to_owned(),
        dispatched: false,
        dispatch_state: DispatchState::NotDispatched,
        application_outcome: "unverified",
        timed_out: false,
        elapsed_ms: 0,
        timeout_ms: duration_millis(budget.timeout),
        observed: request.observed(None, None, 0),
        immediate_error: None,
        evidence_loss: socket.evidence_loss(),
    };
    // A retained activeElement alone does not establish document focus or
    // focus-event delivery in a background/headless page. Activate the exact
    // selected page before text/key preparation, within the original budget.
    if matches!(
        request.action,
        InteractionAction::Type | InteractionAction::Key
    ) {
        if let Err(error) = socket.call("Page.bringToFront", None) {
            summary.immediate_error = Some(error_code(&error.into()));
            summary.timed_out = Instant::now() >= budget.end();
            summary.elapsed_ms = duration_millis(budget.started.elapsed());
            summary.evidence_loss = socket.evidence_loss();
            return summary;
        }
    }
    let mut last_blocker = None;
    loop {
        if Instant::now() >= budget.end() {
            summary.timed_out = true;
            summary.immediate_error = Some(last_blocker.unwrap_or("actionability_incomplete"));
            break;
        }
        match prepare_target(socket, selector, request, &mut last_blocker) {
            Ok(Some(probe)) => {
                summary.observed = request.observed(probe.node_name.clone(), probe.point, 0);
                // Never retry this operation once its input sequence has begun.
                let mut progress = InputProgress::default();
                let result = dispatch_action(socket, request, probe.point, &mut progress);
                summary.observed =
                    request.observed(probe.node_name, probe.point, progress.acknowledged);
                summary.dispatched = result.is_ok();
                summary.dispatch_state = if result.is_ok() {
                    DispatchState::Acknowledged
                } else if progress.possible {
                    DispatchState::Uncertain
                } else {
                    DispatchState::NotDispatched
                };
                if let Err(error) = result {
                    summary.immediate_error = Some(error_code(&error));
                }
                summary.timed_out = Instant::now() >= budget.end();
                break;
            }
            Ok(None) => {
                // Ambiguity and malformed selectors are deterministic, not a request
                // to wait for one arbitrary match to disappear.
                if matches!(
                    last_blocker,
                    Some("ambiguous_selector" | "selector_invalid")
                ) {
                    summary.immediate_error = last_blocker;
                    break;
                }
            }
            Err(error) => {
                summary.timed_out = Instant::now() >= budget.end();
                summary.immediate_error = Some(if summary.timed_out {
                    last_blocker.unwrap_or_else(|| error_code(&error))
                } else {
                    error_code(&error)
                });
                break;
            }
        }
        sleep_until_next_poll(budget.end());
    }
    summary.elapsed_ms = duration_millis(budget.started.elapsed());
    summary.evidence_loss = socket.evidence_loss();
    summary
}

fn error_code(error: &InteractionError) -> &'static str {
    match error {
        InteractionError::Cdp(CdpError::CommandUncertain { .. }) => "cdp_command_uncertain",
        InteractionError::Cdp(CdpError::Command { .. }) => "cdp_command_failed",
        InteractionError::Cdp(CdpError::ResponseInvalid { .. }) => "cdp_response_invalid",
        _ => "cdp_transport_failed",
    }
}

#[derive(Debug, Deserialize)]
struct ActionabilityProbe {
    error: Option<String>,
    node_name: Option<String>,
    rect: Option<[f64; 4]>,
    point: Option<InteractionPoint>,
}

fn blocker(code: &str) -> Result<&'static str, InteractionError> {
    Ok(match code {
        "ambiguous_selector" => "ambiguous_selector",
        "selector_invalid" => "selector_invalid",
        "selector_not_found" => "selector_not_found",
        "element_disabled" => "element_disabled",
        "element_not_editable" => "element_not_editable",
        "element_not_focused" => "element_not_focused",
        "element_not_visible" => "element_not_visible",
        "element_occluded" => "element_occluded",
        "element_unstable" => "element_unstable",
        _ => {
            return Err(CdpError::ResponseInvalid {
                context: "unexpected interaction blocker",
                source: "unknown blocker code".to_owned(),
            }
            .into());
        }
    })
}

fn prepare_target(
    socket: &mut CdpWebSocket,
    selector: &str,
    request: ActionRequest<'_>,
    last_blocker: &mut Option<&'static str>,
) -> Result<Option<ActionabilityProbe>, InteractionError> {
    // Encode the selector as JSON data, never executable source supplied by the caller.
    let expression = format!(
        "(() => {{ try {{ const nodes = document.querySelectorAll({}); return nodes.length === 1 ? nodes[0] : nodes.length ? 'ambiguous_selector' : 'selector_not_found'; }} catch (_) {{ return 'selector_invalid'; }} }})()",
        json!(selector)
    );
    let resolved = socket.call(
        "Runtime.evaluate",
        Some(json!({"expression": expression, "objectGroup": "lantern-interaction"})),
    )?;
    if let Some(code) = resolved["result"]["value"].as_str() {
        *last_blocker = Some(blocker(code)?);
        return Ok(None);
    }
    let object_id =
        resolved["result"]["objectId"]
            .as_str()
            .ok_or_else(|| CdpError::ResponseInvalid {
                context: "failed to resolve interaction target",
                source: "missing target object".to_owned(),
            })?;
    let result = (|| {
        let first = probe_target(socket, object_id, selector, request, None)?;
        if let Some(error) = first.error {
            *last_blocker = Some(blocker(&error)?);
            return Ok(None);
        }
        // One valid sample does not establish stability or instability. Preserve
        // only a blocker actually returned by a probe; an unfinished second
        // sample is an incomplete evaluation, not an observed moving target.
        let deadline = socket
            .deadline()
            .expect("interaction has an operation deadline");
        sleep_until_next_poll(deadline);
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let second = probe_target(socket, object_id, selector, request, first.rect)?;
        if let Some(error) = &second.error {
            *last_blocker = Some(blocker(error)?);
            return Ok(None);
        }
        Ok(Some(second))
    })();
    // Handles belong only to this operation. Cleanup stays inside the shared
    // deadline and cannot mask the last observed blocker or extend the budget.
    if socket.deadline().is_some_and(|d| Instant::now() < d) {
        let _ = socket.call(
            "Runtime.releaseObjectGroup",
            Some(json!({"objectGroup": "lantern-interaction"})),
        );
    }
    result
}

fn probe_target(
    socket: &mut CdpWebSocket,
    object_id: &str,
    selector: &str,
    request: ActionRequest<'_>,
    previous: Option<[f64; 4]>,
) -> Result<ActionabilityProbe, InteractionError> {
    let result = socket.call(
        "Runtime.callFunctionOn",
        Some(json!({
            "objectId": object_id, "functionDeclaration": include_str!("actionability.js"),
            "arguments": [{"value": selector}, {"value": request.command()}, {"value": previous}],
            "returnByValue": true,
        })),
    )?;
    if result.get("exceptionDetails").is_some() {
        return Err(CdpError::ResponseInvalid {
            context: "interaction actionability probe failed",
            source: "page script exception".to_owned(),
        }
        .into());
    }
    let probe: ActionabilityProbe = serde_json::from_value(result["result"]["value"].clone())
        .map_err(|_| CdpError::ResponseInvalid {
            context: "failed to parse interaction actionability probe",
            source: "invalid bounded probe result".to_owned(),
        })?;
    if probe.error.is_none()
        && (probe.rect.is_none()
            || (request.action != InteractionAction::Key && probe.point.is_none()))
    {
        return Err(CdpError::ResponseInvalid {
            context: "incomplete interaction geometry",
            source: "missing geometry".to_owned(),
        }
        .into());
    }
    Ok(probe)
}

#[derive(Default)]
struct InputProgress {
    acknowledged: usize,
    possible: bool,
}

fn input_call(
    socket: &mut CdpWebSocket,
    progress: &mut InputProgress,
    method: &str,
    params: serde_json::Value,
) -> Result<(), InteractionError> {
    match socket.call(method, Some(params)) {
        Ok(_) => {
            progress.acknowledged += 1;
            progress.possible = true;
            Ok(())
        }
        Err(error) => {
            if matches!(error, CdpError::CommandUncertain { .. }) {
                progress.possible = true;
            }
            Err(error.into())
        }
    }
}

fn dispatch_action(
    socket: &mut CdpWebSocket,
    request: ActionRequest<'_>,
    point: Option<InteractionPoint>,
    progress: &mut InputProgress,
) -> Result<(), InteractionError> {
    match request.action {
        InteractionAction::Click => dispatch_click(socket, point.expect("checked point"), progress),
        InteractionAction::Type => input_call(
            socket,
            progress,
            "Input.insertText",
            json!({"text": request.text.unwrap_or("")}),
        ),
        InteractionAction::Key => dispatch_key(socket, request.key.unwrap_or(""), progress),
        InteractionAction::Hover => dispatch_hover(socket, point.expect("checked point"), progress),
        InteractionAction::Wheel => dispatch_wheel(
            socket,
            point.expect("checked point"),
            request.delta_x,
            request.delta_y,
            progress,
        ),
        InteractionAction::Drag => dispatch_drag(
            socket,
            point.expect("checked point"),
            request.end(point.expect("checked point")),
            request.duration,
            progress,
        )
        .map(|_| ()),
    }
}

fn dispatch_click(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
    progress: &mut InputProgress,
) -> Result<(), InteractionError> {
    input_call(
        socket,
        progress,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseMoved",
            "x": point.x,
            "y": point.y,
            "button": "none"
        }),
    )?;
    let result = (|| {
        input_call(
            socket,
            progress,
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed", "x": point.x, "y": point.y,
                "button": "left", "clickCount": 1
            }),
        )?;
        input_call(
            socket,
            progress,
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased", "x": point.x, "y": point.y,
                "button": "left", "clickCount": 1
            }),
        )
    })();
    if let Err(error) = result {
        socket.begin_cleanup();
        let _ = dispatch_drag_release(socket, point);
        return Err(error);
    }

    Ok(())
}

fn dispatch_hover(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
    progress: &mut InputProgress,
) -> Result<(), InteractionError> {
    input_call(
        socket,
        progress,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseMoved",
            "x": point.x,
            "y": point.y,
            "button": "none"
        }),
    )?;

    Ok(())
}

fn dispatch_wheel(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
    delta_x: f64,
    delta_y: f64,
    progress: &mut InputProgress,
) -> Result<(), InteractionError> {
    input_call(
        socket,
        progress,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseWheel",
            "x": point.x,
            "y": point.y,
            "deltaX": delta_x,
            "deltaY": delta_y
        }),
    )?;

    Ok(())
}

fn dispatch_drag(
    socket: &mut CdpWebSocket,
    start: InteractionPoint,
    end: InteractionPoint,
    duration: Duration,
    progress: &mut InputProgress,
) -> Result<usize, InteractionError> {
    dispatch_hover(socket, start, progress)?;
    if let Err(error) = input_call(
        socket,
        progress,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mousePressed",
            "x": start.x,
            "y": start.y,
            "button": "left",
            "buttons": 1,
            "clickCount": 1
        }),
    ) {
        socket.begin_cleanup();
        let _ = dispatch_drag_release(socket, start);
        return Err(error.into());
    }

    let steps = drag_step_count(duration);
    let drag_started = Instant::now();
    let mut event_count = 2;
    let mut last_accepted = start;
    for step in 1..=steps {
        if !duration.is_zero() {
            let scheduled = drag_started + drag_step_elapsed(duration, step, steps);
            sleep_until(
                socket
                    .deadline()
                    .map_or(scheduled, |deadline| deadline.min(scheduled)),
            );
        }
        let fraction = step as f64 / steps as f64;
        let point = InteractionPoint {
            x: (start.x + ((end.x - start.x) * fraction)).round(),
            y: (start.y + ((end.y - start.y) * fraction)).round(),
        };
        if let Err(error) = input_call(
            socket,
            progress,
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "x": point.x,
                "y": point.y,
                "button": "left",
                "buttons": 1
            }),
        ) {
            // A rejected move must not leave the shared browser holding the
            // button down. Attempt one release with a separate bounded cleanup budget,
            // preserving the original movement error if cleanup also fails.
            socket.begin_cleanup();
            let _ = dispatch_drag_release(socket, last_accepted);
            return Err(error.into());
        }
        last_accepted = point;
        event_count += 1;
    }

    if let Err(error) = dispatch_drag_release(socket, end) {
        socket.begin_cleanup();
        let _ = dispatch_drag_release(socket, end);
        return Err(error);
    }
    progress.acknowledged += 1;
    event_count += 1;

    Ok(event_count)
}

fn dispatch_drag_release(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
) -> Result<(), InteractionError> {
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseReleased",
            "x": point.x,
            "y": point.y,
            "button": "left",
            "buttons": 0,
            "clickCount": 1
        })),
    )?;
    Ok(())
}

fn dispatch_key(
    socket: &mut CdpWebSocket,
    key: &str,
    progress: &mut InputProgress,
) -> Result<(), InteractionError> {
    let result = (|| {
        input_call(
            socket,
            progress,
            "Input.dispatchKeyEvent",
            json!({"type": "keyDown", "key": key}),
        )?;
        input_call(
            socket,
            progress,
            "Input.dispatchKeyEvent",
            json!({"type": "keyUp", "key": key}),
        )
    })();
    if let Err(error) = result {
        socket.begin_cleanup();
        let _ = socket.call(
            "Input.dispatchKeyEvent",
            Some(json!({"type": "keyUp", "key": key})),
        );
        return Err(error);
    }

    Ok(())
}

fn drag_step_count(duration: Duration) -> u32 {
    let duration_ms = duration.as_millis();
    if duration_ms == 0 {
        1
    } else {
        duration_ms.div_ceil(DRAG_STEP_INTERVAL.as_millis()).min(60) as u32
    }
}

fn drag_step_elapsed(duration: Duration, step: u32, steps: u32) -> Duration {
    let nanos = duration.as_nanos() * u128::from(step) / u128::from(steps);
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

fn page_summary(target: &TargetInfo, mode: RedactionMode) -> InteractionPageSummary {
    InteractionPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    }
}

fn sleep_until_next_poll(deadline: Instant) {
    let now = Instant::now();
    if now < deadline {
        thread::sleep((deadline - now).min(INTERACTION_POLL_INTERVAL));
    }
}

fn sleep_until(deadline: Instant) {
    let now = Instant::now();
    if now < deadline {
        thread::sleep(deadline - now);
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_actionability_blocker_survives_deadline_without_input() {
        use std::net::TcpListener;
        use tungstenite::Message;
        for reason in [
            "element_disabled",
            "element_occluded",
            "element_not_visible",
            "element_not_focused",
            "element_not_editable",
            "element_unstable",
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let handle = thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                while let Ok(message) = socket.read() {
                    let command: serde_json::Value =
                        serde_json::from_str(&message.into_text().unwrap()).unwrap();
                    let result = match command["method"].as_str().unwrap() {
                        "Runtime.evaluate" => json!({"result":{"objectId":"target"}}),
                        "Runtime.callFunctionOn" => json!({"result":{"value":{"error":reason}}}),
                        "Runtime.releaseObjectGroup" => json!({}),
                        method => panic!("blocked action dispatched {method}"),
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
            });
            let started = Instant::now();
            let mut socket = CdpWebSocket::connect_until(
                &format!("ws://{address}/page"),
                started + Duration::from_millis(150),
            )
            .unwrap();
            let summary = interact_on_socket(
                &mut socket,
                "#target",
                ActionRequest::default(),
                OperationDeadline::from(Duration::from_millis(120)),
            );
            assert!(!summary.dispatched);
            assert!(summary.timed_out);
            assert_eq!(summary.immediate_error, Some(reason));
            assert_eq!(summary.dispatch_state, DispatchState::NotDispatched);
            assert!(started.elapsed() < Duration::from_millis(500));
            drop(socket);
            handle.join().unwrap();
        }
    }

    #[test]
    fn unavailable_page_activation_prevents_text_and_global_key_input() {
        use std::net::TcpListener;
        use tungstenite::Message;
        for action in [InteractionAction::Type, InteractionAction::Key] {
            for stalled in [false, true] {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let address = listener.local_addr().unwrap();
                let handle = thread::spawn(move || {
                    let (stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut socket = tungstenite::accept(stream).unwrap();
                    let command: serde_json::Value =
                        serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
                    assert_eq!(command["method"], "Page.bringToFront");
                    if stalled {
                        thread::sleep(Duration::from_millis(180));
                    } else {
                        socket.send(Message::Text(json!({"id":command["id"],"error":{"code":-32000,"message":"activation unavailable"}}).to_string().into())).unwrap();
                    }
                    assert!(
                        socket.read().is_err(),
                        "activation failure must not dispatch or prepare input"
                    );
                });
                let budget = OperationDeadline::from(Duration::from_millis(120));
                let mut socket =
                    CdpWebSocket::connect_until(&format!("ws://{address}/page"), budget.end())
                        .unwrap();
                let summary = interact_on_socket(
                    &mut socket,
                    "body",
                    ActionRequest {
                        action,
                        text: Some("synthetic"),
                        key: Some("Enter"),
                        ..ActionRequest::default()
                    },
                    budget,
                );
                assert!(!summary.dispatched);
                assert_eq!(summary.dispatch_state, DispatchState::NotDispatched);
                assert_eq!(summary.timed_out, stalled);
                assert_eq!(
                    summary.immediate_error,
                    Some(if stalled {
                        "cdp_command_uncertain"
                    } else {
                        "cdp_command_failed"
                    })
                );
                assert!(budget.started.elapsed() < Duration::from_millis(400));
                drop(socket);
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn observed_blocker_survives_a_later_incomplete_stability_probe() {
        use std::net::TcpListener;
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            let mut probe_count = 0;
            while let Ok(message) = socket.read() {
                let command: serde_json::Value =
                    serde_json::from_str(&message.into_text().unwrap()).unwrap();
                let result = match command["method"].as_str().unwrap() {
                    "Runtime.evaluate" => json!({"result":{"objectId":"target"}}),
                    "Runtime.releaseObjectGroup" => json!({}),
                    "Runtime.callFunctionOn" => {
                        probe_count += 1;
                        match probe_count {
                            1 => json!({"result":{"value":{"error":"element_disabled"}}}),
                            2 => {
                                json!({"result":{"value":{"node_name":"BUTTON","rect":[0,0,50,50],"point":{"x":25,"y":25}}}})
                            }
                            3 => {
                                thread::sleep(Duration::from_millis(300));
                                break;
                            }
                            _ => panic!("unexpected extra probe"),
                        }
                    }
                    method => panic!("incomplete preparation dispatched {method}"),
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
        let budget = OperationDeadline::from(Duration::from_millis(350));
        let mut socket =
            CdpWebSocket::connect_until(&format!("ws://{address}/page"), budget.end()).unwrap();
        let summary = interact_on_socket(&mut socket, "#target", ActionRequest::default(), budget);
        assert!(!summary.dispatched);
        assert!(summary.timed_out);
        assert_eq!(summary.immediate_error, Some("element_disabled"));
        assert_eq!(summary.dispatch_state, DispatchState::NotDispatched);
        drop(socket);
        handle.join().unwrap();
    }

    #[test]
    fn uncertain_click_and_key_get_one_release_without_replay() {
        use std::net::TcpListener;
        use tungstenite::Message;
        for key in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let handle = thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                let events: &[&str] = if key {
                    &["keyDown", "keyUp"]
                } else {
                    &["mouseMoved", "mousePressed", "mouseReleased"]
                };
                for expected in events {
                    let command: serde_json::Value =
                        serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
                    assert_eq!(command["params"]["type"], *expected);
                    if *expected == "mouseMoved" {
                        socket
                            .send(Message::Text(
                                json!({"id":command["id"],"result":{}}).to_string().into(),
                            ))
                            .unwrap();
                    }
                }
                // Neither press nor cleanup is acknowledged. No input replay follows.
                thread::sleep(Duration::from_millis(150));
                assert!(socket.read().is_err());
            });
            let started = Instant::now();
            let mut socket = CdpWebSocket::connect_until(
                &format!("ws://{address}/page"),
                started + Duration::from_millis(80),
            )
            .unwrap();
            let mut progress = InputProgress::default();
            let result = if key {
                dispatch_key(&mut socket, "Enter", &mut progress)
            } else {
                dispatch_click(
                    &mut socket,
                    InteractionPoint { x: 1., y: 1. },
                    &mut progress,
                )
            };
            assert!(matches!(
                result,
                Err(InteractionError::Cdp(CdpError::CommandUncertain { .. }))
            ));
            assert!(progress.possible);
            assert_eq!(progress.acknowledged, usize::from(!key));
            assert!(started.elapsed() < Duration::from_millis(400));
            drop(socket);
            handle.join().unwrap();
        }
    }

    #[test]
    fn cleanup_failure_preserves_original_input_error() {
        use std::net::TcpListener;
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            for (event, message) in [
                ("keyDown", "original rejection"),
                ("keyUp", "cleanup rejection"),
            ] {
                let command: serde_json::Value =
                    serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
                assert_eq!(command["params"]["type"], event);
                socket
                    .send(Message::Text(
                        json!({"id":command["id"],"error":{"code":-32000,"message":message}})
                            .to_string()
                            .into(),
                    ))
                    .unwrap();
            }
        });
        let mut socket = CdpWebSocket::connect_until(
            &format!("ws://{address}/page"),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        let error = dispatch_key(&mut socket, "Enter", &mut InputProgress::default()).unwrap_err();
        assert!(
            matches!(error, InteractionError::Cdp(CdpError::Command { message, .. }) if message == "original rejection")
        );
        handle.join().unwrap();
    }

    #[test]
    fn uncertain_drag_press_gets_one_bounded_release_without_replay() {
        use std::net::TcpListener;
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            for expected in ["mouseMoved", "mousePressed", "mouseReleased"] {
                let command: serde_json::Value =
                    serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
                assert_eq!(command["params"]["type"], expected);
                if expected == "mouseMoved" {
                    socket
                        .send(Message::Text(
                            json!({"id":command["id"],"result":{}}).to_string().into(),
                        ))
                        .unwrap();
                }
                // Withhold press and release acknowledgements to exercise both budgets.
            }
            std::thread::sleep(Duration::from_millis(200));
        });
        let started = Instant::now();
        let mut socket = CdpWebSocket::connect_until(
            &format!("ws://{address}/page"),
            started + Duration::from_millis(80),
        )
        .unwrap();
        let point = InteractionPoint { x: 1., y: 1. };
        assert!(matches!(
            dispatch_drag(
                &mut socket,
                point,
                point,
                Duration::from_secs(5),
                &mut InputProgress::default()
            ),
            Err(InteractionError::Cdp(CdpError::CommandUncertain { .. }))
        ));
        assert!(started.elapsed() < Duration::from_millis(400));
        handle.join().unwrap();
    }

    #[test]
    fn drag_duration_cannot_extend_operation_budget() {
        use std::net::TcpListener;
        use tungstenite::Message;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let command: serde_json::Value =
                    serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
                socket
                    .send(Message::Text(
                        json!({"id":command["id"],"result":{}}).to_string().into(),
                    ))
                    .unwrap();
                if command["params"]["type"] == "mouseReleased" {
                    break;
                }
            }
        });
        let started = Instant::now();
        let mut socket = CdpWebSocket::connect_until(
            &format!("ws://{address}/page"),
            started + Duration::from_millis(80),
        )
        .unwrap();
        assert!(
            dispatch_drag(
                &mut socket,
                InteractionPoint { x: 1., y: 1. },
                InteractionPoint { x: 50., y: 50. },
                Duration::from_secs(5),
                &mut InputProgress::default()
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_millis(400));
        handle.join().unwrap();
    }

    #[test]
    fn drag_timing_spans_requested_duration_with_bounded_steps() {
        let long_duration = Duration::from_millis(30_000);
        let long_steps = drag_step_count(long_duration);

        assert_eq!(long_steps, 60);
        assert_eq!(
            drag_step_elapsed(long_duration, 1, long_steps),
            Duration::from_millis(500)
        );
        assert_eq!(
            drag_step_elapsed(long_duration, long_steps, long_steps),
            long_duration
        );

        let smoke_duration = Duration::from_millis(250);
        let smoke_steps = drag_step_count(smoke_duration);
        assert_eq!(smoke_steps, 16);
        assert_eq!(
            drag_step_elapsed(smoke_duration, smoke_steps, smoke_steps),
            smoke_duration
        );
    }
}
