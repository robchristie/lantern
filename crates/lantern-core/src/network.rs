use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use crate::{
    cdp::{
        CdpError, CdpEvent, CdpWebSocket, DEFAULT_OPERATION_TIMEOUT, EvidenceLoss,
        MAX_DRAIN_EVENTS, TargetInfo,
    },
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
    #[serde(skip_serializing_if = "crate::cdp::EvidenceLoss::is_complete")]
    pub evidence_loss: EvidenceLoss,
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

    let operation_deadline = Instant::now() + DEFAULT_OPERATION_TIMEOUT;
    let mut socket = CdpWebSocket::connect_until(web_socket_debugger_url, operation_deadline)?;
    socket.call("Network.enable", None)?;

    let mut collector = NetworkCollector::new(mode);
    let deadline = Instant::now() + NETWORK_DRAIN_TIMEOUT;

    let mut drained = 0;
    while Instant::now() < deadline && drained < MAX_DRAIN_EVENTS {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(NETWORK_READ_SLICE);
        let Some(event) = socket.read_event(timeout)? else {
            break;
        };
        drained += 1;
        collector.push_event(event);
    }

    if drained == MAX_DRAIN_EVENTS {
        socket.mark_drain_limit();
    }
    if Instant::now() >= deadline || Instant::now() >= operation_deadline {
        socket.mark_collection_deadline();
    }
    collector.record_evidence_loss(socket.evidence_loss());

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

pub(crate) struct NetworkCollector {
    mode: RedactionMode,
    next_sequence: usize,
    failed_count: usize,
    http_error_count: usize,
    truncated: bool,
    evidence_loss: EvidenceLoss,
    requests: HashMap<String, RequestMetadata>,
    request_bytes: usize,
    entries: Vec<NetworkEntry>,
}

impl NetworkCollector {
    pub(crate) fn new(mode: RedactionMode) -> Self {
        Self {
            mode,
            next_sequence: 1,
            failed_count: 0,
            http_error_count: 0,
            truncated: false,
            evidence_loss: EvidenceLoss::default(),
            requests: HashMap::new(),
            request_bytes: 0,
            entries: Vec::new(),
        }
    }

    pub(crate) fn record_evidence_loss(&mut self, loss: EvidenceLoss) {
        self.evidence_loss.dropped_events = self
            .evidence_loss
            .dropped_events
            .saturating_add(loss.dropped_events);
        self.evidence_loss.dropped_event_bytes = self
            .evidence_loss
            .dropped_event_bytes
            .saturating_add(loss.dropped_event_bytes);
        self.evidence_loss.drain_limit_reached |= loss.drain_limit_reached;
        self.evidence_loss.collection_deadline_reached |= loss.collection_deadline_reached;
    }

    pub(crate) fn push_event(&mut self, event: CdpEvent) {
        // Bound retained derived fields even with redaction disabled.
        let bytes = event.params.to_string().len() + event.method.len();
        if bytes > 256 * 1024 {
            self.evidence_loss.dropped_events = self.evidence_loss.dropped_events.saturating_add(1);
            self.evidence_loss.dropped_event_bytes = self
                .evidence_loss
                .dropped_event_bytes
                .saturating_add(bytes as u64);
            return;
        }
        match event.method.as_str() {
            "Network.requestWillBeSent" => self.push_request(event.params),
            "Network.responseReceived" => self.push_response(event.params),
            "Network.loadingFailed" => self.push_failure(event.params),
            "Network.loadingFinished" => {
                if let Some(id) = event
                    .params
                    .get("requestId")
                    .and_then(serde_json::Value::as_str)
                {
                    self.remove_request(id);
                }
            }
            _ => {}
        }
    }

    fn remove_request(&mut self, id: &str) -> Option<RequestMetadata> {
        let metadata = self.requests.remove(id)?;
        self.request_bytes -= metadata.bytes;
        Some(metadata)
    }

    fn push_request(&mut self, params: serde_json::Value) {
        let bytes = params.to_string().len();
        let Ok(event) = serde_json::from_value::<NetworkRequestWillBeSent>(params) else {
            return;
        };

        self.remove_request(&event.request_id);
        if self.requests.len() >= 1024
            || bytes > (4 * 1024 * 1024_usize).saturating_sub(self.request_bytes)
        {
            self.evidence_loss.dropped_events = self.evidence_loss.dropped_events.saturating_add(1);
            self.evidence_loss.dropped_event_bytes = self
                .evidence_loss
                .dropped_event_bytes
                .saturating_add(bytes as u64);
            return;
        }
        self.request_bytes += bytes;
        self.requests.insert(
            event.request_id,
            RequestMetadata {
                bytes,
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

        let metadata = self.remove_request(&event.request_id).unwrap_or_default();
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

    pub(crate) fn finish(self) -> NetworkSummary {
        self.finish_with_collection_gap(true, NETWORK_COLLECTION_GAP_REASON)
    }

    pub(crate) fn finish_with_collection_gap(
        self,
        collection_gap: bool,
        collection_gap_reason: &'static str,
    ) -> NetworkSummary {
        let observed_clean =
            self.entries.is_empty() && !self.truncated && !self.evidence_loss.incomplete();
        NetworkSummary {
            failed_count: self.failed_count,
            http_error_count: self.http_error_count,
            max_entries: NETWORK_MAX_ENTRIES,
            truncated: self.truncated || self.evidence_loss.incomplete(),
            evidence_loss: self.evidence_loss,
            observed_clean,
            collection_gap,
            collection_gap_reason,
            entries: self.entries,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RequestMetadata {
    bytes: usize,
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
    fn lost_transport_or_oversized_evidence_cannot_be_observed_clean() {
        let mut collector = NetworkCollector::new(RedactionMode::Redacted);
        collector.record_evidence_loss(EvidenceLoss {
            dropped_events: 1,
            dropped_event_bytes: 100,
            ..Default::default()
        });
        let summary = collector.finish();
        assert!(!summary.observed_clean);
        assert!(summary.truncated);
        assert_eq!(summary.evidence_loss.dropped_events, 1);
        let mut collector = NetworkCollector::new(RedactionMode::Unredacted);
        collector.push_event(CdpEvent {
            method: "Runtime.exceptionThrown".into(),
            params: serde_json::json!({"large":"x".repeat(256 * 1024)}),
        });
        let summary = collector.finish();
        assert!(!summary.observed_clean);
        assert!(summary.evidence_loss.dropped_event_bytes > 256 * 1024);
    }

    #[test]
    fn request_correlation_has_count_and_byte_limits_and_reclaims_completed_requests() {
        let request = |id: usize, payload: String| CdpEvent {
            method: "Network.requestWillBeSent".into(),
            params: serde_json::json!({"requestId":id.to_string(),"request":{"method":"GET","url":payload}}),
        };
        let mut collector = NetworkCollector::new(RedactionMode::Redacted);
        for id in 0..1100 {
            collector.push_event(request(id, "http://example.test".into()));
        }
        assert_eq!(collector.requests.len(), 1024);
        assert_eq!(collector.evidence_loss.dropped_events, 76);
        collector.push_event(CdpEvent {
            method: "Network.loadingFinished".into(),
            params: serde_json::json!({"requestId":"0"}),
        });
        collector.push_event(request(1101, "http://example.test".into()));
        assert_eq!(collector.requests.len(), 1024);
        assert!(collector.requests.contains_key("1101"));
        let mut collector = NetworkCollector::new(RedactionMode::Unredacted);
        for id in 0..100 {
            collector.push_event(request(
                id,
                format!("http://example.test/{}", "x".repeat(100_000)),
            ));
        }
        assert!(collector.requests.len() < 100);
        assert!(collector.request_bytes <= 4 * 1024 * 1024);
        assert!(!collector.finish().observed_clean);
    }

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
