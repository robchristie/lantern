use std::{
    collections::VecDeque,
    io::{self, ErrorKind, Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tungstenite::{Message, WebSocket, client::client_with_config, protocol::WebSocketConfig};
use url::{Host, Url};

use crate::endpoint::ResolvedEndpoint;

pub const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_PENDING_EVENTS: usize = 1024;
pub const MAX_PENDING_EVENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_HTTP_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DRAIN_EVENTS: usize = 1024;
pub const MAX_DRAIN_TIME: Duration = Duration::from_millis(20);
pub const CLEANUP_TIMEOUT: Duration = Duration::from_millis(100);

/// One budget shared by target selection, attachment, commands and finalisation.
#[derive(Debug, Clone, Copy)]
pub struct OperationDeadline {
    pub started: Instant,
    pub timeout: Duration,
}

impl OperationDeadline {
    pub fn end(self) -> Instant {
        self.started + self.timeout
    }
}

impl From<Duration> for OperationDeadline {
    fn from(timeout: Duration) -> Self {
        Self {
            started: Instant::now(),
            timeout,
        }
    }
}

/// Loss counters describe transport and collector limits, without retaining event data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EvidenceLoss {
    pub dropped_events: u64,
    pub dropped_event_bytes: u64,
    pub drain_limit_reached: bool,
    pub collection_deadline_reached: bool,
}

impl EvidenceLoss {
    pub fn is_complete(&self) -> bool {
        !self.incomplete()
    }

    pub fn incomplete(&self) -> bool {
        self.dropped_events > 0 || self.drain_limit_reached || self.collection_deadline_reached
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(ErrorKind::TimedOut, "CDP operation deadline exceeded"))
}

fn transport_error(source: impl ToString) -> CdpError {
    CdpError::WebSocketTransport {
        context: "CDP transport failed; a sent command may have executed; inspect state before retrying",
        source: source.to_string(),
    }
}

// Track only wire boundaries, never payload contents. Tungstenite can read a
// complete event and part of the next frame in one TCP read. Its private input
// buffer otherwise makes a later timeout indistinguishable from silence.
#[derive(Debug, Default)]
struct IncomingProgress {
    handshake_tail: u32,
    handshake_complete: bool,
    header: [u8; 14],
    header_len: usize,
    payload_remaining: u64,
    fragmented: bool,
}

impl IncomingProgress {
    fn incomplete(&self) -> bool {
        self.header_len != 0 || self.payload_remaining != 0 || self.fragmented
    }

    fn observe(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if !self.handshake_complete {
                self.handshake_tail = (self.handshake_tail << 8) | u32::from(bytes[0]);
                self.handshake_complete = self.handshake_tail == 0x0d0a0d0a;
                bytes = &bytes[1..];
                continue;
            }
            if self.payload_remaining > 0 {
                let consumed = self.payload_remaining.min(bytes.len() as u64) as usize;
                self.payload_remaining -= consumed as u64;
                bytes = &bytes[consumed..];
                continue;
            }
            self.header[self.header_len] = bytes[0];
            self.header_len += 1;
            bytes = &bytes[1..];
            if self.header_len < 2 {
                continue;
            }
            let short_len = self.header[1] & 0x7f;
            let extended = match short_len {
                126 => 2,
                127 => 8,
                _ => 0,
            };
            let masked = if self.header[1] & 0x80 != 0 { 4 } else { 0 };
            if self.header_len < 2 + extended + masked {
                continue;
            }
            self.payload_remaining = match short_len {
                126 => u64::from(u16::from_be_bytes([self.header[2], self.header[3]])),
                127 => {
                    u64::from_be_bytes(self.header[2..10].try_into().expect("eight length bytes"))
                }
                length => u64::from(length),
            };
            let opcode = self.header[0] & 0x0f;
            let fin = self.header[0] & 0x80 != 0;
            if matches!(opcode, 0..=2) {
                self.fragmented = !fin;
            }
            self.header_len = 0;
        }
    }
}

// Check the deadline on every underlying I/O, including fragmented frames and
// handshake traffic. A socket inactivity timeout alone permits endless trickles.
#[derive(Debug)]
struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
    incoming: IncomingProgress,
}
impl Read for DeadlineStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        let bytes = self.stream.read(buf)?;
        self.incoming.observe(&buf[..bytes]);
        Ok(bytes)
    }
}
impl Write for DeadlineStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        remaining(self.deadline)?;
        self.stream.flush()
    }
}

#[derive(Debug, Clone)]
pub struct CdpClient {
    endpoint: ResolvedEndpoint,
    http: ureq::Agent,
    deadline: Option<Instant>,
}

impl CdpClient {
    pub fn new(endpoint: ResolvedEndpoint) -> Self {
        Self {
            endpoint,
            deadline: None,
            http: ureq::AgentBuilder::new()
                .redirects(0)
                .resolver(|address: &str| {
                    if let Some(port) = address.strip_prefix("localhost:") {
                        let port = port
                            .parse::<u16>()
                            .map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))?;
                        return Ok(vec![
                            SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), port),
                            SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port),
                        ]);
                    }
                    address
                        .parse::<SocketAddr>()
                        .map(|a| vec![a])
                        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))
                })
                .build(),
        }
    }

    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn browser_version(&self) -> Result<BrowserVersion, CdpError> {
        self.get_json("json/version")
    }

    pub fn targets(&self) -> Result<Vec<TargetInfo>, CdpError> {
        self.get_json("json/list")
    }

    pub fn close_browser(&self) -> Result<(), CdpError> {
        let deadline = self
            .deadline
            .unwrap_or_else(|| Instant::now() + DEFAULT_OPERATION_TIMEOUT);
        let version = self.clone().with_deadline(deadline).browser_version()?;
        let web_socket_debugger_url =
            version
                .web_socket_debugger_url
                .ok_or_else(|| CdpError::ResponseInvalid {
                    context: "CDP browser version omitted its browser WebSocket URL",
                    source: "webSocketDebuggerUrl is missing".to_owned(),
                })?;
        let mut socket = CdpWebSocket::connect_until(&web_socket_debugger_url, deadline)?;
        socket.call("Browser.close", None)?;
        Ok(())
    }

    fn get_json<T>(&self, path: &str) -> Result<T, CdpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = endpoint_child_url(&self.endpoint, path)?;
        let deadline = self
            .deadline
            .unwrap_or_else(|| Instant::now() + DEFAULT_OPERATION_TIMEOUT);
        let response = self
            .http
            .get(url.as_str())
            .timeout(remaining(deadline).map_err(transport_error)?)
            .call()
            .map_err(CdpError::from)?;
        let mut body = String::new();
        response
            .into_reader()
            .take((MAX_HTTP_BYTES + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|source| CdpError::ResponseInvalid {
                context: "failed to read CDP HTTP response body",
                source: source.to_string(),
            })?;

        if body.len() > MAX_HTTP_BYTES {
            return Err(CdpError::ResponseInvalid {
                context: "CDP HTTP response exceeded byte limit",
                source: MAX_HTTP_BYTES.to_string(),
            });
        }
        remaining(deadline).map_err(transport_error)?;
        let parsed = serde_json::from_str(&body).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP HTTP JSON response",
            source: source.to_string(),
        })?;
        remaining(deadline).map_err(transport_error)?;
        Ok(parsed)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: Option<String>,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: Option<String>,
    #[serde(rename = "User-Agent")]
    pub user_agent: Option<String>,
    #[serde(rename = "V8-Version")]
    pub v8_version: Option<String>,
    #[serde(rename = "WebKit-Version")]
    pub webkit_version: Option<String>,
    pub web_socket_debugger_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub attached: Option<bool>,
    pub browser_context_id: Option<String>,
    pub web_socket_debugger_url: Option<String>,
}

pub struct CdpWebSocket {
    socket: WebSocket<DeadlineStream>,
    deadline: Option<Instant>,
    next_id: u64,
    pending_events: VecDeque<(CdpEvent, usize)>,
    pending_bytes: usize,
    evidence_loss: EvidenceLoss,
}

impl CdpWebSocket {
    pub fn connect(url: &str) -> Result<Self, CdpError> {
        let mut socket = Self::connect_until(url, Instant::now() + DEFAULT_OPERATION_TIMEOUT)?;
        socket.deadline = None;
        Ok(socket)
    }

    pub fn connect_until(url: &str, deadline: Instant) -> Result<Self, CdpError> {
        validate_websocket_url(url)?;
        let parsed = Url::parse(url).map_err(transport_error)?;
        // Only loopback literals and localhost are accepted; avoid unbounded DNS.
        let ips = match parsed.host().expect("validated host") {
            Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
            Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
            Host::Domain(_) => vec![
                IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
                IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            ],
        };
        let mut connected = Err(io::Error::new(ErrorKind::NotConnected, "no local address"));
        for ip in ips {
            connected = TcpStream::connect_timeout(
                &SocketAddr::new(ip, parsed.port().expect("validated port")),
                remaining(deadline).map_err(transport_error)?,
            );
            if connected.is_ok() {
                break;
            }
        }
        let stream = connected.map_err(transport_error)?;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (socket, _) = client_with_config(
            url,
            DeadlineStream {
                stream,
                deadline,
                incoming: IncomingProgress::default(),
            },
            Some(config),
        )
        .map_err(|e| transport_error(format!("{e:?}")))?;
        remaining(deadline).map_err(transport_error)?;
        Ok(Self {
            socket,
            deadline: Some(deadline),
            next_id: 1,
            pending_events: VecDeque::new(),
            pending_bytes: 0,
            evidence_loss: EvidenceLoss::default(),
        })
    }

    pub(crate) fn pop_pending_event(&mut self) -> Option<CdpEvent> {
        let (event, bytes) = self.pending_events.pop_front()?;
        self.pending_bytes -= bytes;
        Some(event)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn evidence_loss(&self) -> EvidenceLoss {
        self.evidence_loss.clone()
    }
    pub fn mark_drain_limit(&mut self) {
        self.evidence_loss.drain_limit_reached = true;
    }
    pub fn mark_collection_deadline(&mut self) {
        self.evidence_loss.collection_deadline_reached = true;
    }

    /// Permit one bounded best-effort release after an uncertain input command.
    pub(crate) fn begin_cleanup(&mut self) {
        self.deadline = Some(Instant::now() + CLEANUP_TIMEOUT);
    }

    pub fn call(&mut self, method: &str, params: Option<Value>) -> Result<Value, CdpError> {
        let deadline = self
            .deadline
            .unwrap_or_else(|| Instant::now() + DEFAULT_OPERATION_TIMEOUT);
        self.socket.get_mut().deadline = deadline;
        remaining(deadline).map_err(transport_error)?;
        let id = self.next_id;
        self.next_id += 1;
        let request = serde_json::to_string(&CdpCommandRequest { id, method, params })
            .map_err(transport_error)?;
        self.socket
            .send(Message::Text(request.into()))
            .map_err(|source| CdpError::CommandUncertain {
                source: source.to_string(),
            })?;
        loop {
            remaining(deadline).map_err(|source| CdpError::CommandUncertain {
                source: source.to_string(),
            })?;
            let message = self
                .socket
                .read()
                .map_err(|source| CdpError::CommandUncertain {
                    source: source.to_string(),
                })?;
            remaining(deadline).map_err(|source| CdpError::CommandUncertain {
                source: source.to_string(),
            })?;
            let Message::Text(text) = message else {
                continue;
            };
            let response: CdpCommandResponse =
                serde_json::from_str(&text).map_err(|source| CdpError::CommandUncertain {
                    source: source.to_string(),
                })?;
            remaining(deadline).map_err(|source| CdpError::CommandUncertain {
                source: source.to_string(),
            })?;
            if response.id != Some(id) {
                self.buffer_event_from_text(&text)?;
                continue;
            }
            if let Some(error) = response.error {
                return Err(CdpError::Command {
                    code: error.code,
                    message: error.message,
                });
            }
            return Ok(response.result.unwrap_or(Value::Null));
        }
    }

    pub fn read_event(&mut self, timeout: Duration) -> Result<Option<CdpEvent>, CdpError> {
        let slice = Instant::now() + timeout;
        let deadline = self.deadline.map_or(slice, |d| d.min(slice));
        if remaining(deadline).is_err() {
            return Ok(None);
        }
        if let Some((event, bytes)) = self.pending_events.pop_front() {
            self.pending_bytes -= bytes;
            return Ok(Some(event));
        }
        self.socket.get_mut().deadline = deadline;
        loop {
            if remaining(deadline).is_err() {
                return Ok(None);
            }
            let message = match self.socket.read() {
                Ok(message) => message,
                Err(tungstenite::Error::Io(source))
                    if matches!(source.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    if self.socket.get_ref().incoming.incomplete() {
                        // A read slice ended with a frame still incomplete. Silence
                        // cannot establish a complete observation in this window.
                        self.mark_collection_deadline();
                    }
                    return Ok(None);
                }
                Err(source) => return Err(transport_error(source)),
            };
            let Message::Text(text) = message else {
                continue;
            };
            if let Some(event) = Self::event_from_text(&text)? {
                if remaining(deadline).is_err() {
                    self.evidence_loss.dropped_events =
                        self.evidence_loss.dropped_events.saturating_add(1);
                    self.evidence_loss.dropped_event_bytes = self
                        .evidence_loss
                        .dropped_event_bytes
                        .saturating_add(text.len() as u64);
                    return Ok(None);
                }
                return Ok(Some(event));
            }
        }
    }

    fn event_from_text(text: &str) -> Result<Option<CdpEvent>, CdpError> {
        let inbound: CdpInboundMessage = serde_json::from_str(text).map_err(transport_error)?;
        if inbound.id.is_some() {
            return Ok(None);
        }
        Ok(inbound.method.map(|method| CdpEvent {
            method,
            params: inbound.params.unwrap_or(Value::Null),
        }))
    }

    fn buffer_event_from_text(&mut self, text: &str) -> Result<(), CdpError> {
        let Some(event) = Self::event_from_text(text)? else {
            return Ok(());
        };
        if self.pending_events.len() >= MAX_PENDING_EVENTS
            || text.len() > MAX_PENDING_EVENT_BYTES.saturating_sub(self.pending_bytes)
        {
            self.evidence_loss.dropped_events = self.evidence_loss.dropped_events.saturating_add(1);
            self.evidence_loss.dropped_event_bytes = self
                .evidence_loss
                .dropped_event_bytes
                .saturating_add(text.len() as u64);
        } else {
            self.pending_bytes += text.len();
            self.pending_events.push_back((event, text.len()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize)]
struct CdpCommandRequest<'a> {
    id: u64,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CdpCommandResponse {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<CdpCommandError>,
}

#[derive(Debug, Deserialize)]
struct CdpInboundMessage {
    id: Option<u64>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CdpCommandError {
    code: i64,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdpError {
    EndpointUrlInvalid {
        source: String,
    },
    HttpStatus {
        status: u16,
    },
    Unreachable {
        source: String,
    },
    ResponseInvalid {
        context: &'static str,
        source: String,
    },
    WebSocketUrlInvalid {
        source: String,
    },
    WebSocketTransport {
        context: &'static str,
        source: String,
    },
    CommandUncertain {
        source: String,
    },
    Command {
        code: i64,
        message: String,
    },
}

impl From<ureq::Error> for CdpError {
    fn from(error: ureq::Error) -> Self {
        match error {
            ureq::Error::Status(status, _) => Self::HttpStatus { status },
            ureq::Error::Transport(source) => Self::Unreachable {
                source: source.to_string(),
            },
        }
    }
}

fn endpoint_child_url(endpoint: &ResolvedEndpoint, child_path: &str) -> Result<Url, CdpError> {
    let mut url = Url::parse(&endpoint.display).map_err(|source| CdpError::EndpointUrlInvalid {
        source: source.to_string(),
    })?;

    let base_path = url.path().trim_end_matches('/');
    let child_path = child_path.trim_start_matches('/');
    let joined_path = if base_path.is_empty() {
        format!("/{child_path}")
    } else {
        format!("{base_path}/{child_path}")
    };

    url.set_path(&joined_path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn validate_websocket_url(web_socket_debugger_url: &str) -> Result<(), CdpError> {
    let url =
        Url::parse(web_socket_debugger_url).map_err(|source| CdpError::WebSocketUrlInvalid {
            source: source.to_string(),
        })?;

    if url.scheme() != "ws" {
        return Err(CdpError::WebSocketUrlInvalid {
            source: "only local ws:// CDP debugger URLs are supported".to_owned(),
        });
    }

    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CdpError::WebSocketUrlInvalid {
            source: "credentials, query strings, and fragments are not supported".to_owned(),
        });
    }

    let Some(host) = url.host() else {
        return Err(CdpError::WebSocketUrlInvalid {
            source: "missing host".to_owned(),
        });
    };

    if !is_local_host(&host) {
        return Err(CdpError::WebSocketUrlInvalid {
            source: "non-local host".to_owned(),
        });
    }

    if url.port().is_none() {
        return Err(CdpError::WebSocketUrlInvalid {
            source: "missing port".to_owned(),
        });
    }

    Ok(())
}

fn is_local_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(addr) => IpAddr::V4(*addr).is_loopback(),
        Host::Ipv6(addr) => IpAddr::V6(*addr).is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::{EndpointSource, ResolvedEndpoint};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn adversarial_socket(
        serve: impl FnOnce(WebSocket<TcpStream>) + Send + 'static,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            serve(tungstenite::accept(stream).unwrap());
        });
        (format!("ws://{address}/devtools/page/test"), handle)
    }

    #[test]
    fn localhost_supports_ipv6_only_http_and_websocket_without_dns() {
        let Ok(listener) = TcpListener::bind("[::1]:0") else {
            return;
        };
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]")
                .unwrap();
            drop(stream);
            let (stream, _) = listener.accept().unwrap();
            let mut server = tungstenite::accept(stream).unwrap();
            server.read().unwrap();
            server
                .send(Message::Text(r#"{"id":1,"result":{}}"#.into()))
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        let endpoint = ResolvedEndpoint {
            source: EndpointSource::Flag,
            display: format!("http://localhost:{port}"),
        };
        assert!(
            CdpClient::new(endpoint)
                .with_deadline(deadline)
                .targets()
                .unwrap()
                .is_empty()
        );
        CdpWebSocket::connect_until(&format!("ws://localhost:{port}/test"), deadline)
            .unwrap()
            .call("Runtime.enable", None)
            .unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn continuous_events_cannot_extend_command_deadline_or_queue_count() {
        let (url, handle) = adversarial_socket(|mut server| {
            server.read().unwrap();
            while server
                .send(Message::Text(
                    r#"{"method":"Runtime.consoleAPICalled","params":{}}"#.into(),
                ))
                .is_ok()
            {}
        });
        let started = Instant::now();
        let mut client =
            CdpWebSocket::connect_until(&url, started + Duration::from_millis(150)).unwrap();
        assert!(matches!(
            client.call("Input.dispatchMouseEvent", None),
            Err(CdpError::CommandUncertain { .. })
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(client.pending_events.len() <= MAX_PENDING_EVENTS);
        assert!(client.pending_bytes <= MAX_PENDING_EVENT_BYTES);
        assert!(client.evidence_loss().dropped_events > 0);
        drop(client);
        handle.join().unwrap();
    }

    #[test]
    fn queued_event_bytes_are_bounded_and_loss_is_counted() {
        let event = serde_json::json!({"method":"Runtime.consoleAPICalled","params":{"payload":"x".repeat(600_000)}}).to_string();
        let bytes = event.len();
        let (url, handle) = adversarial_socket(move |mut server| {
            server.read().unwrap();
            for _ in 0..10 {
                server.send(Message::Text(event.clone().into())).unwrap();
            }
            server
                .send(Message::Text(r#"{"id":1,"result":{}}"#.into()))
                .unwrap();
        });
        let mut client = CdpWebSocket::connect(&url).unwrap();
        client.call("Runtime.enable", None).unwrap();
        assert_eq!(client.pending_events.len(), MAX_PENDING_EVENT_BYTES / bytes);
        assert_eq!(
            client.evidence_loss().dropped_events,
            (10 - MAX_PENDING_EVENT_BYTES / bytes) as u64
        );
        assert_eq!(
            client.evidence_loss().dropped_event_bytes,
            client.evidence_loss().dropped_events * bytes as u64
        );
        while client.pop_pending_event().is_some() {}
        assert_eq!(client.pending_bytes, 0);
        handle.join().unwrap();
    }

    #[test]
    fn irrelevant_responses_cannot_extend_event_read_slice() {
        let (url, handle) = adversarial_socket(|mut server| {
            while server
                .send(Message::Text(r#"{"id":900,"result":{}}"#.into()))
                .is_ok()
            {}
        });
        let mut client = CdpWebSocket::connect(&url).unwrap();
        let started = Instant::now();
        assert!(
            client
                .read_event(Duration::from_millis(70))
                .unwrap()
                .is_none()
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        drop(client);
        handle.join().unwrap();
    }

    #[test]
    fn read_ahead_partial_frame_is_not_mistaken_for_silence() {
        let (url, handle) = adversarial_socket(|mut server| {
            let event = br#"{"method":"Runtime.ready"}"#;
            let mut wire = vec![0x81, event.len() as u8];
            wire.extend_from_slice(event);
            wire.extend_from_slice(&[0x81, 126, 4, 0, b'x']);
            server.get_mut().write_all(&wire).unwrap();
            thread::sleep(Duration::from_millis(150));
        });
        let mut client = CdpWebSocket::connect(&url).unwrap();
        assert!(
            client
                .read_event(Duration::from_millis(50))
                .unwrap()
                .is_some()
        );
        assert!(
            client
                .read_event(Duration::from_millis(30))
                .unwrap()
                .is_none()
        );
        assert!(client.evidence_loss().collection_deadline_reached);
        handle.join().unwrap();
    }

    #[test]
    fn wire_progress_tracks_fragmented_messages_and_control_frames() {
        let mut progress = IncomingProgress::default();
        progress.observe(b"HTTP/1.1 101 Switching Protocols\r\n\r\n");
        progress.observe(&[0x01, 1, b'a']);
        assert!(progress.incomplete());
        progress.observe(&[0x89, 0]);
        assert!(progress.incomplete());
        progress.observe(&[0x80, 126, 0, 1]);
        assert!(progress.incomplete());
        progress.observe(b"b");
        assert!(!progress.incomplete());
    }

    #[test]
    fn event_slice_with_partial_frame_does_not_claim_complete_evidence() {
        let (url, handle) = adversarial_socket(|mut server| {
            let stream = server.get_mut();
            stream.write_all(&[0x81, 126, 4, 0]).unwrap();
            while stream.write_all(b"x").is_ok() {
                thread::sleep(Duration::from_millis(5));
            }
        });
        let mut client = CdpWebSocket::connect(&url).unwrap();
        assert!(
            client
                .read_event(Duration::from_millis(50))
                .unwrap()
                .is_none()
        );
        assert!(client.evidence_loss().collection_deadline_reached);
        drop(client);
        handle.join().unwrap();
    }

    #[test]
    fn partial_frame_trickle_cannot_extend_command_deadline() {
        let (url, handle) = adversarial_socket(|mut server| {
            server.read().unwrap();
            let stream = server.get_mut();
            stream.write_all(&[0x81, 126, 4, 0]).unwrap();
            while stream.write_all(b"x").is_ok() {
                thread::sleep(Duration::from_millis(5));
            }
        });
        let started = Instant::now();
        let mut client =
            CdpWebSocket::connect_until(&url, started + Duration::from_millis(80)).unwrap();
        assert!(matches!(
            client.call("Runtime.enable", None),
            Err(CdpError::CommandUncertain { .. })
        ));
        assert!(started.elapsed() < Duration::from_millis(500));
        drop(client);
        handle.join().unwrap();
    }

    #[test]
    fn oversized_frame_fails_without_reading_declared_payload() {
        let (url, handle) = adversarial_socket(|mut server| {
            server.read().unwrap();
            let mut header = vec![0x81, 127];
            header.extend_from_slice(&((MAX_MESSAGE_BYTES + 1) as u64).to_be_bytes());
            server.get_mut().write_all(&header).unwrap();
        });
        let mut client = CdpWebSocket::connect(&url).unwrap();
        assert!(matches!(
            client.call("Runtime.enable", None),
            Err(CdpError::CommandUncertain { .. })
        ));
        handle.join().unwrap();
    }

    #[test]
    fn handshake_trickle_uses_attachment_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\nX-Trickle: ")
                .unwrap();
            while stream.write_all(b"x").is_ok() {
                thread::sleep(Duration::from_millis(5));
            }
        });
        let started = Instant::now();
        assert!(
            CdpWebSocket::connect_until(
                &format!("ws://{address}/test"),
                started + Duration::from_millis(80)
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        handle.join().unwrap();
    }

    #[test]
    fn http_trickles_and_oversized_bodies_are_bounded() {
        for headers in [true, false] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0; 2048];
                stream.read(&mut buffer).unwrap();
                let prefix: &[u8] = if headers {
                    b"HTTP/1.1 200 OK\r\nX-Trickle: "
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 99999\r\n\r\n"
                };
                stream.write_all(prefix).unwrap();
                while stream.write_all(b"x").is_ok() {
                    thread::sleep(Duration::from_millis(5));
                }
            });
            let started = Instant::now();
            let endpoint = ResolvedEndpoint {
                source: EndpointSource::Flag,
                display: format!("http://{address}"),
            };
            assert!(
                CdpClient::new(endpoint)
                    .with_deadline(started + Duration::from_millis(80))
                    .targets()
                    .is_err()
            );
            assert!(started.elapsed() < Duration::from_millis(500));
            handle.join().unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            stream.read(&mut buffer).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                MAX_HTTP_BYTES + 1
            )
            .unwrap();
            let _ = stream.write_all(&vec![b'x'; MAX_HTTP_BYTES + 1]);
        });
        let endpoint = ResolvedEndpoint {
            source: EndpointSource::Flag,
            display: format!("http://{address}"),
        };
        assert!(matches!(
            CdpClient::new(endpoint).targets(),
            Err(CdpError::ResponseInvalid {
                context: "CDP HTTP response exceeded byte limit",
                ..
            })
        ));
        handle.join().unwrap();
    }

    #[test]
    fn builds_cdp_paths_relative_to_endpoint_prefix() {
        let endpoint = ResolvedEndpoint {
            source: EndpointSource::Flag,
            display: "http://127.0.0.1:9222/devtools".to_owned(),
        };

        let url = endpoint_child_url(&endpoint, "json/version").expect("url should build");

        assert_eq!(url.as_str(), "http://127.0.0.1:9222/devtools/json/version");
    }

    #[test]
    fn parses_browser_version_response_with_chromium_field_names() {
        let version: BrowserVersion = serde_json::from_str(
            r#"{
                "Browser": "Chrome/123.0.0.0",
                "Protocol-Version": "1.3",
                "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc"
            }"#,
        )
        .expect("version JSON should parse");

        assert_eq!(version.browser.as_deref(), Some("Chrome/123.0.0.0"));
        assert_eq!(version.protocol_version.as_deref(), Some("1.3"));
        assert_eq!(
            version.web_socket_debugger_url.as_deref(),
            Some("ws://127.0.0.1:9222/devtools/browser/abc")
        );
    }

    #[test]
    fn parses_target_list_response_with_optional_metadata() {
        let targets: Vec<TargetInfo> = serde_json::from_str(
            r#"[
                {
                    "id": "ABCDEF123456",
                    "type": "page",
                    "title": "Example",
                    "url": "https://example.test/path?token=secret",
                    "attached": true,
                    "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/ABCDEF123456"
                }
            ]"#,
        )
        .expect("target JSON should parse");

        assert_eq!(targets[0].id, "ABCDEF123456");
        assert_eq!(targets[0].kind, "page");
        assert_eq!(targets[0].attached, Some(true));
    }

    #[test]
    fn rejects_non_local_websocket_debugger_urls() {
        let error =
            validate_websocket_url("ws://example.com:9222/devtools/page/ABC").expect_err("remote");

        assert!(matches!(error, CdpError::WebSocketUrlInvalid { .. }));
    }

    #[test]
    fn http_client_reads_browser_version_from_endpoint_prefix() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("fixture should accept");
            let mut buffer = [0_u8; 1024];
            let bytes = stream.read(&mut buffer).expect("fixture should read");
            let request = String::from_utf8_lossy(&buffer[..bytes]);
            assert!(request.starts_with("GET /devtools/json/version HTTP/1.1"));

            let body = r#"{"Browser":"Chrome/123.0.0.0","Protocol-Version":"1.3"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("fixture should write");
        });

        let endpoint = ResolvedEndpoint {
            source: EndpointSource::Flag,
            display: format!("http://{address}/devtools"),
        };
        let version = CdpClient::new(endpoint)
            .browser_version()
            .expect("version should load");

        assert_eq!(version.browser.as_deref(), Some("Chrome/123.0.0.0"));
        assert_eq!(version.protocol_version.as_deref(), Some("1.3"));
        handle.join().expect("fixture should finish");
    }

    #[test]
    fn websocket_client_sends_command_and_returns_matching_result() {
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
            assert!(message.contains(r#""id":1"#));
            assert!(message.contains(r#""method":"Target.getTargetInfo""#));

            socket
                .send(Message::Text(
                    r#"{"method":"Target.targetCreated","params":{}}"#.to_owned().into(),
                ))
                .expect("fixture should write event");
            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"targetInfo":{"targetId":"ABC"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write response");
        });

        let mut client = CdpWebSocket::connect(&format!("ws://{address}/devtools/page/ABC"))
            .expect("websocket should connect");
        let result = client
            .call("Target.getTargetInfo", None)
            .expect("command should complete");

        assert_eq!(result["targetInfo"]["targetId"], "ABC");
        handle.join().expect("fixture should finish");
    }

    #[test]
    fn cdp_client_requests_a_graceful_browser_close() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (mut http_stream, _) = listener.accept().expect("fixture should accept HTTP");
            let mut buffer = [0_u8; 1024];
            let bytes = http_stream
                .read(&mut buffer)
                .expect("fixture should read HTTP");
            let request = String::from_utf8_lossy(&buffer[..bytes]);
            assert!(request.starts_with("GET /json/version HTTP/1.1"));
            let body = format!(
                r#"{{"Browser":"Chrome/123.0.0.0","webSocketDebuggerUrl":"ws://{address}/devtools/browser/fixture"}}"#
            );
            write!(
                http_stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("fixture should write HTTP");
            drop(http_stream);

            let (stream, _) = listener.accept().expect("fixture should accept WebSocket");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");
            let message = socket
                .read()
                .expect("fixture should read close command")
                .into_text()
                .expect("command should be text");
            assert!(message.contains(r#""method":"Browser.close""#));
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write close response");
        });

        let endpoint = ResolvedEndpoint {
            source: EndpointSource::Flag,
            display: format!("http://{address}"),
        };
        CdpClient::new(endpoint)
            .close_browser()
            .expect("browser close should complete");
        handle.join().expect("fixture should finish");
    }

    #[test]
    fn websocket_client_returns_command_error_response() {
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
            assert!(message.contains(r#""id":1"#));
            assert!(message.contains(r#""method":"Runtime.evaluate""#));

            socket
                .send(Message::Text(
                    r#"{"id":1,"error":{"code":-32601,"message":"Method not found"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write response");
        });

        let mut client = CdpWebSocket::connect(&format!("ws://{address}/devtools/page/ABC"))
            .expect("websocket should connect");
        let error = client
            .call("Runtime.evaluate", None)
            .expect_err("command should fail");

        assert_eq!(
            error,
            CdpError::Command {
                code: -32601,
                message: "Method not found".to_owned(),
            }
        );
        handle.join().expect("fixture should finish");
    }
}
