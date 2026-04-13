use std::collections::BTreeMap;

use serde::Serialize;

pub const DOM_SCHEMA_VERSION: u8 = 1;
pub const DOM_MAX_DEPTH: usize = 4;
pub const DOM_MAX_NODES: usize = 80;

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

fn count_nodes(nodes: &[DomNodeSummary]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_nodes(&node.children))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let summary = DomSummary::new(vec![root], DOM_MAX_DEPTH, false);

        assert_eq!(summary.node_count, 2);
    }
}
