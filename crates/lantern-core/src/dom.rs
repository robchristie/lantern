use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::{
    cdp::{CdpError, CdpWebSocket, TargetInfo},
    redaction::{
        RedactionMode, sanitize_dom_attribute, sanitize_dom_text, sanitize_title, sanitize_url,
    },
};

pub const DOM_SCHEMA_VERSION: u8 = 1;
pub const DOM_DEFAULT_DEPTH: usize = 4;
pub const DOM_DEFAULT_MAX_NODES: usize = 80;
pub const DOM_MAX_ALLOWED_DEPTH: usize = 12;
pub const DOM_MAX_ALLOWED_NODES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomSummaryOptions {
    pub max_depth: usize,
    pub max_nodes: usize,
}

impl DomSummaryOptions {
    pub fn new(max_depth: usize, max_nodes: usize) -> Option<Self> {
        if max_depth == 0
            || max_depth > DOM_MAX_ALLOWED_DEPTH
            || max_nodes == 0
            || max_nodes > DOM_MAX_ALLOWED_NODES
        {
            return None;
        }

        Some(Self {
            max_depth,
            max_nodes,
        })
    }
}

impl Default for DomSummaryOptions {
    fn default() -> Self {
        Self {
            max_depth: DOM_DEFAULT_DEPTH,
            max_nodes: DOM_DEFAULT_MAX_NODES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: DomPageSummary,
    pub dom: DomSummary,
}

impl DomCommandOutput {
    pub fn success(page: DomPageSummary, dom: DomSummary) -> Self {
        Self {
            schema_version: DOM_SCHEMA_VERSION,
            command: "dom",
            ok: true,
            page,
            dom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomSummary {
    pub node_count: usize,
    pub max_depth: usize,
    pub truncated: bool,
    pub nodes: Vec<DomNodeSummary>,
}

impl DomSummary {
    pub fn new(nodes: Vec<DomNodeSummary>, max_depth: usize, truncated: bool) -> Self {
        Self {
            node_count: count_nodes(&nodes),
            max_depth,
            truncated,
            nodes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomNodeSummary {
    pub node_id: String,
    pub tag: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub text: Option<String>,
    pub attributes: BTreeMap<String, String>,
    pub child_count: usize,
    pub children: Vec<DomNodeSummary>,
}

impl DomNodeSummary {
    pub fn new(node_id: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            tag: tag.into(),
            role: None,
            name: None,
            text: None,
            attributes: BTreeMap::new(),
            child_count: 0,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomReadError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for DomReadError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn read_dom_summary(
    target: &TargetInfo,
    mode: RedactionMode,
    options: DomSummaryOptions,
) -> Result<DomCommandOutput, DomReadError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(DomReadError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    let result = socket.call(
        "DOM.getDocument",
        Some(json!({
            "depth": options.max_depth,
            "pierce": false
        })),
    )?;
    let response: CdpGetDocumentResponse =
        serde_json::from_value(result).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP DOM.getDocument response",
            source: source.to_string(),
        })?;

    let mut builder = DomSummaryBuilder::new(mode, options);
    let nodes = builder.document_nodes(&response.root);
    let dom = DomSummary::new(nodes, builder.max_emitted_depth, builder.truncated);
    let page = DomPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    };

    Ok(DomCommandOutput::success(page, dom))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpGetDocumentResponse {
    root: CdpDomNode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpDomNode {
    node_id: i64,
    node_type: i64,
    node_name: String,
    local_name: Option<String>,
    node_value: Option<String>,
    attributes: Option<Vec<String>>,
    child_node_count: Option<usize>,
    children: Option<Vec<CdpDomNode>>,
}

struct DomSummaryBuilder {
    mode: RedactionMode,
    options: DomSummaryOptions,
    emitted: usize,
    max_emitted_depth: usize,
    truncated: bool,
}

impl DomSummaryBuilder {
    fn new(mode: RedactionMode, options: DomSummaryOptions) -> Self {
        Self {
            mode,
            options,
            emitted: 0,
            max_emitted_depth: 0,
            truncated: false,
        }
    }

    fn document_nodes(&mut self, root: &CdpDomNode) -> Vec<DomNodeSummary> {
        if root.node_type == 9 {
            return root
                .children
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter_map(|child| self.node_summary(child, 1))
                .collect();
        }

        self.node_summary(root, 1).into_iter().collect()
    }

    fn node_summary(&mut self, node: &CdpDomNode, depth: usize) -> Option<DomNodeSummary> {
        if self.emitted >= self.options.max_nodes {
            self.truncated = true;
            return None;
        }

        if node.node_type != 1 {
            return None;
        }

        self.emitted += 1;
        self.max_emitted_depth = self.max_emitted_depth.max(depth);
        let mut summary = DomNodeSummary::new(format!("n{}", node.node_id), node_tag(node));
        summary.attributes = sanitized_attributes(node.attributes.as_deref(), self.mode);
        summary.text = element_text(node, self.mode);

        let child_elements = node
            .children
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|child| child.node_type == 1)
            .collect::<Vec<_>>();
        let child_element_count = child_elements.len();
        summary.child_count = node.child_node_count.unwrap_or(child_element_count);

        if depth >= self.options.max_depth {
            if !child_elements.is_empty() || summary.child_count > child_elements.len() {
                self.truncated = true;
            }
            return Some(summary);
        }

        for child in child_elements {
            if self.emitted >= self.options.max_nodes {
                self.truncated = true;
                break;
            }
            if let Some(child_summary) = self.node_summary(child, depth + 1) {
                summary.children.push(child_summary);
            }
        }

        if summary.children.len() < child_element_count
            || summary.child_count > node.children.as_ref().map_or(0, Vec::len)
        {
            self.truncated = true;
        }

        Some(summary)
    }
}

fn node_tag(node: &CdpDomNode) -> String {
    node.local_name
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&node.node_name)
        .to_ascii_lowercase()
}

fn sanitized_attributes(
    attributes: Option<&[String]>,
    mode: RedactionMode,
) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();
    let Some(attributes) = attributes else {
        return output;
    };

    for pair in attributes.chunks(2) {
        let [name, value] = pair else {
            continue;
        };
        if let Some((name, value)) = sanitize_dom_attribute(name, value, mode) {
            output.insert(name, value);
        }
    }

    output
}

fn element_text(node: &CdpDomNode, mode: RedactionMode) -> Option<String> {
    if matches!(node_tag(node).as_str(), "script" | "style") {
        return None;
    }

    let text = node
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|child| child.node_type == 3)
        .filter_map(|child| child.node_value.as_deref())
        .collect::<Vec<_>>()
        .join(" ");

    let text = sanitize_dom_text(&text, mode);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn count_nodes(nodes: &[DomNodeSummary]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_nodes(&node.children))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};
    use tungstenite::Message;

    #[test]
    fn dom_summary_options_enforce_public_bounds() {
        assert_eq!(
            DomSummaryOptions::new(DOM_DEFAULT_DEPTH, DOM_DEFAULT_MAX_NODES),
            Some(DomSummaryOptions::default())
        );
        assert!(DomSummaryOptions::new(0, DOM_DEFAULT_MAX_NODES).is_none());
        assert!(DomSummaryOptions::new(DOM_MAX_ALLOWED_DEPTH + 1, DOM_DEFAULT_MAX_NODES).is_none());
        assert!(DomSummaryOptions::new(DOM_DEFAULT_DEPTH, 0).is_none());
        assert!(DomSummaryOptions::new(DOM_DEFAULT_DEPTH, DOM_MAX_ALLOWED_NODES + 1).is_none());
    }

    #[test]
    fn dom_command_output_serializes_in_contract_order() {
        let mut node = DomNodeSummary::new("n1", "main");
        node.attributes.insert("id".to_owned(), "app".to_owned());
        node.child_count = 3;

        let output = DomCommandOutput::success(
            DomPageSummary {
                target_id: "ABCD1234".to_owned(),
                title: Some("Example".to_owned()),
                url_shape: Some("https://example.test/path".to_owned()),
            },
            DomSummary::new(vec![node], 3, true),
        );

        let json = serde_json::to_string(&output).expect("DOM output should serialize");

        assert_eq!(
            json,
            r#"{"schema_version":1,"command":"dom","ok":true,"page":{"target_id":"ABCD1234","title":"Example","url_shape":"https://example.test/path"},"dom":{"node_count":1,"max_depth":3,"truncated":true,"nodes":[{"node_id":"n1","tag":"main","role":null,"name":null,"text":null,"attributes":{"id":"app"},"child_count":3,"children":[]}]}}"#
        );
    }

    #[test]
    fn dom_summary_counts_nested_emitted_nodes() {
        let mut root = DomNodeSummary::new("n1", "html");
        root.children.push(DomNodeSummary::new("n2", "body"));

        let summary = DomSummary::new(vec![root], DOM_DEFAULT_DEPTH, false);

        assert_eq!(summary.node_count, 2);
    }

    #[test]
    fn builds_bounded_dom_summary_from_cdp_document() {
        let response: CdpGetDocumentResponse = serde_json::from_str(
            r##"{
                "root": {
                    "nodeId": 1,
                    "nodeType": 9,
                    "nodeName": "#document",
                    "localName": "",
                    "childNodeCount": 1,
                    "children": [
                        {
                            "nodeId": 2,
                            "nodeType": 1,
                            "nodeName": "HTML",
                            "localName": "html",
                            "attributes": [],
                            "childNodeCount": 1,
                            "children": [
                                {
                                    "nodeId": 3,
                                    "nodeType": 1,
                                    "nodeName": "BODY",
                                    "localName": "body",
                                    "attributes": ["id", "app", "onclick", "steal()"],
                                    "childNodeCount": 2,
                                    "children": [
                                        {"nodeId": 4, "nodeType": 3, "nodeName": "#text", "localName": "", "nodeValue": "Dashboard"},
                                        {"nodeId": 5, "nodeType": 1, "nodeName": "A", "localName": "a", "attributes": ["href", "https://example.test/reset/abcdefabcdefabcdefabcdefabcdefab?token=secret"], "childNodeCount": 0}
                                    ]
                                }
                            ]
                        }
                    ]
                }
            }"##,
        )
        .expect("fixture should parse");

        let mut builder =
            DomSummaryBuilder::new(RedactionMode::Redacted, DomSummaryOptions::default());
        let summary = DomSummary::new(
            builder.document_nodes(&response.root),
            DOM_DEFAULT_DEPTH,
            false,
        );

        assert_eq!(summary.node_count, 3);
        assert_eq!(summary.nodes[0].tag, "html");
        assert_eq!(
            summary.nodes[0].children[0]
                .attributes
                .get("id")
                .map(String::as_str),
            Some("app")
        );
        assert!(
            !summary.nodes[0].children[0]
                .attributes
                .contains_key("onclick")
        );
        assert_eq!(
            summary.nodes[0].children[0].children[0]
                .attributes
                .get("href")
                .map(String::as_str),
            Some("https://example.test/reset/:redacted")
        );
    }

    #[test]
    fn read_dom_summary_uses_page_websocket_and_get_document() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");
            let message = socket
                .read()
                .expect("fixture should read command")
                .into_text()
                .expect("command should be text");
            assert!(message.contains(r#""method":"DOM.getDocument""#));
            assert!(message.contains(r#""depth":4"#));

            socket
                .send(Message::Text(
                    r##"{"id":1,"result":{"root":{"nodeId":1,"nodeType":9,"nodeName":"#document","localName":"","childNodeCount":1,"children":[{"nodeId":2,"nodeType":1,"nodeName":"HTML","localName":"html","attributes":[],"childNodeCount":0}]}}}"##
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write response");
        });

        let target = TargetInfo {
            id: "PAGE_1234567890".to_owned(),
            kind: "page".to_owned(),
            title: Some("Example".to_owned()),
            url: Some("https://example.test/path?token=secret".to_owned()),
            attached: Some(true),
            browser_context_id: None,
            web_socket_debugger_url: Some(format!("ws://{address}/devtools/page/PAGE")),
        };

        let output = read_dom_summary(
            &target,
            RedactionMode::Redacted,
            DomSummaryOptions::default(),
        )
        .expect("summary should load");

        assert_eq!(output.page.target_id, "PAGE_1234567890");
        assert_eq!(
            output.page.url_shape.as_deref(),
            Some("https://example.test/path")
        );
        assert_eq!(output.dom.node_count, 1);
        assert_eq!(output.dom.nodes[0].tag, "html");
        handle.join().expect("fixture should finish");
    }

    #[test]
    fn read_dom_summary_requires_page_websocket_url() {
        let target = TargetInfo {
            id: "PAGE_1234567890".to_owned(),
            kind: "page".to_owned(),
            title: None,
            url: None,
            attached: Some(true),
            browser_context_id: None,
            web_socket_debugger_url: None,
        };

        let error = read_dom_summary(
            &target,
            RedactionMode::Redacted,
            DomSummaryOptions::default(),
        )
        .expect_err("missing websocket should fail");

        assert_eq!(error, DomReadError::TargetWebSocketMissing);
    }
}
