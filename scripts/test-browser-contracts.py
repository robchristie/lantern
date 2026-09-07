#!/usr/bin/env python3
"""Run deterministic Lantern interaction contracts against disposable Chromium."""

import argparse
import base64
from datetime import datetime, timezone
import hashlib
import http.server
import json
import os
from pathlib import Path
import platform
import select
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request
from urllib.parse import urlparse
import uuid


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "scripts/fixtures/browser-contracts/index.html"
COMMAND_TIMEOUT_SECONDS = 12
SUITE_TIMEOUT_SECONDS = 120


class FixtureHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/intentional-fast-failure"):
            body, status, content_type = b"intentional failure\n", 500, "text/plain"
        elif self.path.startswith("/favicon.ico"):
            body, status, content_type = b"", 204, "image/x-icon"
        else:
            body, status, content_type = FIXTURE.read_bytes(), 200, "text/html; charset=utf-8"
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *_args):
        pass


class CdpAuditProxy(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, upstream):
        super().__init__(("127.0.0.1", 0), CdpAuditHandler)
        self.upstream = upstream
        self.input_commands = []
        self.audit_lock = threading.Lock()

    def record(self, payload):
        try:
            message = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError):
            return
        method = message.get("method", "")
        if method.startswith("Input."):
            with self.audit_lock:
                self.input_commands.append(method)

    def checkpoint(self):
        with self.audit_lock:
            return len(self.input_commands)

    def commands_since(self, checkpoint):
        with self.audit_lock:
            return self.input_commands[checkpoint:]


class CdpAuditHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        if self.headers.get("Upgrade", "").lower() == "websocket":
            self.proxy_websocket()
            return
        with urllib.request.urlopen(self.server.upstream + self.path, timeout=3) as response:
            body = response.read()
            content_type = response.headers.get("Content-Type", "application/json")
        if self.path.startswith("/json/list"):
            targets = json.loads(body)
            proxy_host = f"127.0.0.1:{self.server.server_port}"
            for target in targets:
                websocket = target.get("webSocketDebuggerUrl")
                if websocket:
                    target["webSocketDebuggerUrl"] = (
                        f"ws://{proxy_host}{urlparse(websocket).path}"
                    )
            body = json.dumps(targets).encode()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def proxy_websocket(self):
        upstream_url = urlparse(self.server.upstream)
        upstream = socket.create_connection(
            (upstream_url.hostname, upstream_url.port), timeout=3
        )
        upstream_key = base64.b64encode(os.urandom(16)).decode()
        request = (
            f"GET {self.path} HTTP/1.1\r\n"
            f"Host: {upstream_url.hostname}:{upstream_url.port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {upstream_key}\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        ).encode()
        upstream.sendall(request)
        response = receive_headers(upstream)
        if not response.startswith(b"HTTP/1.1 101"):
            upstream.close()
            raise RuntimeError(f"CDP WebSocket upgrade failed: {response[:120]!r}")

        client_key = self.headers["Sec-WebSocket-Key"]
        accept = base64.b64encode(hashlib.sha1(
            (client_key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()
        ).digest()).decode()
        self.send_response(101, "Switching Protocols")
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        self.close_connection = True
        try:
            while True:
                readable, _, _ = select.select([self.connection, upstream], [], [], 1)
                if not readable:
                    continue
                for source in readable:
                    frame, opcode, payload = receive_frame(source)
                    if source is self.connection:
                        self.server.record(payload if opcode == 1 else b"")
                        upstream.sendall(frame)
                    else:
                        self.connection.sendall(frame)
                    if opcode == 8:
                        return
        except (ConnectionError, OSError):
            return
        finally:
            upstream.close()

    def log_message(self, _format, *_args):
        pass


def receive_headers(connection):
    data = bytearray()
    while b"\r\n\r\n" not in data:
        chunk = connection.recv(4096)
        if not chunk:
            raise ConnectionError("connection closed during WebSocket upgrade")
        data.extend(chunk)
        if len(data) > 65536:
            raise RuntimeError("oversized WebSocket upgrade")
    return bytes(data)


def receive_exact(connection, length):
    data = bytearray()
    while len(data) < length:
        chunk = connection.recv(length - len(data))
        if not chunk:
            raise ConnectionError("connection closed during WebSocket frame")
        data.extend(chunk)
    return bytes(data)


def receive_frame(connection):
    header = receive_exact(connection, 2)
    opcode = header[0] & 0x0F
    masked = bool(header[1] & 0x80)
    length = header[1] & 0x7F
    extension = b""
    if length == 126:
        extension = receive_exact(connection, 2)
        length = int.from_bytes(extension, "big")
    elif length == 127:
        extension = receive_exact(connection, 8)
        length = int.from_bytes(extension, "big")
    if length > 16 * 1024 * 1024:
        raise RuntimeError("oversized WebSocket frame in CDP audit proxy")
    mask = receive_exact(connection, 4) if masked else b""
    wire_payload = receive_exact(connection, length)
    payload = wire_payload
    if masked:
        payload = bytes(value ^ mask[index % 4] for index, value in enumerate(wire_payload))
    return header + extension + mask + wire_payload, opcode, payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lantern", default="target/debug/lantern")
    parser.add_argument("--output-dir", default=".smoogle/browser-contracts")
    args = parser.parse_args()

    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    evidence_path = output / "evidence.json"
    evidence = {
        "schema_version": 1,
        "result": "fail",
        "run_id": str(uuid.uuid4()),
        "started_at": datetime.now(timezone.utc).isoformat(),
        "scope": "Real-Chromium interaction contracts; not visual or hardware qualification.",
        "fixture": None,
        "source_revision": None,
        "lantern_capabilities": None,
        "browser": {},
        "environment": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "input_audit": {
            "transport": "loopback-websocket-proxy",
            "recorded_data": "CDP Input method names only",
        },
        "viewport": {},
        "self_tests": [],
        "cases": [],
    }
    write_evidence(evidence_path, evidence)

    lantern = Path(args.lantern).resolve()
    chromium_value = os.environ.get("LANTERN_CHROMIUM")
    if not chromium_value:
        preflight_error(
            parser, evidence_path, evidence,
            "LANTERN_CHROMIUM must name an explicit Chromium binary",
        )
    chromium = Path(chromium_value).resolve()
    if not lantern.is_file() or not os.access(lantern, os.X_OK):
        preflight_error(parser, evidence_path, evidence, f"Lantern is not executable: {lantern}")
    if not chromium.is_file() or not os.access(chromium, os.X_OK):
        preflight_error(
            parser, evidence_path, evidence,
            f"LANTERN_CHROMIUM is not executable: {chromium}",
        )

    fixture_bytes = FIXTURE.read_bytes()
    evidence["fixture"] = {
        "path": str(FIXTURE.relative_to(ROOT)),
        "sha256": hashlib.sha256(fixture_bytes).hexdigest(),
    }
    evidence["source_revision"] = source_revision()
    evidence["lantern_capabilities"] = command_json(
        [str(lantern), "capabilities", "--json"]
    )
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    suite_started = time.monotonic()

    try:
        with tempfile.TemporaryDirectory(prefix="lantern-browser-contracts-") as profile:
            log_path = output / "chromium.log"
            with log_path.open("wb") as browser_log:
                browser = subprocess.Popen(
                    [
                        str(chromium),
                        "--headless=new",
                        "--remote-debugging-address=127.0.0.1",
                        "--remote-debugging-port=0",
                        f"--user-data-dir={profile}",
                        "--no-first-run",
                        "--no-default-browser-check",
                        "--window-size=1000,700",
                        "about:blank",
                    ],
                    stdout=browser_log,
                    stderr=browser_log,
                )
                try:
                    endpoint = await_endpoint(browser, Path(profile), 12)
                    with urllib.request.urlopen(endpoint + "/json/version", timeout=3) as response:
                        version = json.load(response)
                    evidence["browser"] = {
                        "product": version.get("Browser"),
                        "protocol_version": version.get("Protocol-Version"),
                        "executable": str(chromium),
                        "profile": "fresh-temporary-directory",
                        "headless": "new",
                        "requested_window_size": {"width": 1000, "height": 700},
                    }
                    audit = CdpAuditProxy(endpoint)
                    audit_thread = threading.Thread(target=audit.serve_forever, daemon=True)
                    audit_thread.start()
                    try:
                        audit_endpoint = f"http://127.0.0.1:{audit.server_port}"
                        verify_audit_guards(audit, evidence)
                        run_suite(
                            lantern,
                            audit_endpoint,
                            f"http://127.0.0.1:{server.server_port}",
                            evidence,
                            suite_started,
                            audit,
                        )
                        evidence["result"] = "pass"
                    finally:
                        audit.shutdown()
                        audit.server_close()
                        audit_thread.join(timeout=3)
                finally:
                    stop_browser(browser)
    except Exception as error:
        evidence["result"] = "fail"
        evidence["failure"] = str(error)
        active_case = evidence.pop("active_case", None)
        if active_case:
            evidence["cases"].append(
                {"case": active_case, "verdict": "fail", "failure": str(error)}
            )
        raise
    finally:
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=3)
        evidence["elapsed_ms"] = round((time.monotonic() - suite_started) * 1000)
        evidence["finished_at"] = datetime.now(timezone.utc).isoformat()
        write_evidence(evidence_path, evidence)

    print(f"Browser contracts passed; evidence: {evidence_path}")


def write_evidence(path, evidence):
    path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")


def preflight_error(parser, path, evidence, message):
    evidence["failure"] = message
    evidence["finished_at"] = datetime.now(timezone.utc).isoformat()
    write_evidence(path, evidence)
    parser.error(message)


def source_revision():
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=3,
        check=True,
    )
    dirty = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=3,
        check=True,
    )
    return {"commit": result.stdout.strip(), "dirty": bool(dirty.stdout.strip())}


def command_json(command, expected_exit=0):
    value, actual_exit, stdout, stderr = command_json_result(command)
    if actual_exit != expected_exit:
        raise AssertionError(
            f"unexpected exit {actual_exit}, expected {expected_exit}: {' '.join(command)}; "
            f"stdout={stdout!r}; stderr={stderr!r}"
        )
    return value


def command_json_result(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=COMMAND_TIMEOUT_SECONDS)
    stream = result.stdout if result.stdout.strip() else result.stderr
    try:
        value = json.loads(stream)
    except json.JSONDecodeError as error:
        raise AssertionError(f"non-JSON output from {' '.join(command)}: {stream!r}") from error
    return value, result.returncode, result.stdout, result.stderr


def await_endpoint(browser, profile, timeout_seconds):
    active_port = profile / "DevToolsActivePort"
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if browser.poll() is not None:
            raise RuntimeError(f"Chromium exited during startup with {browser.returncode}")
        if active_port.exists():
            port = active_port.read_text().splitlines()[0]
            endpoint = f"http://127.0.0.1:{port}"
            try:
                with urllib.request.urlopen(endpoint + "/json/version", timeout=1):
                    return endpoint
            except OSError:
                pass
        time.sleep(0.05)
    raise TimeoutError("Chromium did not publish a ready CDP endpoint")


def stop_browser(browser):
    if browser.poll() is not None:
        return
    browser.terminate()
    try:
        browser.wait(timeout=5)
    except subprocess.TimeoutExpired:
        browser.kill()
        browser.wait(timeout=3)


def assert_input_contract(case, action, dispatched, commands):
    expected_method = {
        "click": "Input.dispatchMouseEvent",
        "hover": "Input.dispatchMouseEvent",
        "wheel": "Input.dispatchMouseEvent",
        "drag": "Input.dispatchMouseEvent",
        "type": "Input.insertText",
        "key": "Input.dispatchKeyEvent",
    }[action]
    if not dispatched and commands:
        raise AssertionError(f"{case}: forbidden CDP input commands observed: {commands}")
    if dispatched and expected_method not in commands:
        raise AssertionError(
            f"{case}: dispatch was reported without audited {expected_method}: {commands}"
        )


def verify_audit_guards(audit, evidence):
    for label, action, method in (
        ("forbidden-pointer", "click", "Input.dispatchMouseEvent"),
        ("forbidden-text", "type", "Input.insertText"),
        ("forbidden-key", "key", "Input.dispatchKeyEvent"),
    ):
        checkpoint = audit.checkpoint()
        audit.record(json.dumps({"id": -1, "method": method}).encode())
        try:
            assert_input_contract(label, action, False, audit.commands_since(checkpoint))
        except AssertionError:
            evidence["self_tests"].append(
                {"case": label, "injected_method": method, "verdict": "pass"}
            )
        else:
            raise AssertionError(f"audit guard accepted deliberate {method} injection")


def run_suite(lantern, endpoint, fixture_base, evidence, suite_started, audit):
    def invoke(*arguments, expected_exit=0):
        if time.monotonic() - suite_started > SUITE_TIMEOUT_SECONDS:
            raise TimeoutError(f"browser contract suite exceeded {SUITE_TIMEOUT_SECONDS} seconds")
        started = time.monotonic()
        value = command_json(
            [str(lantern), *arguments, "--endpoint", endpoint, "--json"], expected_exit=expected_exit
        )
        return value, round((time.monotonic() - started) * 1000)

    def invoke_retained(*arguments):
        if time.monotonic() - suite_started > SUITE_TIMEOUT_SECONDS:
            raise TimeoutError(f"browser contract suite exceeded {SUITE_TIMEOUT_SECONDS} seconds")
        started = time.monotonic()
        value, actual_exit, stdout, stderr = command_json_result(
            [str(lantern), *arguments, "--endpoint", endpoint, "--json"]
        )
        return (
            value,
            round((time.monotonic() - started) * 1000),
            actual_exit,
            stdout,
            stderr,
        )

    def page_title():
        page, _ = invoke("page")
        return page["page"]["title"]

    def navigate(scenario):
        opened, _ = invoke("open", f"{fixture_base}/?case={scenario}")
        if not opened.get("ok"):
            raise AssertionError(f"navigation failed for {scenario}: {opened}")
        ready, _ = invoke("wait", "ready", "--state", "complete", "--timeout-ms", "2000")
        if not ready.get("wait", {}).get("matched"):
            raise AssertionError(f"fixture did not become ready for {scenario}: {ready}")
        title = page_title()
        parts = title.rsplit(":", 1)
        if len(parts) != 2 or "x" not in parts[1]:
            raise AssertionError(f"fixture did not report its viewport: {title}")
        width, height = (int(item) for item in parts[1].split("x", 1))
        evidence["viewport"] = {
            "width": width,
            "height": height,
            "source": "fixture window.innerWidth/innerHeight",
        }
        return title

    def interaction(case, scenario, command, expected_exit, dispatched, error, expected_state):
        evidence["active_case"] = case
        before = navigate(scenario)
        checkpoint = audit.checkpoint()
        value, elapsed_ms, actual_exit, stdout, stderr = invoke_retained(*command)
        summary = value.get("interaction", {})
        input_commands = audit.commands_since(checkpoint)
        after = page_title()
        expected_title = f"{scenario}:{expected_state}:" + before.rsplit(":", 1)[1]
        failures = []
        if actual_exit != expected_exit:
            failures.append(f"exit {actual_exit}, expected {expected_exit}")
        if summary.get("dispatched") is not dispatched:
            failures.append(
                f"dispatched {summary.get('dispatched')!r}, expected {dispatched!r}"
            )
        if summary.get("immediate_error") != error:
            failures.append(
                f"immediate_error {summary.get('immediate_error')!r}, expected {error!r}"
            )
        try:
            assert_input_contract(case, command[0], dispatched, input_commands)
        except AssertionError as audit_error:
            failures.append(str(audit_error))
        if after != expected_title:
            failures.append(f"application state {after!r}, expected {expected_title!r}")
        case_evidence = {
            "case": case,
            "command": command[0],
            "strict": "--strict" in command,
            "exit_code": actual_exit,
            "expected_exit_code": expected_exit,
            "actual_exit_code": actual_exit,
            "actual_stdout": stdout,
            "actual_stderr": stderr,
            "actual_output": value,
            "dispatched": summary.get("dispatched"),
            "dispatch_state": summary.get("dispatch_state"),
            "timed_out": summary.get("timed_out"),
            "immediate_error": summary.get("immediate_error"),
            "audited_input_commands": input_commands,
            "application_state_before": before,
            "application_state_after": after,
            "elapsed_ms": elapsed_ms,
            "verdict": "fail" if failures else "pass",
        }
        if failures:
            case_evidence["failure"] = "; ".join(failures)
        evidence["cases"].append(case_evidence)
        evidence.pop("active_case")
        if failures:
            raise AssertionError(f"{case}: {case_evidence['failure']}")

    invoke("doctor")
    interaction(
        "fresh-browser-focus-listener-disables", "focus-disables",
        [
            "type", "--selector", "#target", "--text", "lantern text",
            "--timeout-ms", "1000", "--strict",
        ],
        1, False, "element_disabled", "ready",
    )
    interaction(
        "unique-click", "unique",
        ["click", "--selector", "#target", "--timeout-ms", "2000", "--strict"],
        0, True, None, "clicked",
    )
    interaction(
        "duplicate-selector-strict", "duplicate",
        ["click", "--selector", ".target", "--timeout-ms", "1000", "--strict"],
        1, False, "ambiguous_selector", "ready",
    )
    interaction(
        "duplicate-selector-legacy", "duplicate",
        ["click", "--selector", ".target", "--timeout-ms", "1000"],
        0, False, "ambiguous_selector", "ready",
    )
    for scenario in ("native-disabled", "fieldset-disabled", "aria-disabled"):
        interaction(
            scenario, scenario,
            ["click", "--selector", "#target", "--timeout-ms", "1000", "--strict"],
            1, False, "element_disabled", "ready",
        )
    interaction(
        "hover-disabled", "hover-disabled",
        ["hover", "--selector", "#target", "--timeout-ms", "2000", "--strict"],
        0, True, None, "hovered",
    )
    for scenario in ("readonly", "noneditable"):
        interaction(
            scenario, scenario,
            [
                "type", "--selector", "#target", "--text", "lantern text",
                "--timeout-ms", "1000", "--strict",
            ],
            1, False, "element_not_editable", "ready",
        )
    interaction(
        "focus-listener-disables", "focus-disables",
        [
            "type", "--selector", "#target", "--text", "lantern text",
            "--timeout-ms", "1000", "--strict",
        ],
        1, False, "element_disabled", "ready",
    )
    interaction(
        "focus-listener-readonly", "focus-readonly",
        [
            "type", "--selector", "#target", "--text", "lantern text",
            "--timeout-ms", "1000", "--strict",
        ],
        1, False, "element_not_editable", "ready",
    )
    interaction(
        "nonfocusable-key", "nonfocusable-key",
        ["key", "--selector", "#target", "--key", "Enter", "--timeout-ms", "1000", "--strict"],
        1, False, "element_not_focused", "ready",
    )
    interaction(
        "occluded-target", "occluded",
        ["click", "--selector", "#target", "--timeout-ms", "1000", "--strict"],
        1, False, "element_occluded", "ready",
    )
    interaction(
        "offscreen-scroll", "offscreen",
        ["click", "--selector", "#target", "--timeout-ms", "2000", "--strict"],
        0, True, None, "clicked",
    )
    interaction(
        "padded-empty-button", "padded-empty",
        ["click", "--selector", "#target", "--timeout-ms", "2000", "--strict"],
        0, True, None, "clicked",
    )
    interaction(
        "unstable-moving-target", "unstable",
        ["click", "--selector", "#moving", "--timeout-ms", "1000", "--strict"],
        1, False, "element_unstable", "ready",
    )
    interaction(
        "delayed-rendering", "delayed",
        ["click", "--selector", "#target", "--timeout-ms", "2000", "--strict"],
        0, True, None, "clicked",
    )
    interaction(
        "type-and-focus", "text-focus-key",
        ["type", "--selector", "#target", "--text", "lantern text", "--timeout-ms", "2000", "--strict"],
        0, True, None, "typed",
    )
    evidence["active_case"] = "focused-key-body-shortcut"
    before_key = page_title()
    key_checkpoint = audit.checkpoint()
    key_value, key_ms, key_exit, key_stdout, key_stderr = invoke_retained(
        "key", "--selector", "body", "--key", "ArrowDown",
        "--timeout-ms", "2000", "--strict",
    )
    key_summary = key_value.get("interaction", {})
    key_input_commands = audit.commands_since(key_checkpoint)
    after_key = page_title()
    key_failures = []
    if key_exit != 0:
        key_failures.append(f"exit {key_exit}, expected 0")
    if not key_summary.get("dispatched"):
        key_failures.append(
            f"dispatched {key_summary.get('dispatched')!r}, expected true"
        )
    try:
        assert_input_contract(
            "focused-key-body-shortcut", "key", True, key_input_commands
        )
    except AssertionError as audit_error:
        key_failures.append(str(audit_error))
    if after_key.split(":")[1] != "key-focused":
        key_failures.append(
            f"application state {after_key!r}, expected focused key delivery"
        )
    key_evidence = {
        "case": "focused-key-body-shortcut",
        "command": "key",
        "strict": True,
        "exit_code": key_exit,
        "expected_exit_code": 0,
        "actual_exit_code": key_exit,
        "actual_stdout": key_stdout,
        "actual_stderr": key_stderr,
        "actual_output": key_value,
        "dispatched": key_summary.get("dispatched"),
        "dispatch_state": key_summary.get("dispatch_state"),
        "timed_out": key_summary.get("timed_out"),
        "immediate_error": key_summary.get("immediate_error"),
        "audited_input_commands": key_input_commands,
        "application_state_before": before_key,
        "application_state_after": after_key,
        "elapsed_ms": key_ms,
        "verdict": "fail" if key_failures else "pass",
    }
    if key_failures:
        key_evidence["failure"] = "; ".join(key_failures)
    evidence["cases"].append(key_evidence)
    evidence.pop("active_case")
    if key_failures:
        raise AssertionError(
            f"focused-key-body-shortcut: {key_evidence['failure']}"
        )

    interaction(
        "pointer-hover", "pointer",
        ["hover", "--selector", "#surface", "--timeout-ms", "2000", "--strict"],
        0, True, None, "hovered",
    )
    interaction(
        "pointer-wheel", "pointer",
        ["wheel", "--selector", "#surface", "--dy", "80", "--timeout-ms", "2000", "--strict"],
        0, True, None, "wheeled",
    )
    interaction(
        "pointer-drag", "pointer",
        [
            "drag", "--selector", "#surface", "--dx", "80", "--dy", "30",
            "--duration-ms", "120", "--timeout-ms", "2000", "--strict",
        ],
        0, True, None, "dragged",
    )


if __name__ == "__main__":
    main()
