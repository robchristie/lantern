use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    cdp::{CdpError, CdpEvent, CdpWebSocket, TargetInfo},
    redaction::{RedactionMode, TITLE_TEXT_LIMIT, sanitize_text, sanitize_title, sanitize_url},
};

pub const NETWORK_SCHEMA_VERSION: u8 = 1;
pub const NETWORK_MAX_ENTRIES: usize = 20;
pub const NETWORK_COLLECTION_GAP_REASON: &str = "events_before_network_enable_not_observed";
const NETWORK_DRAIN_TIMEOUT: Duration = Duration::from_millis(300);
const NETWORK_READ_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkCommandOutput {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub page: NetworkPageSummary,
    pub network: NetworkSummary,
}

impl NetworkCommandOutput {
    pub fn success(page: NetworkPageSummary, network: NetworkSummary) -> Self {
        Self {
            schema_version: NETWORK_SCHEMA_VERSION,
            command: "network",
            ok: true,
            page,
            network,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkPageSummary {
    pub target_id: String,
    pub title: Option<String>,
    pub url_shape: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkSummary {
    pub failed_count: usize,
    pub http_error_count: usize,
    pub max_entries: usize,
    pub truncated: bool,
    pub observed_clean: bool,
    pub collection_gap: bool,
    pub collection_gap_reason: &'static str,
    pub entries: Vec<NetworkEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkEntry {
    pub sequence: usize,
    pub kind: NetworkEntryKind,
    pub request_id: String,
    pub method: Option<String>,
    pub url_shape: Option<String>,
    pub resource_type: Option<String>,
    pub status: Option<u16>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEntryKind {
    FailedRequest,
    HttpErrorResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkReadError {
    TargetWebSocketMissing,
    Cdp(CdpError),
}

impl From<CdpError> for NetworkReadError {
    fn from(error: CdpError) -> Self {
        Self::Cdp(error)
    }
}

pub fn read_network_failures(
    target: &TargetInfo,
    mode: RedactionMode,
) -> Result<NetworkCommandOutput, NetworkReadError> {
    let web_socket_debugger_url = target
        .web_socket_debugger_url
        .as_deref()
        .ok_or(NetworkReadError::TargetWebSocketMissing)?;

    let mut socket = CdpWebSocket::connect(web_socket_debugger_url)?;
    socket.call("Network.enable", None)?;

    let mut collector = NetworkCollector::new(mode);
    let deadline = Instant::now() + NETWORK_DRAIN_TIMEOUT;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(NETWORK_READ_SLICE);
        let Some(event) = socket.read_event(timeout)? else {
            break;
        };
        collector.push_event(event);
    }

    let page = NetworkPageSummary {
        target_id: target.id.clone(),
        title: target
            .title
            .as_ref()
            .map(|title| sanitize_title(title, mode)),
        url_shape: target.url.as_ref().and_then(|url| sanitize_url(url, mode)),
    };

    Ok(NetworkCommandOutput::success(page, collector.finish()))
}

struct NetworkCollector {
    mode: RedactionMode,
    next_sequence: usize,
    failed_count: usize,
    http_error_count: usize,
    truncated: bool,
    requests: HashMap<String, RequestMetadata>,
    entries: Vec<NetworkEntry>,
}

impl NetworkCollector {
    fn new(mode: RedactionMode) -> Self {
        Self {
            mode,
            next_sequence: 1,
            failed_count: 0,
            http_error_count: 0,
            truncated: false,
            requests: HashMap::new(),
            entries: Vec::new(),
        }
    }

    fn push_event(&mut self, event: CdpEvent) {
        match event.method.as_str() {
            "Network.requestWillBeSent" => self.push_request(event.params),
            "Network.responseReceived" => self.push_response(event.params),
            "Network.loadingFailed" => self.push_failure(event.params),
            _ => {}
        }
    }

    fn push_request(&mut self, params: serde_json::Value) {
        let Ok(event) = serde_json::from_value::<NetworkRequestWillBeSent>(params) else {
            return;
        };

        self.requests.insert(
            event.request_id,
            RequestMetadata {
                method: Some(sanitize_short_label(&event.request.method, self.mode)),
                url_shape: sanitize_url(&event.request.url, self.mode),
            },
        );
    }

    fn push_response(&mut self, params: serde_json::Value) {
        let Ok(event) = serde_json::from_value::<NetworkResponseReceived>(params) else {
            return;
        };
        if event.response.status < 400 {
            return;
        }

        let metadata = self
            .requests
            .get(&event.request_id)
            .cloned()
            .unwrap_or_default();
        let status = if event.response.status <= u16::MAX as u64 {
            Some(event.response.status as u16)
        } else {
            None
        };
        let entry = NetworkEntry {
            sequence: 0,
            kind: NetworkEntryKind::HttpErrorResponse,
            request_id: event.request_id,
            method: metadata.method,
            url_shape: metadata
                .url_shape
                .or_else(|| sanitize_url(&event.response.url, self.mode)),
            resource_type: event
                .kind
                .map(|kind| sanitize_short_label(&kind, self.mode)),
            status,
            failure_reason: None,
        };
        self.push_entry(entry);
    }

    fn push_failure(&mut self, params: serde_json::Value) {
        let Ok(event) = serde_json::from_value::<NetworkLoadingFailed>(params) else {
            return;
        };

        let metadata = self
            .requests
            .get(&event.request_id)
            .cloned()
            .unwrap_or_default();
        let entry = NetworkEntry {
            sequence: 0,
            kind: NetworkEntryKind::FailedRequest,
            request_id: event.request_id,
            method: metadata.method,
            url_shape: metadata.url_shape,
            resource_type: event
                .kind
                .map(|kind| sanitize_short_label(&kind, self.mode)),
            status: None,
            failure_reason: Some(sanitize_short_label(&event.error_text, self.mode)),
        };
        self.push_entry(entry);
    }

    fn push_entry(&mut self, mut entry: NetworkEntry) {
        if self.entries.len() >= NETWORK_MAX_ENTRIES {
            self.truncated = true;
            return;
        }

        entry.sequence = self.next_sequence;
        self.next_sequence += 1;
        match entry.kind {
            NetworkEntryKind::FailedRequest => self.failed_count += 1,
            NetworkEntryKind::HttpErrorResponse => self.http_error_count += 1,
        }
        self.entries.push(entry);
    }

    fn finish(self) -> NetworkSummary {
        let observed_clean = self.entries.is_empty();
        NetworkSummary {
            failed_count: self.failed_count,
            http_error_count: self.http_error_count,
            max_entries: NETWORK_MAX_ENTRIES,
            truncated: self.truncated,
            observed_clean,
            collection_gap: true,
            collection_gap_reason: NETWORK_COLLECTION_GAP_REASON,
            entries: self.entries,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RequestMetadata {
    method: Option<String>,
    url_shape: Option<String>,
}

fn sanitize_short_label(value: &str, _mode: RedactionMode) -> String {
    sanitize_text(value, TITLE_TEXT_LIMIT, RedactionMode::Redacted)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkRequestWillBeSent {
    request_id: String,
    request: NetworkRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkRequest {
    url: String,
    method: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkResponseReceived {
    request_id: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    response: NetworkResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkResponse {
    url: String,
    status: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkLoadingFailed {
    request_id: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    error_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collects_failed_and_http_error_entries_with_request_metadata() {
        let mut collector = NetworkCollector::new(RedactionMode::Redacted);

        collector.push_event(CdpEvent {
            method: "Network.requestWillBeSent".to_owned(),
            params: json!({
                "requestId": "REQ_1",
                "request": {
                    "method": "GET",
                    "url": "https://user:pass@example.test/reset/abcdef123456abcdef123456abcdef12?token=secret#frag"
                }
            }),
        });
        collector.push_event(CdpEvent {
            method: "Network.loadingFailed".to_owned(),
            params: json!({
                "requestId": "REQ_1",
                "type": "Fetch",
                "errorText": "net::ERR_FAILED"
            }),
        });
        collector.push_event(CdpEvent {
            method: "Network.requestWillBeSent".to_owned(),
            params: json!({
                "requestId": "REQ_2",
                "request": {
                    "method": "POST",
                    "url": "https://example.test/api/save?token=secret"
                }
            }),
        });
        collector.push_event(CdpEvent {
            method: "Network.responseReceived".to_owned(),
            params: json!({
                "requestId": "REQ_2",
                "type": "XHR",
                "response": {
                    "url": "https://example.test/api/save?token=secret",
                    "status": 500
                }
            }),
        });

        let summary = collector.finish();

        assert_eq!(summary.failed_count, 1);
        assert_eq!(summary.http_error_count, 1);
        assert!(!summary.observed_clean);
        assert!(summary.collection_gap);
        assert_eq!(
            summary.entries[0].url_shape.as_deref(),
            Some("https://example.test/reset/:redacted")
        );
        assert_eq!(
            summary.entries[0].failure_reason.as_deref(),
            Some("net::ERR_FAILED")
        );
        assert_eq!(summary.entries[1].status, Some(500));
    }

    #[test]
    fn clean_window_still_reports_collection_gap() {
        let summary = NetworkCollector::new(RedactionMode::Redacted).finish();

        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.http_error_count, 0);
        assert!(summary.observed_clean);
        assert!(summary.collection_gap);
        assert_eq!(
            summary.collection_gap_reason,
            "events_before_network_enable_not_observed"
        );
    }
}
