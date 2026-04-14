use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    cdp::{CdpError, CdpWebSocket, TargetInfo},
    redaction::{RedactionMode, sanitize_dom_text, sanitize_title, sanitize_url},
};

pub const LAYOUT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: LayoutPageSummary,
    pub layout: LayoutAudit,
}

impl LayoutCommandOutput {
    pub fn success(page: LayoutPageSummary, layout: LayoutAudit) -> Self {
        Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            command: "layout",
            ok: true,
            page,
            layout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutAudit {
    pub viewport: LayoutViewport,
    pub finding_count: usize,
    pub truncated: bool,
    pub findings: Vec<LayoutFinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutViewport {
    pub inner_width: f64,
    pub inner_height: f64,
    pub client_width: f64,
    pub client_height: f64,
    pub scroll_width: f64,
    pub scroll_height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutFinding {
    pub kind: String,
    pub severity: String,
    pub selector: String,
    pub tag: String,
    pub id: Option<String>,
    pub class_sample: Option<String>,
    pub text_sample: Option<String>,
    pub metrics: LayoutFindingMetrics,
    pub container_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFindingMetrics {
    pub left: f64,
    pub right: f64,
    pub width: f64,
    pub client_width: f64,
    pub scroll_width: f64,
    pub container_left: Option<f64>,
    pub container_right: Option<f64>,
    pub viewport_width: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutReadError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for LayoutReadError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn read_layout_audit(
    target: &TargetInfo,
    mode: RedactionMode,
) -> Result<LayoutCommandOutput, LayoutReadError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(LayoutReadError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let result = socket.call(
        "Runtime.evaluate",
        Some(json!({
            "expression": LAYOUT_AUDIT_SCRIPT,
            "returnByValue": true,
            "timeout": 2000
        })),
    )?;
    let response: CdpRuntimeEvaluateResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP Runtime.evaluate response",
            source: source.to_string(),
        })?;
    let raw = response
        .result
        .value
        .ok_or_else(|| CdpError::ResponseInvalid {
            context: "failed to read layout audit response",
            source: "Runtime.evaluate returned no by-value result".to_owned(),
        })?;
    let raw: RawLayoutAudit =
        serde_json::from_value(raw).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse layout audit result",
            source: source.to_string(),
        })?;

    let findings = raw
        .findings
        .into_iter()
        .map(|finding| LayoutFinding {
            kind: finding.kind,
            severity: finding.severity,
            selector: finding.selector,
            tag: finding.tag,
            id: finding.id,
            class_sample: finding.class_sample,
            text_sample: finding
                .text_sample
                .map(|text| sanitize_dom_text(&text, mode)),
            metrics: finding.metrics,
            container_selector: finding.container_selector,
        })
        .collect::<Vec<_>>();
    let layout = LayoutAudit {
        viewport: raw.viewport,
        finding_count: findings.len(),
        truncated: raw.truncated,
        findings,
    };
    let page = LayoutPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    };

    Ok(LayoutCommandOutput::success(page, layout))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpRuntimeEvaluateResponse {
    result: CdpRuntimeRemoteObject,
}

#[derive(Debug, Deserialize)]
struct CdpRuntimeRemoteObject {
    value: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLayoutAudit {
    viewport: LayoutViewport,
    truncated: bool,
    findings: Vec<RawLayoutFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLayoutFinding {
    kind: String,
    severity: String,
    selector: String,
    tag: String,
    id: Option<String>,
    class_sample: Option<String>,
    text_sample: Option<String>,
    metrics: LayoutFindingMetrics,
    container_selector: Option<String>,
}

const LAYOUT_AUDIT_SCRIPT: &str = r##"
(() => {
  const maxFindings = 40;
  const tolerance = 2;
  const viewport = {
    innerWidth: window.innerWidth,
    innerHeight: window.innerHeight,
    clientWidth: document.documentElement.clientWidth,
    clientHeight: document.documentElement.clientHeight,
    scrollWidth: document.documentElement.scrollWidth,
    scrollHeight: document.documentElement.scrollHeight,
  };
  const findings = [];
  let truncated = false;
  const seen = new Set();
  function cssPath(el) {
    if (!el || !el.tagName) return "unknown";
    const parts = [];
    let node = el;
    while (node && node.nodeType === 1 && parts.length < 5) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        part += "#" + node.id;
        parts.unshift(part);
        break;
      }
      const classes = Array.from(node.classList || []).slice(0, 3);
      if (classes.length) part += "." + classes.join(".");
      const parent = node.parentElement;
      if (parent) {
        const siblings = Array.from(parent.children).filter((child) => child.tagName === node.tagName);
        if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
      }
      parts.unshift(part);
      node = parent;
    }
    return parts.join(" > ");
  }
  function textSample(el) {
    const text = (el.innerText || el.textContent || "").replace(/\s+/g, " ").trim();
    return text ? text.slice(0, 160) : null;
  }
  function visible(el, rect, style) {
    return rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden";
  }
  function containerFor(el) {
    let node = el.parentElement;
    while (node && node.nodeType === 1) {
      if (node.matches(".panel, .next-action, .tracker-space, [data-layout-container]")) return node;
      node = node.parentElement;
    }
    return null;
  }
  function push(kind, severity, el, rect, container) {
    const key = `${kind}:${cssPath(el)}`;
    if (seen.has(key)) return;
    seen.add(key);
    if (findings.length >= maxFindings) {
      truncated = true;
      return;
    }
    const containerRect = container && container !== el ? container.getBoundingClientRect() : null;
    findings.push({
      kind,
      severity,
      selector: cssPath(el),
      tag: el.tagName.toLowerCase(),
      id: el.id || null,
      classSample: el.className && typeof el.className === "string" ? el.className.slice(0, 120) : null,
      textSample: textSample(el),
      metrics: {
        left: rect.left,
        right: rect.right,
        width: rect.width,
        clientWidth: el.clientWidth || 0,
        scrollWidth: el.scrollWidth || 0,
        containerLeft: containerRect ? containerRect.left : null,
        containerRight: containerRect ? containerRect.right : null,
        viewportWidth: viewport.clientWidth,
      },
      containerSelector: containerRect ? cssPath(container) : null,
    });
  }
  if (viewport.scrollWidth > viewport.clientWidth + tolerance) {
    const bodyRect = document.body.getBoundingClientRect();
    push("document-horizontal-overflow", "p1", document.body, bodyRect, null);
  }
  for (const el of Array.from(document.querySelectorAll("body *"))) {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    if (!visible(el, rect, style)) continue;
    const hasText = !!textSample(el);
    const overflowContainer = ["auto", "scroll"].includes(style.overflowX);
    if (hasText && !overflowContainer && el.scrollWidth > el.clientWidth + tolerance) {
      push("element-horizontal-overflow", "p2", el, rect, containerFor(el));
    }
    const container = containerFor(el);
    if (container && container !== el) {
      const containerRect = container.getBoundingClientRect();
      if (rect.right > containerRect.right + tolerance || rect.left < containerRect.left - tolerance) {
        push("element-escapes-container", "p2", el, rect, container);
      }
    }
    if (rect.right > viewport.clientWidth + tolerance || rect.left < -tolerance) {
      push("element-escapes-viewport", "p2", el, rect, null);
    }
  }
  return { viewport, truncated, findings };
})()
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_redacts_layout_audit_result() {
        let raw = json!({
            "viewport": {
                "innerWidth": 1200.0,
                "innerHeight": 800.0,
                "clientWidth": 1180.0,
                "clientHeight": 780.0,
                "scrollWidth": 1220.0,
                "scrollHeight": 900.0
            },
            "truncated": false,
            "findings": [{
                "kind": "element-horizontal-overflow",
                "severity": "p2",
                "selector": "main > table",
                "tag": "td",
                "id": null,
                "classSample": "summary",
                "textSample": "Profile token=secret https://example.test/session/abcdef123456abcdef123456abcdef12?debug=1",
                "metrics": {
                    "left": 10.0,
                    "right": 1300.0,
                    "width": 1290.0,
                    "clientWidth": 100.0,
                    "scrollWidth": 500.0,
                    "containerLeft": 0.0,
                    "containerRight": 1180.0,
                    "viewportWidth": 1180.0
                },
                "containerSelector": "main"
            }]
        });
        let raw: RawLayoutAudit = serde_json::from_value(raw).expect("raw audit");
        let findings = raw
            .findings
            .into_iter()
            .map(|finding| LayoutFinding {
                kind: finding.kind,
                severity: finding.severity,
                selector: finding.selector,
                tag: finding.tag,
                id: finding.id,
                class_sample: finding.class_sample,
                text_sample: finding
                    .text_sample
                    .map(|text| sanitize_dom_text(&text, RedactionMode::Redacted)),
                metrics: finding.metrics,
                container_selector: finding.container_selector,
            })
            .collect::<Vec<_>>();

        assert_eq!(findings.len(), 1);
        assert!(
            findings[0]
                .text_sample
                .as_deref()
                .unwrap()
                .contains(":redacted")
        );
    }
}
