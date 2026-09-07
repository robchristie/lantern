//! Compact computed AX evidence. No values, raw properties or reusable node IDs.
use crate::{
    cdp::{CdpError, CdpWebSocket, OperationDeadline, TargetInfo},
    redaction::{RedactionMode, sanitize_dom_text, truncate_scalars},
    semantic,
};
use serde::Serialize;
use serde_json::json;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct AccessibilityOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub scope: &'static str,
    pub target_id: String,
    pub depth_limit: usize,
    pub node_limit: usize,
    pub truncated: bool,
    pub nodes: Vec<AccessibleNode>,
    #[serde(skip_serializing_if = "crate::cdp::EvidenceLoss::is_complete")]
    pub evidence_loss: crate::cdp::EvidenceLoss,
}
#[derive(Debug, Serialize)]
pub struct AccessibleNode {
    pub role: String,
    pub name: String,
}

pub fn read_accessibility(
    target: &TargetInfo,
    mode: RedactionMode,
    depth: usize,
    max_nodes: usize,
    budget: OperationDeadline,
) -> Result<AccessibilityOutput, CdpError> {
    if crate::dom::DomSummaryOptions::new(depth, max_nodes).is_none() {
        return Err(CdpError::ResponseInvalid {
            context: "invalid accessibility limits",
            source: "depth must be 1–12 and nodes 1–500".into(),
        });
    }
    let url =
        target
            .web_socket_debugger_url
            .as_deref()
            .ok_or_else(|| CdpError::ResponseInvalid {
                context: "accessibility target has no websocket",
                source: "missing target websocket".into(),
            })?;
    let mut socket = CdpWebSocket::connect_until(url, budget.end())?;
    let result = (|| {
        let tree = socket.call("Accessibility.getFullAXTree", Some(json!({"depth":depth})))?;
        let nodes = tree["nodes"]
            .as_array()
            .ok_or_else(|| CdpError::ResponseInvalid {
                context: "invalid accessibility tree",
                source: "missing nodes".into(),
            })?;
        let mut summaries = Vec::new();
        // Depth bounds are deliberately reported as potentially truncated: the
        // browser does not provide a trustworthy total for the omitted subtree.
        let ids: std::collections::HashSet<_> =
            nodes.iter().filter_map(|n| n["nodeId"].as_str()).collect();
        let mut truncated = nodes.iter().any(|n| {
            n["childIds"].as_array().is_some_and(|children| {
                children
                    .iter()
                    .filter_map(|id| id.as_str())
                    .any(|id| !ids.contains(id))
            })
        });
        for node in nodes {
            if node["ignored"] != false {
                continue;
            }
            if semantic::resolve_element(&mut socket, node)?.is_none() {
                continue;
            }
            if summaries.len() == max_nodes {
                truncated = true;
                break;
            }
            let clean = |field| {
                truncate_scalars(
                    &sanitize_dom_text(node[field]["value"].as_str().unwrap_or(""), mode),
                    500,
                )
            };
            summaries.push(AccessibleNode {
                role: clean("role"),
                name: clean("name"),
            });
        }
        Ok(AccessibilityOutput {
            schema_version: 1,
            command: "accessibility",
            ok: true,
            scope: "ordinary_main_document_elements",
            target_id: target.id.clone(),
            depth_limit: depth,
            node_limit: max_nodes,
            truncated,
            nodes: summaries,
            evidence_loss: socket.evidence_loss(),
        })
    })();
    if Instant::now() < budget.end() {
        let _ = socket.call(
            "Runtime.releaseObjectGroup",
            Some(json!({"objectGroup":semantic::GROUP})),
        );
    }
    result
}
