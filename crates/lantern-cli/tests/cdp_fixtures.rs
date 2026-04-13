use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output},
    thread::{self, JoinHandle},
};

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
