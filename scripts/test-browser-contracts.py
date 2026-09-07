#!/usr/bin/env python3
"""Run deterministic Lantern interaction contracts against disposable Chromium."""

import argparse
from datetime import datetime, timezone
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import time
import urllib.request
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lantern", default="target/debug/lantern")
    parser.add_argument("--output-dir", default=".smoogle/browser-contracts")
    args = parser.parse_args()

    lantern = Path(args.lantern).resolve()
    chromium_value = os.environ.get("LANTERN_CHROMIUM")
    if not chromium_value:
        parser.error("LANTERN_CHROMIUM must name an explicit Chromium binary")
    chromium = Path(chromium_value).resolve()
    if not lantern.is_file() or not os.access(lantern, os.X_OK):
        parser.error(f"Lantern is not executable: {lantern}")
    if not chromium.is_file() or not os.access(chromium, os.X_OK):
        parser.error(f"LANTERN_CHROMIUM is not executable: {chromium}")

    output = Path(args.output_dir).resolve()
    output.mkdir(parents=True, exist_ok=True)
    evidence_path = output / "evidence.json"
    fixture_bytes = FIXTURE.read_bytes()
    fixture_identity = {
        "path": str(FIXTURE.relative_to(ROOT)),
        "sha256": hashlib.sha256(fixture_bytes).hexdigest(),
    }
    evidence = {
        "schema_version": 1,
        "result": "fail",
        "run_id": str(uuid.uuid4()),
        "started_at": datetime.now(timezone.utc).isoformat(),
        "scope": "Real-Chromium interaction contracts; not visual or hardware qualification.",
        "fixture": fixture_identity,
        "source_revision": None,
        "lantern_capabilities": None,
        "browser": {},
        "viewport": {},
        "cases": [],
    }
    evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
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
                    run_suite(
                        lantern,
                        endpoint,
                        f"http://127.0.0.1:{server.server_port}",
                        evidence,
                        suite_started,
                    )
                    evidence["result"] = "pass"
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
        evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")

    print(f"Browser contracts passed; evidence: {evidence_path}")


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
    result = subprocess.run(command, capture_output=True, text=True, timeout=COMMAND_TIMEOUT_SECONDS)
    if result.returncode != expected_exit:
        raise AssertionError(
            f"unexpected exit {result.returncode}, expected {expected_exit}: {' '.join(command)}; "
            f"stdout={result.stdout!r}; stderr={result.stderr!r}"
        )
    stream = result.stdout if result.stdout.strip() else result.stderr
    try:
        value = json.loads(stream)
    except json.JSONDecodeError as error:
        raise AssertionError(f"non-JSON output from {' '.join(command)}: {stream!r}") from error
    return value


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


def run_suite(lantern, endpoint, fixture_base, evidence, suite_started):
    def invoke(*arguments, expected_exit=0):
        if time.monotonic() - suite_started > SUITE_TIMEOUT_SECONDS:
            raise TimeoutError(f"browser contract suite exceeded {SUITE_TIMEOUT_SECONDS} seconds")
        started = time.monotonic()
        value = command_json(
            [str(lantern), *arguments, "--endpoint", endpoint, "--json"], expected_exit=expected_exit
        )
        return value, round((time.monotonic() - started) * 1000)

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
        value, elapsed_ms = invoke(*command, expected_exit=expected_exit)
        summary = value.get("interaction", {})
        if summary.get("dispatched") is not dispatched:
            raise AssertionError(f"{case}: unexpected dispatch state: {value}")
        if summary.get("immediate_error") != error:
            raise AssertionError(f"{case}: unexpected immediate error: {value}")
        after = page_title()
        expected_title = f"{scenario}:{expected_state}:" + before.rsplit(":", 1)[1]
        if after != expected_title:
            raise AssertionError(f"{case}: application state {after!r}, expected {expected_title!r}")
        evidence["cases"].append(
            {
                "case": case,
                "command": command[0],
                "strict": "--strict" in command,
                "exit_code": expected_exit,
                "dispatched": summary.get("dispatched"),
                "dispatch_state": summary.get("dispatch_state"),
                "timed_out": summary.get("timed_out"),
                "immediate_error": summary.get("immediate_error"),
                "application_state_before": before,
                "application_state_after": after,
                "elapsed_ms": elapsed_ms,
                "verdict": "pass",
            }
        )
        evidence.pop("active_case")

    invoke("doctor")
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
    key_value, key_ms = invoke(
        "key", "--selector", "body", "--key", "ArrowDown",
        "--timeout-ms", "2000", "--strict",
    )
    key_summary = key_value.get("interaction", {})
    after_key = page_title()
    if not key_summary.get("dispatched") or after_key.split(":")[1] != "key-focused":
        raise AssertionError(f"key was not delivered to the focused input: {key_value}")
    evidence["cases"].append({
        "case": "focused-key-body-shortcut",
        "command": "key",
        "strict": True,
        "exit_code": 0,
        "dispatched": key_summary.get("dispatched"),
        "dispatch_state": key_summary.get("dispatch_state"),
        "timed_out": key_summary.get("timed_out"),
        "immediate_error": key_summary.get("immediate_error"),
        "application_state_before": before_key,
        "application_state_after": after_key,
        "elapsed_ms": key_ms,
        "verdict": "pass",
    })
    evidence.pop("active_case")

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
