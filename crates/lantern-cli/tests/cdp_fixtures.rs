use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output},
    thread::{self, JoinHandle},
};

use tungstenite::Message;

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
            assert!(
                message.contains(r#""depth":4"#),
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

fn lantern<const N: usize>(args: [&str; N], env_endpoint: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lantern"));
    command.args(args);
    command.env_remove("LANTERN_CDP_ENDPOINT");
    if let Some(endpoint) = env_endpoint {
        command.env("LANTERN_CDP_ENDPOINT", endpoint);
    }

    command.output().expect("lantern should run")
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
