use std::{
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    cdp::{CdpError, CdpWebSocket, TargetInfo},
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
    fn success(
        command: &'static str,
        page: InteractionPageSummary,
        interaction: InteractionSummary,
    ) -> Self {
        Self {
            schema_version: INTERACTION_SCHEMA_VERSION,
            command,
            ok: true,
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
    pub timed_out: bool,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub observed: InteractionObservedState,
    pub immediate_error: Option<&'static str>,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
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
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            if let Some(clickable_point) = clickable_point(&mut socket, node_id)? {
                dispatch_click(&mut socket, clickable_point)?;
                return Ok(InteractionCommandOutput::success(
                    "click",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Click,
                        selector: selector.to_owned(),
                        dispatched: true,
                        timed_out: false,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::click(node_name, Some(clickable_point)),
                        immediate_error: None,
                    },
                ));
            }

            if Instant::now() >= deadline {
                return Ok(InteractionCommandOutput::success(
                    "click",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Click,
                        selector: selector.to_owned(),
                        dispatched: false,
                        timed_out: true,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::click(node_name, None),
                        immediate_error: Some("element_not_clickable"),
                    },
                ));
            }
        } else if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "click",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Click,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::click(None, None),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

pub fn type_text(
    target: &TargetInfo,
    selector: &str,
    text: &str,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            socket.call("DOM.focus", Some(json!({ "nodeId": node_id })))?;
            socket.call("Input.insertText", Some(json!({ "text": text })))?;
            return Ok(InteractionCommandOutput::success(
                "type",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Type,
                    selector: selector.to_owned(),
                    dispatched: true,
                    timed_out: false,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::type_text(
                        node_name,
                        Some(text.chars().count()),
                    ),
                    immediate_error: None,
                },
            ));
        }

        if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "type",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Type,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::type_text(None, None),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

pub fn press_key(
    target: &TargetInfo,
    selector: &str,
    key: &str,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            let _ = socket.call("DOM.focus", Some(json!({ "nodeId": node_id })));
            dispatch_key(&mut socket, key)?;
            return Ok(InteractionCommandOutput::success(
                "key",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Key,
                    selector: selector.to_owned(),
                    dispatched: true,
                    timed_out: false,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::key(node_name, key, 2),
                    immediate_error: None,
                },
            ));
        }

        if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "key",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Key,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::key(None, key, 0),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

pub fn hover_element(
    target: &TargetInfo,
    selector: &str,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            if let Some(point) = clickable_point(&mut socket, node_id)? {
                dispatch_hover(&mut socket, point)?;
                return Ok(InteractionCommandOutput::success(
                    "hover",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Hover,
                        selector: selector.to_owned(),
                        dispatched: true,
                        timed_out: false,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::pointer(
                            node_name,
                            Some(point),
                            None,
                            None,
                            None,
                            Some(1),
                        ),
                        immediate_error: None,
                    },
                ));
            }

            if Instant::now() >= deadline {
                return Ok(InteractionCommandOutput::success(
                    "hover",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Hover,
                        selector: selector.to_owned(),
                        dispatched: false,
                        timed_out: true,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::pointer(
                            node_name,
                            None,
                            None,
                            None,
                            None,
                            Some(0),
                        ),
                        immediate_error: Some("element_not_pointable"),
                    },
                ));
            }
        } else if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "hover",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Hover,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::pointer(
                        None,
                        None,
                        None,
                        None,
                        None,
                        Some(0),
                    ),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

pub fn wheel_element(
    target: &TargetInfo,
    selector: &str,
    delta_x: f64,
    delta_y: f64,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            if let Some(point) = clickable_point(&mut socket, node_id)? {
                dispatch_wheel(&mut socket, point, delta_x, delta_y)?;
                return Ok(InteractionCommandOutput::success(
                    "wheel",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Wheel,
                        selector: selector.to_owned(),
                        dispatched: true,
                        timed_out: false,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::pointer(
                            node_name,
                            Some(point),
                            Some(delta_x),
                            Some(delta_y),
                            None,
                            Some(1),
                        ),
                        immediate_error: None,
                    },
                ));
            }

            if Instant::now() >= deadline {
                return Ok(InteractionCommandOutput::success(
                    "wheel",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Wheel,
                        selector: selector.to_owned(),
                        dispatched: false,
                        timed_out: true,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::pointer(
                            node_name,
                            None,
                            Some(delta_x),
                            Some(delta_y),
                            None,
                            Some(0),
                        ),
                        immediate_error: Some("element_not_pointable"),
                    },
                ));
            }
        } else if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "wheel",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Wheel,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::pointer(
                        None,
                        None,
                        Some(delta_x),
                        Some(delta_y),
                        None,
                        Some(0),
                    ),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

pub fn drag_element(
    target: &TargetInfo,
    selector: &str,
    delta_x: f64,
    delta_y: f64,
    duration: Duration,
    timeout: Duration,
    mode: RedactionMode,
) -> Result<InteractionCommandOutput, InteractionError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(InteractionError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let started = Instant::now();
    let deadline = started + timeout;

    loop {
        if let Some(node_id) = query_selector(&mut socket, selector)? {
            let node_name = describe_node_name(&mut socket, node_id).ok().flatten();
            if let Some(start_point) = clickable_point(&mut socket, node_id)? {
                let end_point = InteractionPoint {
                    x: (start_point.x + delta_x).round(),
                    y: (start_point.y + delta_y).round(),
                };
                let input_event_count =
                    dispatch_drag(&mut socket, start_point, end_point, duration)?;
                return Ok(InteractionCommandOutput::success(
                    "drag",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Drag,
                        selector: selector.to_owned(),
                        dispatched: true,
                        timed_out: false,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::drag(
                            node_name,
                            Some(start_point),
                            Some(end_point),
                            delta_x,
                            delta_y,
                            duration_millis(duration),
                            input_event_count,
                        ),
                        immediate_error: None,
                    },
                ));
            }

            if Instant::now() >= deadline {
                return Ok(InteractionCommandOutput::success(
                    "drag",
                    page_summary(target, mode),
                    InteractionSummary {
                        action: InteractionAction::Drag,
                        selector: selector.to_owned(),
                        dispatched: false,
                        timed_out: true,
                        elapsed_ms: duration_millis(started.elapsed()),
                        timeout_ms: duration_millis(timeout),
                        observed: InteractionObservedState::drag(
                            node_name,
                            None,
                            None,
                            delta_x,
                            delta_y,
                            duration_millis(duration),
                            0,
                        ),
                        immediate_error: Some("element_not_pointable"),
                    },
                ));
            }
        } else if Instant::now() >= deadline {
            return Ok(InteractionCommandOutput::success(
                "drag",
                page_summary(target, mode),
                InteractionSummary {
                    action: InteractionAction::Drag,
                    selector: selector.to_owned(),
                    dispatched: false,
                    timed_out: true,
                    elapsed_ms: duration_millis(started.elapsed()),
                    timeout_ms: duration_millis(timeout),
                    observed: InteractionObservedState::drag(
                        None,
                        None,
                        None,
                        delta_x,
                        delta_y,
                        duration_millis(duration),
                        0,
                    ),
                    immediate_error: Some("selector_not_found"),
                },
            ));
        }

        sleep_until_next_poll(deadline);
    }
}

fn query_selector(
    socket: &mut CdpWebSocket,
    selector: &str,
) -> Result<Option<u64>, InteractionError> {
    let document = socket.call("DOM.getDocument", Some(json!({ "depth": 0 })))?;
    let response: CdpGetDocumentResponse =
        serde_json::from_value(document).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP DOM.getDocument response",
            source: source.to_string(),
        })?;

    let query = socket.call(
        "DOM.querySelector",
        Some(json!({
            "nodeId": response.root.node_id,
            "selector": selector
        })),
    )?;
    let response: CdpQuerySelectorResponse =
        serde_json::from_value(query).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP DOM.querySelector response",
            source: source.to_string(),
        })?;

    Ok(if response.node_id == 0 {
        None
    } else {
        Some(response.node_id)
    })
}

fn describe_node_name(
    socket: &mut CdpWebSocket,
    node_id: u64,
) -> Result<Option<String>, InteractionError> {
    let result = socket.call("DOM.describeNode", Some(json!({ "nodeId": node_id })))?;
    let response: CdpDescribeNodeResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP DOM.describeNode response",
            source: source.to_string(),
        })?;

    Ok(Some(response.node.node_name))
}

fn clickable_point(
    socket: &mut CdpWebSocket,
    node_id: u64,
) -> Result<Option<InteractionPoint>, InteractionError> {
    let result = match socket.call("DOM.getBoxModel", Some(json!({ "nodeId": node_id }))) {
        Ok(result) => result,
        Err(CdpError::Command { .. }) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let response: CdpGetBoxModelResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP DOM.getBoxModel response",
            source: source.to_string(),
        })?;

    point_from_quad(&response.model.content)
}

fn point_from_quad(quad: &[f64]) -> Result<Option<InteractionPoint>, InteractionError> {
    if quad.len() < 8 || quad.iter().any(|value| !value.is_finite()) {
        return Ok(None);
    }

    let x_values = [quad[0], quad[2], quad[4], quad[6]];
    let y_values = [quad[1], quad[3], quad[5], quad[7]];
    let min_x = x_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = x_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_y = y_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = y_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    if min_x >= max_x || min_y >= max_y {
        return Ok(None);
    }

    Ok(Some(InteractionPoint {
        x: ((min_x + max_x) / 2.0).round(),
        y: ((min_y + max_y) / 2.0).round(),
    }))
}

fn dispatch_click(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
) -> Result<(), InteractionError> {
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseMoved",
            "x": point.x,
            "y": point.y,
            "button": "none"
        })),
    )?;
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mousePressed",
            "x": point.x,
            "y": point.y,
            "button": "left",
            "clickCount": 1
        })),
    )?;
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseReleased",
            "x": point.x,
            "y": point.y,
            "button": "left",
            "clickCount": 1
        })),
    )?;

    Ok(())
}

fn dispatch_hover(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
) -> Result<(), InteractionError> {
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseMoved",
            "x": point.x,
            "y": point.y,
            "button": "none"
        })),
    )?;

    Ok(())
}

fn dispatch_wheel(
    socket: &mut CdpWebSocket,
    point: InteractionPoint,
    delta_x: f64,
    delta_y: f64,
) -> Result<(), InteractionError> {
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseWheel",
            "x": point.x,
            "y": point.y,
            "deltaX": delta_x,
            "deltaY": delta_y
        })),
    )?;

    Ok(())
}

fn dispatch_drag(
    socket: &mut CdpWebSocket,
    start: InteractionPoint,
    end: InteractionPoint,
    duration: Duration,
) -> Result<usize, InteractionError> {
    dispatch_hover(socket, start)?;
    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mousePressed",
            "x": start.x,
            "y": start.y,
            "button": "left",
            "buttons": 1,
            "clickCount": 1
        })),
    )?;

    let steps = drag_step_count(duration);
    let drag_started = Instant::now();
    let mut event_count = 2;
    for step in 1..=steps {
        if !duration.is_zero() {
            sleep_until(drag_started + drag_step_elapsed(duration, step, steps));
        }
        let progress = step as f64 / steps as f64;
        let point = InteractionPoint {
            x: (start.x + ((end.x - start.x) * progress)).round(),
            y: (start.y + ((end.y - start.y) * progress)).round(),
        };
        socket.call(
            "Input.dispatchMouseEvent",
            Some(json!({
                "type": "mouseMoved",
                "x": point.x,
                "y": point.y,
                "button": "left",
                "buttons": 1
            })),
        )?;
        event_count += 1;
    }

    socket.call(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseReleased",
            "x": end.x,
            "y": end.y,
            "button": "left",
            "buttons": 0,
            "clickCount": 1
        })),
    )?;
    event_count += 1;

    Ok(event_count)
}

fn dispatch_key(socket: &mut CdpWebSocket, key: &str) -> Result<(), InteractionError> {
    socket.call(
        "Input.dispatchKeyEvent",
        Some(json!({
            "type": "keyDown",
            "key": key
        })),
    )?;
    socket.call(
        "Input.dispatchKeyEvent",
        Some(json!({
            "type": "keyUp",
            "key": key
        })),
    )?;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpGetDocumentResponse {
    root: CdpDocumentRoot,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpDocumentRoot {
    node_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpQuerySelectorResponse {
    node_id: u64,
}

#[derive(Debug, Deserialize)]
struct CdpDescribeNodeResponse {
    node: CdpDescribedNode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpDescribedNode {
    node_name: String,
}

#[derive(Debug, Deserialize)]
struct CdpGetBoxModelResponse {
    model: CdpBoxModel,
}

#[derive(Debug, Deserialize)]
struct CdpBoxModel {
    content: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_center_point_from_content_quad() {
        let point = point_from_quad(&[10.0, 20.0, 110.0, 20.0, 110.0, 60.0, 10.0, 60.0])
            .expect("quad should parse")
            .expect("quad should have area");

        assert_eq!(point, InteractionPoint { x: 60.0, y: 40.0 });
    }

    #[test]
    fn rejects_degenerate_content_quad() {
        assert_eq!(
            point_from_quad(&[10.0, 20.0, 10.0, 20.0, 10.0, 20.0, 10.0, 20.0])
                .expect("quad should parse"),
            None
        );
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
