use std::{
    net::{IpAddr, TcpStream},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tungstenite::{Message, WebSocket, connect, stream::MaybeTlsStream};
use url::{Host, Url};

use crate::endpoint::ResolvedEndpoint;

#[derive(Debug, Clone)]
pub struct CdpClient {
    endpoint: ResolvedEndpoint,
    http: ureq::Agent,
}

impl CdpClient {
    pub fn new(endpoint: ResolvedEndpoint) -> Self {
        Self {
            endpoint,
            http: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(2))
                .timeout_read(Duration::from_secs(10))
                .timeout_write(Duration::from_secs(5))
                .build(),
        }
    }

    pub fn browser_version(&self) -> Result<BrowserVersion, CdpError> {
        self.get_json("json/version")
    }

    pub fn targets(&self) -> Result<Vec<TargetInfo>, CdpError> {
        self.get_json("json/list")
    }

    fn get_json<T>(&self, path: &str) -> Result<T, CdpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = endpoint_child_url(&self.endpoint, path)?;
        let response = self.http.get(url.as_str()).call().map_err(CdpError::from)?;
        let body = response
            .into_string()
            .map_err(|source| CdpError::ResponseInvalid {
                context: "failed to read CDP HTTP response body",
                source: source.to_string(),
            })?;

        serde_json::from_str(&body).map_err(|source| CdpError::ResponseInvalid {
            context: "failed to parse CDP HTTP JSON response",
            source: source.to_string(),
        })
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
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
}

impl CdpWebSocket {
    pub fn connect(web_socket_debugger_url: &str) -> Result<Self, CdpError> {
        validate_websocket_url(web_socket_debugger_url)?;
        let (socket, _) =
            connect(web_socket_debugger_url).map_err(|source| CdpError::WebSocketTransport {
                context: "failed to connect to CDP WebSocket",
                source: source.to_string(),
            })?;

        Ok(Self { socket, next_id: 1 })
    }

    pub fn call(&mut self, method: &str, params: Option<Value>) -> Result<Value, CdpError> {
        let id = self.next_id;
        self.next_id += 1;

        let request = CdpCommandRequest { id, method, params };
        let request =
            serde_json::to_string(&request).map_err(|source| CdpError::ResponseInvalid {
                context: "failed to serialize CDP WebSocket command",
                source: source.to_string(),
            })?;

        self.socket
            .send(Message::Text(request.into()))
            .map_err(|source| CdpError::WebSocketTransport {
                context: "failed to send CDP WebSocket command",
                source: source.to_string(),
            })?;

        loop {
            let message = self
                .socket
                .read()
                .map_err(|source| CdpError::WebSocketTransport {
                    context: "failed to read CDP WebSocket response",
                    source: source.to_string(),
                })?;

            let Message::Text(text) = message else {
                continue;
            };

            let response: CdpCommandResponse =
                serde_json::from_str(&text).map_err(|source| CdpError::ResponseInvalid {
                    context: "failed to parse CDP WebSocket JSON response",
                    source: source.to_string(),
                })?;

            if response.id != Some(id) {
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
