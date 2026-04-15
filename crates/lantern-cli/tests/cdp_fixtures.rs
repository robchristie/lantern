use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output},
    thread::{self, JoinHandle},
    time::Duration,
};

use tungstenite::Message;

fn read_expect(socket: &mut tungstenite::WebSocket<TcpStream>, expected: &str) -> String {
    let message = socket
        .read()
        .expect("fixture should read command")
        .into_text()
        .expect("command should be text")
        .to_string();
    assert!(
        message.contains(expected),
        "unexpected websocket command: {message:?}"
    );
    message
}

struct HttpFixture {
    endpoint: String,
    handle: JoinHandle<()>,
}

impl HttpFixture {
    fn one_response(expected_path: &'static str, body: impl Into<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let body = body.into();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("fixture should accept");
            let mut buffer = [0_u8; 4096];
            let bytes = stream
                .read(&mut buffer)
                .expect("fixture should read request");
            let request = String::from_utf8_lossy(&buffer[..bytes]);
            assert!(
                request.starts_with(&format!("GET {expected_path} HTTP/1.1")),
                "unexpected request line: {request:?}"
            );

            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("fixture should write response");
        });

        Self {
            endpoint: format!("http://{address}"),
            handle,
        }
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn finish(self) {
        self.handle.join().expect("fixture should finish");
    }
}

struct WebSocketFixture {
    url: String,
    handle: JoinHandle<()>,
}

impl WebSocketFixture {
    fn one_dom_response(body: impl Into<String>) -> Self {
        Self::one_dom_response_with_depth(body, 4)
    }

    fn one_dom_response_with_depth(body: impl Into<String>, expected_depth: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let body = body.into();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");
            let message = socket
                .read()
                .expect("fixture should read command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"DOM.getDocument""#),
                "unexpected websocket command: {message:?}"
            );
            let expected_depth = format!(r#""depth":{expected_depth}"#);
            assert!(
                message.contains(&expected_depth),
                "DOM fixture should request bounded depth: {message:?}"
            );

            socket
                .send(Message::Text(body.into()))
                .expect("fixture should write response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_navigation_response(final_url: &'static str, ready_state: &'static str) -> Self {
        Self::navigation_response(final_url, ready_state, None)
    }

    fn one_navigation_aborted_response(final_url: &'static str, ready_state: &'static str) -> Self {
        Self::navigation_response(final_url, ready_state, Some("net::ERR_ABORTED"))
    }

    fn navigation_response(
        final_url: &'static str,
        ready_state: &'static str,
        error_text: Option<&'static str>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read navigate command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.navigate""#),
                "unexpected websocket command: {message:?}"
            );
            assert!(
                message.contains(
                    r#""url":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag""#
                ),
                "navigation fixture should send the original requested URL: {message:?}"
            );
            let navigate_response = if let Some(error_text) = error_text {
                format!(
                    r#"{{"id":1,"result":{{"frameId":"FRAME_123","loaderId":"LOADER_123","errorText":"{error_text}"}}}}"#
                )
            } else {
                r#"{"id":1,"result":{"frameId":"FRAME_123","loaderId":"LOADER_123"}}"#.to_owned()
            };
            socket
                .send(Message::Text(navigate_response.into()))
                .expect("fixture should write navigate response");

            let message = socket
                .read()
                .expect("fixture should read history command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.getNavigationHistory""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":2,"result":{{"currentIndex":0,"entries":[{{"id":1,"url":"{final_url}","title":"Loaded"}}]}}}}"#
                    )
                    .into(),
                ))
                .expect("fixture should write history response");

            let message = socket
                .read()
                .expect("fixture should read ready state command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.evaluate""#),
                "unexpected websocket command: {message:?}"
            );
            assert!(
                message.contains(r#""expression":"document.readyState""#),
                "navigation fixture should only request readyState: {message:?}"
            );
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":3,"result":{{"result":{{"type":"string","value":"{ready_state}"}}}}}}"#
                    )
                    .into(),
                ))
                .expect("fixture should write ready state response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_navigation_history_error_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read navigate command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.navigate""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"frameId":"FRAME_123","loaderId":"LOADER_123"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write navigate response");

            let message = socket
                .read()
                .expect("fixture should read history command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.getNavigationHistory""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":2,"error":{"code":-32000,"message":"Not attached to an active page"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write history error response");

            let message = socket
                .read()
                .expect("fixture should read ready state command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.evaluate""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":3,"result":{"result":{"type":"string","value":"complete"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write ready state response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_navigation_error() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");
            let message = socket
                .read()
                .expect("fixture should read navigate command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.navigate""#),
                "unexpected websocket command: {message:?}"
            );

            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"frameId":"FRAME_123","errorText":"net::ERR_NAME_NOT_RESOLVED"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write navigate response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_console_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read runtime enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Runtime.enable response");

            let message = socket
                .read()
                .expect("fixture should read log enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Log.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":2,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Log.enable response");

            socket
                .send(Message::Text(
                    r#"{"method":"Runtime.consoleAPICalled","params":{"type":"error","args":[{"type":"string","value":"Failed token=supersecret at https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag"},{"type":"number","value":500}],"stackTrace":{"callFrames":[{"functionName":"render","url":"https://example.test/assets/app.js?build=123#main","lineNumber":42,"columnNumber":7}]}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write console event");
            socket
                .send(Message::Text(
                    r#"{"method":"Runtime.exceptionThrown","params":{"exceptionDetails":{"text":"Uncaught","url":"https://example.test/session/abcdef123456abcdef123456abcdef12?debug=1","lineNumber":9,"columnNumber":3,"exception":{"type":"object","subtype":"error","description":"Error: password=hunter2 from https://example.test/secret/abcdabcdabcdabcdabcdabcd?x=1"},"stackTrace":{"callFrames":[{"functionName":"boot","url":"https://example.test/session/abcdef123456abcdef123456abcdef12?debug=1","lineNumber":9,"columnNumber":3},{"functionName":"main","url":"https://example.test/main.js","lineNumber":1,"columnNumber":1}]}}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write exception event");
            thread::sleep(Duration::from_millis(500));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn clean_console_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read runtime enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Runtime.enable response");

            let message = socket
                .read()
                .expect("fixture should read log enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Log.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":2,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Log.enable response");
            thread::sleep(Duration::from_millis(500));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_console_response_with_interleaved_enable_events() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read runtime enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"method":"Runtime.consoleAPICalled","params":{"type":"error","args":[{"type":"string","value":"Interleaved token=supersecret"}],"stackTrace":{"callFrames":[{"functionName":"boot","url":"https://example.test/assets/runtime.js","lineNumber":12,"columnNumber":4}]}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write interleaved runtime event");
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Runtime.enable response");

            let message = socket
                .read()
                .expect("fixture should read log enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Log.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"method":"Log.entryAdded","params":{"entry":{"level":"error","text":"Interleaved password=hunter2 at https://user:pass@example.test/private/abcdef123456abcdef123456?debug=1","url":"https://example.test/logs/abcdef123456abcdef123456?x=1","lineNumber":8}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write interleaved log event");
            socket
                .send(Message::Text(r#"{"id":2,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Log.enable response");
            thread::sleep(Duration::from_millis(500));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_wait_ready_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            for (id, ready_state) in [(1, "loading"), (2, "complete")] {
                let message = socket
                    .read()
                    .expect("fixture should read ready-state command")
                    .into_text()
                    .expect("command should be text");
                assert!(
                    message.contains(r#""method":"Runtime.evaluate""#),
                    "unexpected websocket command: {message:?}"
                );
                assert!(
                    message.contains(r#""expression":"document.readyState""#),
                    "ready fixture should only request readyState: {message:?}"
                );
                socket
                    .send(Message::Text(
                        format!(
                            r#"{{"id":{id},"result":{{"result":{{"type":"string","value":"{ready_state}"}}}}}}"#
                        )
                        .into(),
                    ))
                    .expect("fixture should write ready-state response");
            }
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_wait_url_response(final_url: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read history command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.getNavigationHistory""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":1,"result":{{"currentIndex":0,"entries":[{{"id":1,"url":"{final_url}","title":"Loaded"}}]}}}}"#
                    )
                    .into(),
                ))
                .expect("fixture should write history response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_wait_selector_response(present: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read selector command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.evaluate""#),
                "unexpected websocket command: {message:?}"
            );
            assert!(
                message.contains(r#"document.querySelector(\"[data-testid=dashboard]\")"#),
                "selector fixture should query the requested selector: {message:?}"
            );
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":1,"result":{{"result":{{"type":"boolean","value":{present}}}}}}}"#
                    )
                    .into(),
                ))
                .expect("fixture should write selector response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_wait_text_response(text: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read text command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Runtime.evaluate""#),
                "unexpected websocket command: {message:?}"
            );
            assert!(
                message.contains(r#"document.querySelector(\"main\")"#),
                "text fixture should query the requested selector: {message:?}"
            );
            socket
                .send(Message::Text(
                    format!(
                        r#"{{"id":1,"result":{{"result":{{"type":"string","value":"{text}"}}}}}}"#
                    )
                    .into(),
                ))
                .expect("fixture should write text response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_wait_quiet_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            for (id, method) in [
                (1, "Runtime.enable"),
                (2, "Log.enable"),
                (3, "Page.enable"),
                (4, "Page.setLifecycleEventsEnabled"),
            ] {
                let message = socket
                    .read()
                    .expect("fixture should read quiet setup command")
                    .into_text()
                    .expect("command should be text");
                assert!(
                    message.contains(&format!(r#""method":"{method}""#)),
                    "unexpected websocket command: {message:?}"
                );
                socket
                    .send(Message::Text(
                        format!(r#"{{"id":{id},"result":{{}}}}"#).into(),
                    ))
                    .expect("fixture should write quiet setup response");
            }

            thread::sleep(Duration::from_millis(100));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_screenshot_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read layout metrics command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.getLayoutMetrics""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"cssVisualViewport":{"clientWidth":1280,"clientHeight":720}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write layout metrics response");

            let message = socket
                .read()
                .expect("fixture should read screenshot command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Page.captureScreenshot""#),
                "unexpected websocket command: {message:?}"
            );
            assert!(
                message.contains(r#""format":"png""#),
                "screenshot fixture should request PNG format: {message:?}"
            );
            assert!(
                message.contains(r#""captureBeyondViewport":false"#),
                "screenshot fixture should capture visible viewport only: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":2,"result":{"data":"iVBORw0KGgo="}}"#.to_owned().into(),
                ))
                .expect("fixture should write screenshot response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_network_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read network enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Network.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Network.enable response");

            socket
                .send(Message::Text(
                    r#"{"method":"Network.requestWillBeSent","params":{"requestId":"REQ_FAIL","request":{"method":"GET","url":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write failed request metadata");
            socket
                .send(Message::Text(
                    r#"{"method":"Network.loadingFailed","params":{"requestId":"REQ_FAIL","type":"Fetch","errorText":"net::ERR_NAME_NOT_RESOLVED"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write loading failed event");
            socket
                .send(Message::Text(
                    r#"{"method":"Network.requestWillBeSent","params":{"requestId":"REQ_HTTP","request":{"method":"POST","url":"https://example.test/api/save?token=secret#frag"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write HTTP request metadata");
            socket
                .send(Message::Text(
                    r#"{"method":"Network.responseReceived","params":{"requestId":"REQ_HTTP","type":"XHR","response":{"url":"https://example.test/api/save?token=secret#frag","status":500}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write HTTP error response event");
            thread::sleep(Duration::from_millis(500));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_network_clean_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            let message = socket
                .read()
                .expect("fixture should read network enable command")
                .into_text()
                .expect("command should be text");
            assert!(
                message.contains(r#""method":"Network.enable""#),
                "unexpected websocket command: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":1,"result":{}}"#.to_owned().into()))
                .expect("fixture should write Network.enable response");
            thread::sleep(Duration::from_millis(500));
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_flow_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            for (id, method) in [
                (1, "Runtime.enable"),
                (2, "Log.enable"),
                (3, "Network.enable"),
                (4, "Page.enable"),
            ] {
                let message = read_expect(&mut socket, &format!(r#""method":"{method}""#));
                assert!(
                    message.contains(r#""id":"#),
                    "flow fixture should receive command ids: {message:?}"
                );
                socket
                    .send(Message::Text(
                        format!(r#"{{"id":{id},"result":{{}}}}"#).into(),
                    ))
                    .expect("fixture should write enable response");
            }

            let message = read_expect(&mut socket, r#""method":"Page.navigate""#);
            assert!(
                message.contains(r#""url":"https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag""#),
                "flow fixture should navigate using the requested URL: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":5,"result":{"frameId":"FRAME_123","loaderId":"LOADER_123"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write navigate response");

            socket
                .send(Message::Text(
                    r#"{"method":"Runtime.consoleAPICalled","params":{"type":"error","args":[{"type":"string","value":"Flow token=supersecret at https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag"}],"stackTrace":{"callFrames":[{"functionName":"render","url":"https://example.test/assets/app.js?build=123#main","lineNumber":42,"columnNumber":7}]}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write console event");
            socket
                .send(Message::Text(
                    r#"{"method":"Network.requestWillBeSent","params":{"requestId":"REQ_FLOW","request":{"method":"GET","url":"https://example.test/api/secret/abcdef123456abcdef123456abcdef12?token=secret#frag"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write request metadata");
            socket
                .send(Message::Text(
                    r#"{"method":"Network.loadingFailed","params":{"requestId":"REQ_FLOW","type":"Fetch","errorText":"net::ERR_NAME_NOT_RESOLVED"}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write network failure");

            let message = read_expect(&mut socket, r#""method":"Runtime.evaluate""#);
            assert!(
                message.contains(r#""expression":"document.title""#),
                "flow fixture should request page title: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":6,"result":{"result":{"type":"string","value":"Flow checkout page"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write title response");

            let message = read_expect(&mut socket, r#""method":"Runtime.evaluate""#);
            assert!(
                message.contains(r#""expression":"document.readyState""#),
                "flow fixture should request ready state: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":7,"result":{"result":{"type":"string","value":"complete"}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write ready-state response");

            read_expect(&mut socket, r#""method":"Page.getNavigationHistory""#);
            socket
                .send(Message::Text(
                    r#"{"id":8,"result":{"currentIndex":0,"entries":[{"id":1,"url":"https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag","title":"Loaded"}]}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write history response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_click_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            read_expect(&mut socket, r#""method":"DOM.getDocument""#);
            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"root":{"nodeId":1}}}"#.to_owned().into(),
                ))
                .expect("fixture should write document response");

            let message = read_expect(&mut socket, r#""method":"DOM.querySelector""#);
            assert!(
                message.contains(r#""selector":"[data-testid=save]""#),
                "click fixture should query the requested selector: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":2,"result":{"nodeId":42}}"#.to_owned().into(),
                ))
                .expect("fixture should write selector response");

            read_expect(&mut socket, r#""method":"DOM.describeNode""#);
            socket
                .send(Message::Text(
                    r#"{"id":3,"result":{"node":{"nodeName":"BUTTON"}}}"#.to_owned().into(),
                ))
                .expect("fixture should write node response");

            read_expect(&mut socket, r#""method":"DOM.getBoxModel""#);
            socket
                .send(Message::Text(
                    r#"{"id":4,"result":{"model":{"content":[10,20,110,20,110,60,10,60]}}}"#
                        .to_owned()
                        .into(),
                ))
                .expect("fixture should write box response");

            for (id, event_type) in [(5, "mouseMoved"), (6, "mousePressed"), (7, "mouseReleased")] {
                let message = read_expect(&mut socket, r#""method":"Input.dispatchMouseEvent""#);
                assert!(
                    message.contains(&format!(r#""type":"{event_type}""#)),
                    "click fixture should dispatch {event_type}: {message:?}"
                );
                assert!(
                    message.contains(r#""x":60.0"#) && message.contains(r#""y":40.0"#),
                    "click fixture should dispatch at computed center: {message:?}"
                );
                socket
                    .send(Message::Text(
                        format!(r#"{{"id":{id},"result":{{}}}}"#).into(),
                    ))
                    .expect("fixture should write input response");
            }
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_type_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");

            read_expect(&mut socket, r#""method":"DOM.getDocument""#);
            socket
                .send(Message::Text(
                    r#"{"id":1,"result":{"root":{"nodeId":1}}}"#.to_owned().into(),
                ))
                .expect("fixture should write document response");

            let message = read_expect(&mut socket, r#""method":"DOM.querySelector""#);
            assert!(
                message.contains(r#""selector":"input[name=q]""#),
                "type fixture should query the requested selector: {message:?}"
            );
            socket
                .send(Message::Text(
                    r#"{"id":2,"result":{"nodeId":21}}"#.to_owned().into(),
                ))
                .expect("fixture should write selector response");

            read_expect(&mut socket, r#""method":"DOM.describeNode""#);
            socket
                .send(Message::Text(
                    r#"{"id":3,"result":{"node":{"nodeName":"INPUT"}}}"#.to_owned().into(),
                ))
                .expect("fixture should write node response");

            read_expect(&mut socket, r#""method":"DOM.focus""#);
            socket
                .send(Message::Text(r#"{"id":4,"result":{}}"#.to_owned().into()))
                .expect("fixture should write focus response");

            let message = read_expect(&mut socket, r#""method":"Input.insertText""#);
            assert!(
                message.contains(r#""text":"hello lantern""#),
                "type fixture should insert requested text: {message:?}"
            );
            socket
                .send(Message::Text(r#"{"id":5,"result":{}}"#.to_owned().into()))
                .expect("fixture should write insert response");
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn one_click_selector_timeout_response() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        let address = listener.local_addr().expect("fixture should have address");
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("fixture should accept");
            let mut socket = tungstenite::accept(stream).expect("fixture websocket should accept");
            let mut id = 1;

            while let Ok(message) = socket.read() {
                let message = message.into_text().expect("command should be text");
                if message.contains(r#""method":"DOM.getDocument""#) {
                    socket
                        .send(Message::Text(
                            format!(r#"{{"id":{id},"result":{{"root":{{"nodeId":1}}}}}}"#).into(),
                        ))
                        .expect("fixture should write document response");
                } else if message.contains(r#""method":"DOM.querySelector""#) {
                    socket
                        .send(Message::Text(
                            format!(r#"{{"id":{id},"result":{{"nodeId":0}}}}"#).into(),
                        ))
                        .expect("fixture should write selector response");
                } else {
                    panic!("unexpected websocket command: {message:?}");
                }
                id += 1;
            }
        });

        Self {
            url: format!("ws://{address}/devtools/page/PAGE_ATTACHED_1234567890"),
            handle,
        }
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn finish(self) {
        self.handle.join().expect("fixture should finish");
    }
}

#[test]
fn doctor_json_uses_env_endpoint_and_reads_browser_metadata() {
    let fixture = HttpFixture::one_response(
        "/json/version",
        r#"{"Browser":"Chrome/123.0.0.0","Protocol-Version":"1.3","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/browser/abc"}"#,
    );

    let output = lantern(["--json", "doctor"], Some(fixture.endpoint()));

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            r#"{{"schema_version":1,"command":"doctor","ok":true,"endpoint":{{"source":"env","display":"{}"}},"browser":{{"product":"Chrome/123.0.0.0","protocol_version":"1.3"}},"websocket":{{"available":true}}}}"#,
            fixture.endpoint()
        ) + "\n"
    );
    fixture.finish();
}

#[test]
fn targets_json_orders_targets_and_redacts_browser_data() {
    let fixture = HttpFixture::one_response(
        "/devtools/json/list",
        r#"[
            {
                "id": "WORKER_1234567890",
                "type": "service_worker",
                "title": "Worker",
                "url": "https://example.test/worker.js",
                "attached": true
            },
            {
                "id": "PAGE_DETACHED_1234567890",
                "type": "page",
                "title": "Detached page",
                "url": "https://example.test/users/123/settings?tab=billing#token",
                "attached": false
            },
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
                "attached": true
            }
        ]"#,
    );
    let endpoint = format!("{}/devtools", fixture.endpoint());

    let output = lantern(["--json", "--endpoint", &endpoint, "targets"], None);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"targets","ok":true,"targets":[{"id":"PAGE_ATTACHED_1234567890","type":"page","title":"Attached page","url_shape":"https://example.test/reset/:redacted","attached":true},{"id":"PAGE_DETACHED_1234567890","type":"page","title":"Detached page","url_shape":"https://example.test/users/123/settings","attached":false},{"id":"WORKER_1234567890","type":"service_worker","title":"Worker","url_shape":"https://example.test/worker.js","attached":true}]}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn targets_json_no_redact_preserves_full_url_and_title_text() {
    let long_title = "A".repeat(121);
    let body = format!(
        r#"[{{"id":"PAGE_FULL_1234567890","type":"page","title":"{}","url":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag","attached":true}}]"#,
        long_title
    );
    let fixture = HttpFixture::one_response("/json/list", body);

    let output = lantern(
        [
            "--json",
            "--no-redact",
            "--endpoint",
            fixture.endpoint(),
            "targets",
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            r#"{{"schema_version":1,"command":"targets","ok":true,"targets":[{{"id":"PAGE_FULL_1234567890","type":"page","title":"{}","url_shape":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag","attached":true}}]}}"#,
            long_title
        ) + "\n"
    );
    fixture.finish();
}

#[test]
fn page_json_selects_single_attached_page_from_target_fixture() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_DETACHED_1234567890",
                "type": "page",
                "title": "Detached page",
                "url": "https://example.test/detached",
                "attached": false
            },
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/session/abcdef123456abcdef123456abcdef12",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "page"], None);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"page","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Attached page","url_shape":"https://example.test/session/:redacted","loading_state":null}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn page_json_selects_explicit_target_id_from_multiple_pages() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ONE_1234567890",
                "type": "page",
                "title": "One",
                "url": "https://example.test/one",
                "attached": true
            },
            {
                "id": "PAGE_TWO_1234567890",
                "type": "page",
                "title": "Two",
                "url": "https://example.test/two",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "--target-id",
            "PAGE_TWO_1234567890",
            "page",
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"page","ok":true,"page":{"target_id":"PAGE_TWO_1234567890","title":"Two","url_shape":"https://example.test/two","loading_state":null}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn page_json_reports_explicit_target_id_that_is_not_a_page() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "WORKER_1234567890",
                "type": "service_worker",
                "title": "Worker",
                "url": "https://example.test/worker.js",
                "attached": true
            },
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/page",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "--target-id",
            "WORKER_1234567890",
            "page",
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_not_found","message":"No page target matched the requested target id.","hint":"Run lantern targets and pass the exact id of a page target."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn open_json_navigates_selected_page_and_redacts_url_shapes() {
    let websocket = WebSocketFixture::one_navigation_response(
        "https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        "loading",
    );
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "open",
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"open","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"navigation":{"requested_url_shape":"https://example.test/reset/:redacted","final_url_shape":"https://example.test/reset/:redacted","loading_state":"loading"}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn open_json_treats_aborted_navigation_as_success_when_page_state_is_readable() {
    let websocket = WebSocketFixture::one_navigation_aborted_response(
        "https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        "complete",
    );
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "open",
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"open","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"navigation":{"requested_url_shape":"https://example.test/reset/:redacted","final_url_shape":"https://example.test/reset/:redacted","loading_state":"complete"}}"#.to_owned()
            + "
"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn open_json_succeeds_when_post_navigation_history_is_unavailable() {
    let websocket = WebSocketFixture::one_navigation_history_error_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "open",
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"open","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"navigation":{"requested_url_shape":"https://example.test/reset/:redacted","final_url_shape":null,"loading_state":"complete"}}"#.to_owned()
            + "
"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn open_json_no_redact_preserves_requested_and_final_urls() {
    let requested =
        "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag";
    let websocket = WebSocketFixture::one_navigation_response(requested, "complete");
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--no-redact",
            "--endpoint",
            fixture.endpoint(),
            "open",
            requested,
        ],
        None,
    );

    assert_success(&output);
    assert!(
        stdout(&output).contains(
            r#""requested_url_shape":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag","final_url_shape":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag""#
        ),
        "unexpected stdout: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn open_json_reports_invalid_navigation_url_before_cdp_websocket() {
    let output = lantern(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "open",
            "javascript:alert(1)",
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"url_invalid","message":"Invalid navigation URL.","hint":"Pass an absolute http:// or https:// URL, or the exact URL about:blank."}}"#.to_owned()
            + "\n"
    );
}

#[test]
fn open_json_reports_navigation_failure() {
    let websocket = WebSocketFixture::one_navigation_error();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "open",
            "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"navigation_failed","message":"Chromium failed the requested page navigation.","hint":"Check that the URL is reachable from Chromium, then retry."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_ready_json_reports_match_elapsed_timeout_and_observed_state() {
    let websocket = WebSocketFixture::one_wait_ready_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "wait",
            "ready",
            "--state",
            "complete",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["command"], "wait");
    assert_eq!(json["page"]["target_id"], "PAGE_ATTACHED_1234567890");
    assert_eq!(json["wait"]["condition"], "ready");
    assert_eq!(json["wait"]["matched"], true);
    assert_eq!(json["wait"]["timed_out"], false);
    assert_eq!(json["wait"]["timeout_ms"], 1000);
    assert_eq!(json["wait"]["observed"]["ready_state"], "complete");
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_url_json_matches_redacted_current_url_shape() {
    let websocket = WebSocketFixture::one_wait_url_response(
        "https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
    );
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "wait",
            "url",
            "--url-shape",
            "https://example.test/reset/:redacted",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["wait"]["condition"], "url");
    assert_eq!(json["wait"]["matched"], true);
    assert_eq!(
        json["wait"]["observed"]["expected_url_shape"],
        "https://example.test/reset/:redacted"
    );
    assert_eq!(
        json["wait"]["observed"]["current_url_shape"],
        "https://example.test/reset/:redacted"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_selector_json_reports_timeout_as_condition_result() {
    let websocket = WebSocketFixture::one_wait_selector_response(false);
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "wait",
            "selector",
            "--selector",
            "[data-testid=dashboard]",
            "--timeout-ms",
            "1",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["wait"]["condition"], "selector");
    assert_eq!(json["wait"]["matched"], false);
    assert_eq!(json["wait"]["timed_out"], true);
    assert_eq!(
        json["wait"]["observed"]["selector"],
        "[data-testid=dashboard]"
    );
    assert_eq!(json["wait"]["observed"]["present"], false);
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_text_json_matches_text_content_substring() {
    let websocket = WebSocketFixture::one_wait_text_response("Checkout Dashboard Ready");
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "wait",
            "text",
            "--selector",
            "main",
            "--text",
            "Dashboard",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["wait"]["condition"], "text");
    assert_eq!(json["wait"]["matched"], true);
    assert_eq!(json["wait"]["observed"]["selector"], "main");
    assert_eq!(json["wait"]["observed"]["expected_text"], "Dashboard");
    assert_eq!(
        json["wait"]["observed"]["current_text"],
        "Checkout Dashboard Ready"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_quiet_json_reports_quiet_period_without_events() {
    let websocket = WebSocketFixture::one_wait_quiet_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "wait",
            "quiet",
            "--quiet-ms",
            "10",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["wait"]["condition"], "quiet");
    assert_eq!(json["wait"]["matched"], true);
    assert_eq!(json["wait"]["timed_out"], false);
    assert_eq!(json["wait"]["observed"]["quiet_ms"], 10);
    assert_eq!(json["wait"]["observed"]["event_count"], 0);
    assert_eq!(
        json["wait"]["observed"]["last_event"],
        serde_json::Value::Null
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn wait_json_rejects_invalid_timeout_bounds() {
    let output = lantern(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "ready",
            "--timeout-ms",
            "0",
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"wait_timeout_invalid","message":"Invalid wait timeout.","hint":"Pass --timeout-ms from 1 through 30000; for quiet waits, --quiet-ms must also be in range and no larger than --timeout-ms."}}"#.to_owned()
            + "\n"
    );
}

#[test]
fn wait_json_rejects_irrelevant_condition_flags_before_endpoint_resolution() {
    assert_wait_irrelevant_flags_rejected(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "ready",
            "--selector",
            "main",
            "--timeout-ms",
            "1000",
        ],
        "Unsupported flag for wait ready.",
        "Run lantern wait ready [--state <loading|interactive|complete>] --timeout-ms <MS>.",
    );
    assert_wait_irrelevant_flags_rejected(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "url",
            "--url-shape",
            "https://example.test/dashboard",
            "--state",
            "complete",
            "--timeout-ms",
            "1000",
        ],
        "Unsupported flag for wait url.",
        "Run lantern wait url --url-shape <URL_SHAPE> --timeout-ms <MS>.",
    );
    assert_wait_irrelevant_flags_rejected(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "selector",
            "--selector",
            "main",
            "--text",
            "Ready",
            "--timeout-ms",
            "1000",
        ],
        "Unsupported flag for wait selector.",
        "Run lantern wait selector --selector <CSS_SELECTOR> --timeout-ms <MS>.",
    );
    assert_wait_irrelevant_flags_rejected(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "text",
            "--selector",
            "main",
            "--text",
            "Ready",
            "--quiet-ms",
            "10",
            "--timeout-ms",
            "1000",
        ],
        "Unsupported flag for wait text.",
        "Run lantern wait text --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>.",
    );
    assert_wait_irrelevant_flags_rejected(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "wait",
            "quiet",
            "--quiet-ms",
            "10",
            "--url-shape",
            "https://example.test/dashboard",
            "--timeout-ms",
            "1000",
        ],
        "Unsupported flag for wait quiet.",
        "Run lantern wait quiet --quiet-ms <MS> --timeout-ms <MS>.",
    );
}

#[test]
fn dom_json_summarizes_document_and_redacts_sensitive_values() {
    let websocket = WebSocketFixture::one_dom_response(dom_document_response());
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "dom"], None);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"dom","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"dom":{"node_count":5,"max_depth":4,"truncated":true,"nodes":[{"node_id":"n2","tag":"html","role":null,"name":null,"text":null,"attributes":{"class":"root shell"},"child_count":1,"children":[{"node_id":"n3","tag":"body","role":null,"name":null,"text":null,"attributes":{"id":"app","role":"document"},"child_count":1,"children":[{"node_id":"n5","tag":"main","role":null,"name":null,"text":"Dashboard","attributes":{"aria-label":"Main panel","data-testid":"dashboard","href":"https://example.test/reset/:redacted","name":"search","src":"https://cdn.example.test/assets/app.js","title":"Main title"},"child_count":2,"children":[{"node_id":"n7","tag":"script","role":null,"name":null,"text":null,"attributes":{},"child_count":1,"children":[]},{"node_id":"n8","tag":"section","role":null,"name":null,"text":null,"attributes":{"data-cy":"primary-section"},"child_count":1,"children":[]}]}]}]}]}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn dom_json_accepts_depth_and_max_nodes_flags() {
    let websocket = WebSocketFixture::one_dom_response_with_depth(dom_document_response(), 6);
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "dom",
            "--depth",
            "6",
            "--max-nodes",
            "3",
        ],
        None,
    );

    assert_success(&output);
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout should be JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["dom"]["node_count"], 3);
    assert_eq!(json["dom"]["max_depth"], 3);
    assert_eq!(json["dom"]["truncated"], true);
    fixture.finish();
    websocket.finish();
}

#[test]
fn dom_json_rejects_invalid_dom_limits_before_endpoint_resolution() {
    let output = lantern(["--json", "dom", "--depth", "13"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");

    let json: serde_json::Value =
        serde_json::from_str(&stderr(&output)).expect("stderr should be JSON");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "dom_limit_invalid");
    assert_eq!(json["error"]["message"], "Invalid DOM summary limit.");
}

#[test]
fn dom_flags_are_rejected_for_non_dom_commands() {
    let output = lantern(["--json", "page", "--depth", "6"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");

    let json: serde_json::Value =
        serde_json::from_str(&stderr(&output)).expect("stderr should be JSON");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "usage");
    assert_eq!(
        json["error"]["message"],
        "DOM limit flags are only supported by dom."
    );
}

#[test]
fn console_json_collects_errors_and_exceptions_with_redaction() {
    let websocket = WebSocketFixture::one_console_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "console"],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"console","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"console":{"message_count":1,"exception_count":1,"max_entries":20,"message_text_limit":500,"truncated":false,"observed_clean":false,"collection_gap":true,"collection_gap_reason":"events_before_runtime_log_enable_not_observed","entries":[{"sequence":1,"kind":"console","severity":"error","message":"Failed token=:redacted at https://example.test/reset/:redacted 500","source_url_shape":"https://example.test/assets/app.js","line":42,"column":7,"argument_count":2,"stack_frame_count":1},{"sequence":2,"kind":"exception","severity":"error","message":"Error: password=:redacted from https://example.test/secret/:redacted","source_url_shape":"https://example.test/session/:redacted","line":9,"column":3,"argument_count":0,"stack_frame_count":2}]}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn console_json_collects_events_interleaved_with_enable_responses() {
    let websocket = WebSocketFixture::one_console_response_with_interleaved_enable_events();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "console"],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"console","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"console":{"message_count":2,"exception_count":0,"max_entries":20,"message_text_limit":500,"truncated":false,"observed_clean":false,"collection_gap":true,"collection_gap_reason":"events_before_runtime_log_enable_not_observed","entries":[{"sequence":1,"kind":"console","severity":"error","message":"Interleaved token=:redacted","source_url_shape":"https://example.test/assets/runtime.js","line":12,"column":4,"argument_count":1,"stack_frame_count":1},{"sequence":2,"kind":"console","severity":"error","message":"Interleaved password=:redacted at https://example.test/private/:redacted","source_url_shape":"https://example.test/logs/:redacted","line":8,"column":null,"argument_count":0,"stack_frame_count":0}]}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn console_json_distinguishes_observed_clean_from_collection_gap() {
    let websocket = WebSocketFixture::clean_console_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "console"],
        None,
    );

    assert_success(&output);
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("json stdout");
    assert_eq!(json["command"], "console");
    assert_eq!(json["console"]["message_count"], 0);
    assert_eq!(json["console"]["exception_count"], 0);
    assert_eq!(json["console"]["observed_clean"], true);
    assert_eq!(json["console"]["collection_gap"], true);
    assert_eq!(
        json["console"]["collection_gap_reason"],
        "events_before_runtime_log_enable_not_observed"
    );
    assert_eq!(json["console"]["entries"], serde_json::json!([]));
    fixture.finish();
    websocket.finish();
}

#[test]
fn console_human_reports_counts_and_entries() {
    let websocket = WebSocketFixture::one_console_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(["--endpoint", fixture.endpoint(), "console"], None);

    assert_success(&output);
    assert!(
        stdout(&output).starts_with(
            "console: PAGE_ATT title=\"Checkout token page\" url=https://example.test/reset/:redacted messages=1 exceptions=1 observed_clean=false collection_gap=true truncated=false\n"
        ),
        "unexpected stdout: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn console_json_reports_missing_page_websocket_url() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/page",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "console"],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_websocket_missing","message":"Selected page target did not expose a WebSocket debugger URL.","hint":"Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn network_json_collects_failed_requests_and_http_errors_with_redaction() {
    let websocket = WebSocketFixture::one_network_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "network"],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{"schema_version":1,"command":"network","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","title":"Checkout token page","url_shape":"https://example.test/reset/:redacted"},"network":{"failed_count":1,"http_error_count":1,"max_entries":20,"truncated":false,"observed_clean":false,"collection_gap":true,"collection_gap_reason":"events_before_network_enable_not_observed","entries":[{"sequence":1,"kind":"failed_request","request_id":"REQ_FAIL","method":"GET","url_shape":"https://example.test/reset/:redacted","resource_type":"Fetch","status":null,"failure_reason":"net::ERR_NAME_NOT_RESOLVED"},{"sequence":2,"kind":"http_error_response","request_id":"REQ_HTTP","method":"POST","url_shape":"https://example.test/api/save","resource_type":"XHR","status":500,"failure_reason":null}]}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn network_json_no_redact_preserves_request_urls() {
    let websocket = WebSocketFixture::one_network_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--no-redact",
            "--endpoint",
            fixture.endpoint(),
            "network",
        ],
        None,
    );

    assert_success(&output);
    assert!(
        stdout(&output).contains(
            r#""url_shape":"https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag""#
        ),
        "unexpected stdout: {:?}",
        stdout(&output)
    );
    assert!(
        stdout(&output)
            .contains(r#""url_shape":"https://example.test/api/save?token=secret#frag""#),
        "unexpected stdout: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn network_human_reports_counts_entries_and_collection_gap() {
    let websocket = WebSocketFixture::one_network_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(["--endpoint", fixture.endpoint(), "network"], None);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "network: PAGE_ATT title=\"Checkout token page\" url=https://example.test/reset/:redacted failed=1 http_errors=1 observed_clean=false collection_gap=true truncated=false\n1 failed_request GET https://example.test/reset/:redacted resource=Fetch status=null reason=net::ERR_NAME_NOT_RESOLVED\n2 http_error_response POST https://example.test/api/save resource=XHR status=500 reason=null\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn network_json_distinguishes_observed_clean_from_collection_gap() {
    let websocket = WebSocketFixture::one_network_clean_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "network"],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["command"], "network");
    assert_eq!(json["network"]["failed_count"], 0);
    assert_eq!(json["network"]["http_error_count"], 0);
    assert_eq!(json["network"]["observed_clean"], true);
    assert_eq!(json["network"]["collection_gap"], true);
    assert_eq!(
        json["network"]["collection_gap_reason"],
        "events_before_network_enable_not_observed"
    );
    assert_eq!(json["network"]["entries"], serde_json::json!([]));
    fixture.finish();
    websocket.finish();
}

#[test]
fn network_json_reports_missing_page_websocket_url() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/page",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(
        ["--json", "--endpoint", fixture.endpoint(), "network"],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_websocket_missing","message":"Selected page target did not expose a WebSocket debugger URL.","hint":"Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn flow_json_observes_navigation_wait_console_and_network_on_one_attachment() {
    let websocket = WebSocketFixture::one_flow_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "flow",
            "--open",
            "https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
            "--timeout-ms",
            "1000",
            "--quiet-ms",
            "100",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["command"], "flow");
    assert_eq!(json["flow"]["single_attachment"], true);
    assert_eq!(json["flow"]["opened_url"], true);
    assert_eq!(json["flow"]["observation_started_before_navigation"], true);
    assert_eq!(json["flow"]["wait"]["condition"], "quiet");
    assert_eq!(json["flow"]["wait"]["matched"], true);
    assert_eq!(json["page"]["title"], "Flow checkout page");
    assert_eq!(
        json["page"]["url_shape"],
        "https://example.test/reset/:redacted"
    );
    assert_eq!(json["console"]["message_count"], 1);
    assert_eq!(json["console"]["collection_gap"], false);
    assert_eq!(
        json["console"]["collection_gap_reason"],
        "collection_started_before_observed_flow"
    );
    assert_eq!(json["network"]["failed_count"], 1);
    assert_eq!(json["network"]["collection_gap"], false);
    assert_eq!(
        json["network"]["collection_gap_reason"],
        "collection_started_before_observed_flow"
    );
    assert!(
        stdout(&output).contains("token=:redacted"),
        "flow output should redact browser-derived secret-looking text: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn screenshot_json_captures_visible_viewport_to_explicit_output_path() {
    let websocket = WebSocketFixture::one_screenshot_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));
    let output_path = unique_temp_png_path("capture");
    let output_path_string = output_path.to_string_lossy().into_owned();

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "screenshot",
            "--output",
            &output_path_string,
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["command"], "screenshot");
    assert_eq!(json["page"]["target_id"], "PAGE_ATTACHED_1234567890");
    assert_eq!(
        json["page"]["url_shape"],
        "https://example.test/reset/:redacted"
    );
    assert_eq!(json["screenshot"]["format"], "png");
    assert_eq!(json["screenshot"]["width"], 1280);
    assert_eq!(json["screenshot"]["height"], 720);
    assert_eq!(json["screenshot"]["byte_count"], 8);
    assert_eq!(json["screenshot"]["path"], output_path_string);
    assert_eq!(json["screenshot"]["overwritten"], false);
    assert_eq!(
        json["screenshot"]["redaction_caveat"],
        "screenshot_contains_visible_page_pixels"
    );
    assert_eq!(
        fs::read(&output_path).expect("screenshot file should be written"),
        b"\x89PNG\r\n\x1a\n"
    );
    fs::remove_file(&output_path).expect("screenshot file should clean up");
    fixture.finish();
    websocket.finish();
}

#[test]
fn screenshot_human_reports_short_metadata_and_path() {
    let websocket = WebSocketFixture::one_screenshot_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));
    let output_path = unique_temp_png_path("human");
    let output_path_string = output_path.to_string_lossy().into_owned();

    let output = lantern(
        [
            "--endpoint",
            fixture.endpoint(),
            "screenshot",
            "--output",
            &output_path_string,
        ],
        None,
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "screenshot: PAGE_ATT title=\"Checkout token page\" url=https://example.test/reset/:redacted dimensions=1280x720 bytes=8 path={} overwritten=false caveat=screenshot_contains_visible_page_pixels\n",
            output_path_string
        )
    );
    fs::remove_file(&output_path).expect("screenshot file should clean up");
    fixture.finish();
    websocket.finish();
}

#[test]
fn screenshot_json_rejects_existing_output_without_overwrite_before_cdp() {
    let output_path = unique_temp_png_path("exists");
    fs::write(&output_path, b"existing").expect("fixture should write existing file");
    let output_path_string = output_path.to_string_lossy().into_owned();

    let output = lantern(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "screenshot",
            "--output",
            &output_path_string,
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"screenshot_output_exists","message":"Screenshot output path already exists.","hint":"Pass --overwrite to replace the file, or choose a different --output path."}}"#.to_owned()
            + "\n"
    );
    assert_eq!(
        fs::read(&output_path).expect("existing file should remain"),
        b"existing"
    );
    fs::remove_file(&output_path).expect("fixture should clean up");
}

#[test]
fn screenshot_json_overwrites_existing_output_when_requested() {
    let websocket = WebSocketFixture::one_screenshot_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));
    let output_path = unique_temp_png_path("overwrite");
    fs::write(&output_path, b"existing").expect("fixture should write existing file");
    let output_path_string = output_path.to_string_lossy().into_owned();

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "screenshot",
            "--output",
            &output_path_string,
            "--overwrite",
        ],
        None,
    );

    assert_success(&output);
    let json = json_stdout(&output);
    assert_eq!(json["screenshot"]["overwritten"], true);
    assert_eq!(
        fs::read(&output_path).expect("screenshot file should be overwritten"),
        b"\x89PNG\r\n\x1a\n"
    );
    fs::remove_file(&output_path).expect("screenshot file should clean up");
    fixture.finish();
    websocket.finish();
}

#[test]
fn screenshot_json_reports_missing_page_websocket_url() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/page",
                "attached": true
            }
        ]"#,
    );
    let output_path = unique_temp_png_path("missing-websocket");
    let output_path_string = output_path.to_string_lossy().into_owned();

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "screenshot",
            "--output",
            &output_path_string,
        ],
        None,
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_websocket_missing","message":"Selected page target did not expose a WebSocket debugger URL.","hint":"Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled."}}"#.to_owned()
            + "\n"
    );
    assert!(!output_path.exists());
    fixture.finish();
}

#[test]
fn click_json_dispatches_mouse_events_at_selected_element_center() {
    let websocket = WebSocketFixture::one_click_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "click",
            "--selector",
            "[data-testid=save]",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout should be JSON");
    assert_eq!(json["command"], "click");
    assert_eq!(json["interaction"]["action"], "click");
    assert_eq!(json["interaction"]["selector"], "[data-testid=save]");
    assert_eq!(json["interaction"]["dispatched"], true);
    assert_eq!(json["interaction"]["timed_out"], false);
    assert_eq!(json["interaction"]["timeout_ms"], 1000);
    assert_eq!(json["interaction"]["observed"]["node_name"], "BUTTON");
    assert_eq!(
        json["interaction"]["observed"]["clickable_point"]["x"],
        60.0
    );
    assert_eq!(
        json["interaction"]["observed"]["clickable_point"]["y"],
        40.0
    );
    assert_eq!(
        json["interaction"]["observed"]["inserted_text_length"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["interaction"]["immediate_error"],
        serde_json::Value::Null
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn type_json_focuses_selected_element_and_inserts_text_without_echoing_value() {
    let websocket = WebSocketFixture::one_type_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "type",
            "--selector",
            "input[name=q]",
            "--text",
            "hello lantern",
            "--timeout-ms",
            "1000",
        ],
        None,
    );

    assert_success(&output);
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout should be JSON");
    assert_eq!(json["command"], "type");
    assert_eq!(json["interaction"]["action"], "type");
    assert_eq!(json["interaction"]["selector"], "input[name=q]");
    assert_eq!(json["interaction"]["dispatched"], true);
    assert_eq!(json["interaction"]["observed"]["node_name"], "INPUT");
    assert_eq!(json["interaction"]["observed"]["inserted_text_length"], 13);
    assert_eq!(
        json["interaction"]["observed"]["clickable_point"],
        serde_json::Value::Null
    );
    assert!(
        !stdout(&output).contains("hello lantern"),
        "type output should not echo inserted text: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn click_json_reports_selector_timeout_as_undispatched_action() {
    let websocket = WebSocketFixture::one_click_selector_timeout_response();
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "click",
            "--selector",
            "[data-testid=missing]",
            "--timeout-ms",
            "1",
        ],
        None,
    );

    assert_success(&output);
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout should be JSON");
    assert_eq!(json["command"], "click");
    assert_eq!(json["interaction"]["dispatched"], false);
    assert_eq!(json["interaction"]["timed_out"], true);
    assert_eq!(json["interaction"]["immediate_error"], "selector_not_found");
    assert_eq!(
        json["interaction"]["observed"]["node_name"],
        serde_json::Value::Null
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn type_json_requires_selector_text_and_bounded_timeout_before_cdp() {
    let missing_text = lantern(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "type",
            "--selector",
            "input",
            "--timeout-ms",
            "1000",
        ],
        None,
    );
    assert!(!missing_text.status.success());
    assert_eq!(missing_text.status.code(), Some(2));
    assert_eq!(
        stderr(&missing_text),
        r#"{"schema_version":1,"ok":false,"error":{"code":"usage","message":"Missing --text for type.","hint":"Run lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>."}}"#.to_owned()
            + "\n"
    );

    let invalid_timeout = lantern(
        [
            "--json",
            "--endpoint",
            "http://127.0.0.1:1",
            "type",
            "--selector",
            "input",
            "--text",
            "hello",
            "--timeout-ms",
            "30001",
        ],
        None,
    );
    assert!(!invalid_timeout.status.success());
    assert_eq!(invalid_timeout.status.code(), Some(2));
    assert_eq!(
        stderr(&invalid_timeout),
        r#"{"schema_version":1,"ok":false,"error":{"code":"interaction_timeout_invalid","message":"Invalid interaction timeout.","hint":"Pass --timeout-ms from 1 through 30000."}}"#.to_owned()
            + "\n"
    );
}

#[test]
fn dom_json_selects_explicit_target_id_from_multiple_pages() {
    let websocket = WebSocketFixture::one_dom_response(dom_document_response());
    let fixture = HttpFixture::one_response(
        "/json/list",
        format!(
            r#"[
                {{
                    "id": "PAGE_ONE_1234567890",
                    "type": "page",
                    "title": "One",
                    "url": "https://example.test/one",
                    "attached": true
                }},
                {{
                    "id": "PAGE_ATTACHED_1234567890",
                    "type": "page",
                    "title": "Checkout token page",
                    "url": "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
                    "attached": true,
                    "webSocketDebuggerUrl": "{}"
                }}
            ]"#,
            websocket.url()
        ),
    );

    let output = lantern(
        [
            "--json",
            "--endpoint",
            fixture.endpoint(),
            "--target-id",
            "PAGE_ATTACHED_1234567890",
            "dom",
        ],
        None,
    );

    assert_success(&output);
    assert!(
        stdout(&output).starts_with(
            r#"{"schema_version":1,"command":"dom","ok":true,"page":{"target_id":"PAGE_ATTACHED_1234567890","#
        ),
        "unexpected stdout: {:?}",
        stdout(&output)
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn dom_human_summarizes_document_fixture() {
    let websocket = WebSocketFixture::one_dom_response(dom_document_response());
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(["--endpoint", fixture.endpoint(), "dom"], None);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "dom: PAGE_ATT title=\"Checkout token page\" url=https://example.test/reset/:redacted nodes=5 depth=4 truncated=true\nhtml\n  body#app[role=document]\n    main[data-testid=dashboard] \"Dashboard\"\n      script\n      section[data-cy=primary-section]\n"
    );
    fixture.finish();
    websocket.finish();
}

#[test]
fn dom_json_reports_unavailable_page_target() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "WORKER_1234567890",
                "type": "service_worker",
                "title": "Worker",
                "url": "https://example.test/worker.js",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "dom"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_not_found","message":"No page target was available.","hint":"Open a page in the attached Chromium instance and retry."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn dom_json_reports_missing_page_websocket_url() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Attached page",
                "url": "https://example.test/page",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "dom"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_websocket_missing","message":"Selected page target did not expose a WebSocket debugger URL.","hint":"Refresh the target list, open a normal page target, or restart Chromium with remote debugging enabled."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn dom_json_reports_ambiguous_page_target() {
    let fixture = HttpFixture::one_response(
        "/json/list",
        r#"[
            {
                "id": "PAGE_ONE_1234567890",
                "type": "page",
                "title": "One",
                "url": "https://example.test/one",
                "attached": true
            },
            {
                "id": "PAGE_TWO_1234567890",
                "type": "page",
                "title": "Two",
                "url": "https://example.test/two",
                "attached": true
            }
        ]"#,
    );

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "dom"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"target_ambiguous","message":"Multiple page targets matched.","hint":"Close extra pages, leave exactly one attached page target, or pass --target-id."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
}

#[test]
fn dom_json_reports_cdp_command_error() {
    let websocket = WebSocketFixture::one_dom_response(
        r#"{"id":1,"error":{"code":-32000,"message":"Could not find node with given id"}}"#,
    );
    let fixture =
        HttpFixture::one_response("/json/list", target_list_with_websocket(websocket.url()));

    let output = lantern(["--json", "--endpoint", fixture.endpoint(), "dom"], None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        r#"{"schema_version":1,"ok":false,"error":{"code":"cdp_response_invalid","message":"CDP endpoint returned an invalid response.","hint":"Retry against a fresh Chromium DevTools endpoint or inspect /json/version and /json/list."}}"#.to_owned()
            + "\n"
    );
    fixture.finish();
    websocket.finish();
}

fn target_list_with_websocket(web_socket_debugger_url: &str) -> String {
    format!(
        r#"[
            {{
                "id": "PAGE_ATTACHED_1234567890",
                "type": "page",
                "title": "Checkout token page",
                "url": "https://user:pass@example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab?token=secret#frag",
                "attached": true,
                "webSocketDebuggerUrl": "{web_socket_debugger_url}"
            }}
        ]"#
    )
}

fn dom_document_response() -> &'static str {
    r##"{"id":1,"result":{"root":{"nodeId":1,"nodeType":9,"nodeName":"#document","localName":"","childNodeCount":1,"children":[{"nodeId":2,"nodeType":1,"nodeName":"HTML","localName":"html","attributes":["class","root shell"],"childNodeCount":1,"children":[{"nodeId":3,"nodeType":1,"nodeName":"BODY","localName":"body","attributes":["id","app","role","document","onclick","steal()","data-token","secret"],"childNodeCount":1,"children":[{"nodeId":5,"nodeType":1,"nodeName":"MAIN","localName":"main","attributes":["data-testid","dashboard","aria-label","Main panel","name","search","href","https://example.test/reset/abcdabcdabcdabcdabcdabcd?token=secret#frag","src","https://cdn.example.test/assets/app.js?version=123","title","Main title","password","hunter2","name","session"],"childNodeCount":2,"children":[{"nodeId":6,"nodeType":3,"nodeName":"#text","localName":"","nodeValue":"Dashboard"},{"nodeId":7,"nodeType":1,"nodeName":"SCRIPT","localName":"script","childNodeCount":1,"children":[{"nodeId":70,"nodeType":3,"nodeName":"#text","localName":"","nodeValue":"secretScript()"}]},{"nodeId":8,"nodeType":1,"nodeName":"SECTION","localName":"section","attributes":["data-cy","primary-section"],"childNodeCount":1,"children":[{"nodeId":9,"nodeType":1,"nodeName":"BUTTON","localName":"button","attributes":["data-test","save-button"],"childNodeCount":0}]}]}]}]}]}}}"##
}

fn unique_temp_png_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lantern-{label}-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    ));
    if path.exists() {
        fs::remove_file(&path).expect("stale temp path should be removable");
    }
    path
}

fn lantern<const N: usize>(args: [&str; N], env_endpoint: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lantern"));
    command.args(args);
    command.env_remove("LANTERN_CDP_ENDPOINT");
    if let Some(endpoint) = env_endpoint {
        command.env("LANTERN_CDP_ENDPOINT", endpoint);
    }

    command.output().expect("lantern should run")
}

fn assert_wait_irrelevant_flags_rejected<const N: usize>(
    args: [&str; N],
    message: &str,
    hint: &str,
) {
    let output = lantern(args, None);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");

    let json: serde_json::Value =
        serde_json::from_str(&stderr(&output)).expect("stderr should be JSON");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "usage");
    assert_eq!(json["error"]["message"], message);
    assert_eq!(json["error"]["hint"], hint);
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "expected success, status={:?}, stdout={:?}, stderr={:?}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
    assert_eq!(stderr(output), "");
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json_stdout(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).expect("stdout should be JSON")
}
