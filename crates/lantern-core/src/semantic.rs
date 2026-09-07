//! Operation-scoped computed accessibility and ordinary main-document targets.
use crate::cdp::{CdpError, CdpWebSocket};
use crate::redaction::{RedactionMode, sanitize_dom_text, truncate_scalars};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionTarget {
    Css { selector: String },
    Role { role: String, name: String },
    TestId { test_id: String },
}
impl From<&str> for InteractionTarget {
    fn from(selector: &str) -> Self {
        Self::Css {
            selector: selector.into(),
        }
    }
}
impl From<&String> for InteractionTarget {
    fn from(selector: &String) -> Self {
        Self::from(selector.as_str())
    }
}
impl From<&InteractionTarget> for InteractionTarget {
    fn from(target: &InteractionTarget) -> Self {
        target.clone()
    }
}
impl InteractionTarget {
    pub(crate) fn css(&self) -> Option<&str> {
        match self {
            Self::Css { selector } => Some(selector),
            _ => None,
        }
    }
    pub(crate) fn summary(&self, mode: RedactionMode) -> Self {
        let clean = |s: &str| truncate_scalars(&sanitize_dom_text(s, mode), 500);
        match self {
            Self::Css { selector } => Self::Css {
                selector: selector.clone(),
            },
            Self::Role { role, name } => Self::Role {
                role: clean(role),
                name: clean(name),
            },
            Self::TestId { test_id } => Self::TestId {
                test_id: clean(test_id),
            },
        }
    }
}

pub(crate) const GROUP: &str = "lantern-interaction";
fn invalid() -> CdpError {
    CdpError::ResponseInvalid {
        context: "invalid semantic DOM response",
        source: "missing bounded result".into(),
    }
}

pub(crate) fn resolve(
    socket: &mut CdpWebSocket,
    target: &InteractionTarget,
) -> Result<Value, CdpError> {
    let expression = match target {
        InteractionTarget::Css { selector } => {
            format!("document.querySelectorAll({})", json!(selector))
        }
        InteractionTarget::TestId { test_id } => format!(
            "Array.from(document.querySelectorAll('[data-testid]')).filter(n => n.getAttribute('data-testid') === {})",
            json!(test_id)
        ),
        InteractionTarget::Role { role, name } => {
            let root = socket.call("DOM.getDocument", Some(json!({"depth":0})))?;
            let nodes = socket.call(
                "Accessibility.queryAXTree",
                Some(json!({"nodeId":root["root"]["nodeId"],"role":role,"accessibleName":name})),
            )?;
            let nodes = nodes["nodes"].as_array().ok_or_else(invalid)?;
            let mut found = None;
            for node in nodes {
                if node["ignored"] != false
                    || node["role"]["value"] != *role
                    || node["name"]["value"] != *name
                {
                    continue;
                }
                if let Some(object) = resolve_element(socket, node)? {
                    if found.is_some() {
                        return Ok(json!({"result":{"value":"ambiguous_selector"}}));
                    }
                    found = Some(object);
                }
            }
            return Ok(found
                .map(|object| json!({"result":object}))
                .unwrap_or_else(|| json!({"result":{"value":"selector_not_found"}})));
        }
    };
    socket.call("Runtime.evaluate", Some(json!({"expression":format!("(() => {{ try {{ const nodes = {expression}; return nodes.length === 1 ? nodes[0] : nodes.length ? 'ambiguous_selector' : 'selector_not_found'; }} catch (_) {{ return 'selector_invalid'; }} }})()"),"objectGroup":GROUP})))
}

// backendDOMNodeId is a BackendNodeId, never a frontend NodeId. Validate the
// resolved wrapper in the main world before accepting it as an input candidate.
pub(crate) fn resolve_element(
    socket: &mut CdpWebSocket,
    node: &Value,
) -> Result<Option<Value>, CdpError> {
    let Some(backend) = node["backendDOMNodeId"].as_u64() else {
        return Ok(None);
    };
    let result = socket.call(
        "DOM.resolveNode",
        Some(json!({"backendNodeId":backend,"objectGroup":GROUP})),
    )?;
    let object = &result["object"];
    let id = object["objectId"].as_str().ok_or_else(invalid)?;
    // Calling in the main execution context, with the wrapper as an argument,
    // excludes same-origin child documents as well as shadow-root descendants.
    let main = socket.call(
        "Runtime.evaluate",
        Some(json!({"expression":"document", "objectGroup":GROUP})),
    )?;
    let checked = socket.call("Runtime.callFunctionOn", Some(json!({
        "objectId":main["result"]["objectId"],
        "functionDeclaration":"function(node){return node instanceof Element && node.isConnected && node.ownerDocument === this && node.getRootNode() === this}",
        "arguments":[{"objectId":id}],"returnByValue":true
    })))?;
    Ok((checked["result"]["value"] == true).then(|| object.clone()))
}

pub(crate) fn same_target(
    socket: &mut CdpWebSocket,
    target: &InteractionTarget,
    object_id: &str,
) -> Result<Option<&'static str>, CdpError> {
    let next = resolve(socket, target)?;
    if let Some(code) = next["result"]["value"].as_str() {
        return Ok(Some(if code == "ambiguous_selector" {
            "ambiguous_selector"
        } else {
            "element_unstable"
        }));
    }
    let next_id = next["result"]["objectId"].as_str().ok_or_else(invalid)?;
    let value = socket.call("Runtime.callFunctionOn", Some(json!({"objectId":object_id,"functionDeclaration":"function(other){return this === other && this.isConnected}","arguments":[{"objectId":next_id}],"returnByValue":true})))?;
    Ok((value["result"]["value"] != true).then_some("element_unstable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        time::{Duration, Instant},
    };
    use tungstenite::Message;

    #[test]
    fn backend_identity_is_resolved_not_joined_to_frontend_ids() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            for (method, result) in [
                ("DOM.getDocument", json!({"root":{"nodeId":1}})),
                (
                    "Accessibility.queryAXTree",
                    json!({"nodes":[
                        {"ignored":true,"role":{"value":"button"},"name":{"value":"Save"},"backendDOMNodeId":999},
                        {"ignored":false,"role":{"value":"button"},"name":{"value":"Save"},"backendDOMNodeId":42}
                    ]}),
                ),
                (
                    "DOM.resolveNode",
                    json!({"object":{"objectId":"backend-42-wrapper"}}),
                ),
                (
                    "Runtime.evaluate",
                    json!({"result":{"objectId":"main-document"}}),
                ),
                ("Runtime.callFunctionOn", json!({"result":{"value":true}})),
            ] {
                let request: Value =
                    serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
                assert_eq!(request["method"], method);
                if method == "DOM.resolveNode" {
                    assert_eq!(request["params"]["backendNodeId"], 42);
                    assert!(request["params"].get("nodeId").is_none());
                }
                if method == "Runtime.callFunctionOn" {
                    assert_eq!(request["params"]["objectId"], "main-document");
                    assert_eq!(
                        request["params"]["arguments"][0]["objectId"],
                        "backend-42-wrapper"
                    );
                }
                ws.send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            }
        });
        let mut socket = CdpWebSocket::connect_until(
            &format!("ws://{address}"),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        let result = resolve(
            &mut socket,
            &InteractionTarget::Role {
                role: "button".into(),
                name: "Save".into(),
            },
        )
        .unwrap();
        assert_eq!(result["result"]["objectId"], "backend-42-wrapper");
        server.join().unwrap();
    }

    #[test]
    fn semantic_request_summaries_redact_and_bound_both_modes() {
        let target = InteractionTarget::Role {
            role: "button".into(),
            name: format!("token=secret-value {}", "x".repeat(2000)),
        };
        let redacted = serde_json::to_string(&target.summary(RedactionMode::Redacted)).unwrap();
        assert!(!redacted.contains("secret-value"));
        assert!(redacted.len() < 600);
        let unredacted = serde_json::to_string(&target.summary(RedactionMode::Unredacted)).unwrap();
        assert!(unredacted.contains("secret-value"));
        assert!(unredacted.len() < 600);
        let test = InteractionTarget::TestId {
            test_id: "token=secret-value".into(),
        };
        assert!(
            !serde_json::to_string(&test.summary(RedactionMode::Redacted))
                .unwrap()
                .contains("secret-value")
        );
    }
}
