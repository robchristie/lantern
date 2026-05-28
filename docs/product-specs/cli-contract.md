# CLI Contract

This contract defines the implemented Lantern command surface. It is the source of truth for `lantern-cli` until a later ExecPlan intentionally broadens the product scope.

The shared human-output, JSON ordering, redaction, truncation, DOM, console, network, screenshot, persistence, and test policy lives in `docs/product-specs/output-policy.md`. This contract defines command-specific fields and current implemented behavior.

## Scope

The command surface is bounded browser inspection, selected-page navigation, selected-page console feedback, selected-page network failure feedback, one bounded session-observation flow, explicit selected-page screenshot capture, selected-element click/text-entry/key interactions against an explicit Chromium DevTools Protocol endpoint, and an opt-in managed disposable browser container lifecycle. Endpoint-based commands do not launch Chromium, manage browser lifetime, create browser tabs, evaluate arbitrary JavaScript, perform full network capture or HAR generation, crawl pages, run test-runner abstractions, or persist browser artifacts beyond an explicitly requested screenshot output path. DOM inspection is limited to the bounded `lantern dom` summary documented below; full HTML dumps and broad DOM artifact capture remain out of scope.

## Command Surface

Implemented commands:

- `lantern doctor`
- `lantern targets`
- `lantern page`
- `lantern dom`
- `lantern open <URL>`
- `lantern wait <ready|url|selector|text|quiet>`
- `lantern console`
- `lantern network`
- `lantern flow --timeout-ms <MS> [--quiet-ms <MS>] [--open <URL>]`
- `lantern screenshot --output <PATH>`
- `lantern click --selector <CSS_SELECTOR> --timeout-ms <MS>`
- `lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>`
- `lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>`
- `lantern browser start`
- `lantern browser list`
- `lantern browser status <ID>`
- `lantern browser endpoint <ID>`
- `lantern browser stop <ID>`
- `lantern browser prune`

Reserved for future milestones and not implemented yet:

- persistent authenticated managed browser profiles
- JavaScript evaluation
- full network inspection, HAR, request body, or response body commands
- daemon, MCP, TUI, or web UI commands

Each implemented command accepts the shared flags below. Unknown commands, unknown flags, invalid flag values, and unsupported combinations must fail with exit code `2`.

Lifecycle commands are explicit. Existing CDP commands resolve only `--endpoint`
or `LANTERN_CDP_ENDPOINT` and must not auto-start or auto-select a managed
browser.

## Shared Flags

### `--endpoint <URL>`

Sets the Chromium CDP HTTP endpoint for this invocation.

Accepted shapes:

- `http://127.0.0.1:9222`
- `http://localhost:9222`
- `http://[::1]:9222`

The value may include a path prefix when Chromium is reverse-proxied locally, but Lantern resolves browser metadata and target listing relative to that base endpoint. Query strings and fragments are invalid for the base endpoint.

The first milestone only supports local HTTP endpoints. `https://`, `ws://`, `wss://`, non-local hosts, credentials in URLs, and unix sockets are out of scope unless a later security review expands the threat model.

### `--json`

Emits stable machine-readable JSON to stdout. Human-readable progress text must not be mixed into stdout when `--json` is set. Errors still go to stderr as described in [Error Output](#error-output).

### `--no-redact`

Disables URL shape redaction and text truncation for fields that are otherwise redacted by default. This flag is intended for local debugging only. It must never change which browser artifacts Lantern persists, because implemented commands do not persist browser artifacts.

When `--no-redact` is absent, commands must avoid full URLs and other high-risk browser data by default.

For `lantern dom`, `--no-redact` may relax truncation and URL-shape redaction only where `docs/product-specs/output-policy.md` allows it. It must not cause Lantern to collect or print values the DOM policy forbids by design, including script contents, style contents, hidden form values, cookies, local storage, or credential-like attribute values.

### `--target-id <CDP_TARGET_ID>`

Selects an exact CDP page target id for commands that operate on one page target.

Supported commands:

- `lantern page`
- `lantern dom`
- `lantern open`
- `lantern wait`
- `lantern console`
- `lantern network`
- `lantern flow`
- `lantern screenshot`
- `lantern click`
- `lantern type`
- `lantern key`

The value must match the complete `targets[].id` value from `lantern targets`; short id prefixes are not accepted. Supplying `--target-id` to commands that do not operate on one page target, such as `doctor` or `targets`, is an unsupported combination and fails with exit code `2`.

### `-h`, `--help`

Prints command or subcommand help and exits with code `0`.

### `-V`, `--version`

Prints the Lantern CLI version and exits with code `0`.

## Managed Browser Lifecycle

Purpose: create and manage isolated disposable local Chromium/CDP containers for
concurrent agents while preserving endpoint-pure inspection commands.

Supported forms:

- `lantern browser start [--runtime podman|docker] [--image IMAGE] [--id ID] [--wait-ms MS]`
- `lantern browser list`
- `lantern browser status <ID>`
- `lantern browser endpoint <ID>`
- `lantern browser stop <ID>`
- `lantern browser prune`

Default `start` behavior:

- chooses `podman` when available, otherwise `docker`, unless `--runtime` is set
- uses image `localhost/lantern-browser-cdp:stable`
- creates a unique id when `--id` is omitted
- creates a dedicated profile directory under `.smoogle/lantern/browser-instances/<id>/profile`
- runs the container detached
- publishes container CDP port `9222` to a runtime-assigned random host port on `127.0.0.1`
- also publishes VNC `5900` and noVNC `6080` to random host-loopback ports when the runtime provides those mappings
- labels containers with `dev.lantern.managed=true` and `dev.lantern.instance-id=<ID>`
- for Docker, runs the container as the host profile-directory owner, sets `HOME=/tmp`, and passes `CHROME_NO_SANDBOX=1` so the image adds Chromium `--no-sandbox`
- waits for `/json/version` readiness before reporting success

Human output should include the id, status, runtime, endpoint, noVNC URL when
available, and profile path. JSON output shape for single-instance commands:

```json
{
  "schema_version": 1,
  "command": "browser_start",
  "ok": true,
  "instance": {
    "id": "lantern-browser-123",
    "name": "lantern-browser-123",
    "runtime": "podman",
    "image": "localhost/lantern-browser-cdp:stable",
    "status": "running",
    "endpoint": "http://127.0.0.1:43123",
    "cdp_host_port": 43123,
    "novnc_url": "http://127.0.0.1:43124/vnc.html",
    "profile_dir": ".smoogle/lantern/browser-instances/lantern-browser-123/profile"
  }
}
```

`browser list` returns:

```json
{
  "schema_version": 1,
  "command": "browser_list",
  "ok": true,
  "instances": []
}
```

`browser prune` returns:

```json
{
  "schema_version": 1,
  "command": "browser_prune",
  "ok": true,
  "pruned": ["lantern-browser-123"]
}
```

Lifecycle commands must not accept `--endpoint`, `--target-id`, `--no-redact`,
navigation, wait, DOM, screenshot, or interaction flags. Endpoint commands must
not accept lifecycle flags.

Command-specific error cases:

- no installed selected runtime: exit code `1`, error code `browser_runtime_unavailable`
- runtime command failure: exit code `1`, error code `browser_runtime_failed`
- CDP port was not published: exit code `1`, error code `browser_port_missing`
- managed browser readiness timeout: exit code `1`, error code `browser_not_ready`
- state read/write failure: exit code `1`, error code `browser_state_failed`

Security behavior:

- CDP host publishing is loopback-only by default.
- Managed profiles are local untracked runtime state and are disposable by
  default.
- The lifecycle registry stores endpoints, runtime metadata, and profile paths;
  it must not store cookies, local storage, DOM dumps, screenshots, console
  payloads, network bodies, or full browser artifacts.

## Endpoint Resolution

Lantern resolves the CDP endpoint in this order:

1. `--endpoint <URL>`
2. `LANTERN_CDP_ENDPOINT`

If neither source is set, endpoint resolution fails with exit code `2` and a short stderr message explaining both supported configuration sources.

If both sources are set, `--endpoint` wins for that invocation.

Lantern must validate the resolved endpoint before attempting CDP HTTP calls. Validation failures are usage/configuration errors with exit code `2`; unreachable or unhealthy but syntactically valid endpoints are runtime failures with exit code `1`.

Lantern does not read an endpoint config file in the first milestone. A later task may add a config file only after specifying the file path, precedence, and redaction behavior.

## Commands

### `lantern doctor`

Purpose: verify that the resolved CDP endpoint is reachable and identify the attached browser.

CDP inputs:

- `GET /json/version`

Human output should include:

- resolved endpoint host and port, without credentials or query details
- reachability status
- browser product/version when available
- protocol version when available
- WebSocket debugger URL presence, not the full URL by default

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "doctor",
  "ok": true,
  "endpoint": {
    "source": "flag",
    "display": "http://127.0.0.1:9222"
  },
  "browser": {
    "product": "Chrome/123.0.0.0",
    "protocol_version": "1.3"
  },
  "websocket": {
    "available": true
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `endpoint.source`
- `endpoint.display`
- `browser.product`
- `browser.protocol_version`
- `websocket.available`

`browser.product` and `browser.protocol_version` may be `null` when the endpoint responds without those fields.

### `lantern targets`

Purpose: list Chromium targets using sanitized, bounded metadata.

CDP inputs:

- `GET /json/list`

Human output should include one line per target with:

- target id short form
- target type
- title, truncated by default
- URL shape, not the full URL by default
- attached status when available

Default target ordering:

1. `page` targets before non-page targets
2. active or attached targets before detached targets when the field is available
3. original CDP list order as the final tiebreaker

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "targets",
  "ok": true,
  "targets": [
    {
      "id": "ABCD1234",
      "type": "page",
      "title": "Example",
      "url_shape": "https://example.test/path",
      "attached": true
    }
  ]
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `targets`
- `targets[].id`
- `targets[].type`
- `targets[].title`
- `targets[].url_shape`
- `targets[].attached`

`targets[].title`, `targets[].url_shape`, and `targets[].attached` may be `null` when unavailable. `targets[].id` must use the CDP target id exactly in JSON output; human output may abbreviate it.

### `lantern page`

Purpose: summarize one page target with concise state useful to a coding agent.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint when later implementation needs live page state.

Target selection:

`lantern page` accepts `--target-id <CDP_TARGET_ID>` to select a page target by exact CDP target id.

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

The selector is intentionally named `--target-id` rather than `--target`, so later commands can distinguish CDP target ids from CSS selectors or other page selectors. The value is an exact, case-sensitive match against `targets[].id`; Lantern does not accept short target id prefixes.

Human output should include:

- selected target id short form
- title, truncated by default
- URL shape, not the full URL by default
- loading state when available

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "page",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path",
    "loading_state": "complete"
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `page.loading_state`

`page.title`, `page.url_shape`, and `page.loading_state` may be `null` when unavailable.

### `lantern open <URL>`

Purpose: navigate the selected existing page target to an operator-supplied URL.

`lantern open` does not launch Chromium, create a new page, close pages, manage profiles, or retry browser startup. It operates only on a page target that already exists in the configured local CDP endpoint.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- `Page.navigate` with the supplied URL.
- Bounded follow-up page-state reads, such as `Page.getNavigationHistory` and a fixed `document.readyState` read, when CDP makes those values available.

Allowed URL schemes:

- `http://...`
- `https://...`
- `about:blank`

The URL must be absolute. `about:` is accepted only for the exact URL `about:blank`. `file:`, `data:`, `javascript:`, `blob:`, `chrome:`, `devtools:`, relative URLs, credentials-only forms, and malformed URLs fail with exit code `2` and error code `url_invalid`.

Target selection:

`lantern open` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern open` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Human output should include:

- selected target id short form
- requested URL shape, not the full URL by default
- final URL shape when available, not the full URL by default
- loading state when available

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "open",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/old"
  },
  "navigation": {
    "requested_url_shape": "https://example.test/new",
    "final_url_shape": "https://example.test/new",
    "loading_state": "loading"
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `navigation.requested_url_shape`
- `navigation.final_url_shape`
- `navigation.loading_state`

`page.title`, `page.url_shape`, `navigation.final_url_shape`, and `navigation.loading_state` may be `null` when unavailable.

Command-specific error cases:

- invalid or unsupported requested URL: exit code `2`, error code `url_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP navigation failure: exit code `1`, error code `navigation_failed`
- malformed CDP navigation or page-state response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- `requested_url_shape` and `final_url_shape` use the URL shape policy by default.
- query strings, fragments, credentials, and sensitive-looking path segments are omitted or redacted by default.
- `about:blank` is emitted as `about:blank`.
- `--no-redact` may expose the full requested and final URLs for this invocation only.
- CDP navigation error text is not included in JSON success output and is not echoed in stable error output by default.

### `lantern wait <ready|url|selector|text|quiet>`

Purpose: wait a bounded amount of time for useful selected-page state after navigation or interaction.

`lantern wait` is a polling and event-quietness helper, not a test runner. It does not launch Chromium, navigate pages, dispatch interactions, execute operator-supplied JavaScript, persist artifacts, or keep a long-running watch open. The command returns a condition result even when the condition is not met before timeout; unmet conditions are reported in stdout with `matched=false` and `timed_out=true`, while CLI usage, target, endpoint, and CDP failures use the normal non-zero error path.

Supported conditions:

- `lantern wait ready --timeout-ms <MS> [--state <loading|interactive|complete>]`
- `lantern wait url --timeout-ms <MS> --url-shape <URL_SHAPE>`
- `lantern wait selector --timeout-ms <MS> --selector <CSS_SELECTOR>`
- `lantern wait text --timeout-ms <MS> --selector <CSS_SELECTOR> --text <TEXT>`
- `lantern wait quiet --timeout-ms <MS> --quiet-ms <MS>`

Timeouts are explicit and bounded. `--timeout-ms` is required for every wait condition and must be from `1` through `30000`. `--quiet-ms` is required only for `quiet`, must be from `1` through `30000`, and must be less than or equal to `--timeout-ms`.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- For `ready`, bounded `Runtime.evaluate` reads of `document.readyState`.
- For `url`, bounded `Page.getNavigationHistory` reads of the current entry URL.
- For `selector`, bounded `Runtime.evaluate` reads of `document.querySelector(<selector>)`.
- For `text`, bounded `Runtime.evaluate` reads of the selected element's `textContent`.
- For `quiet`, `Runtime.enable`, `Log.enable`, `Page.enable`, best-effort `Page.setLifecycleEventsEnabled`, and bounded event reads until no Runtime, Log, or Page lifecycle event is observed for the requested quiet period.

Target selection:

`lantern wait` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern wait` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Human output should include:

- selected target id short form
- condition kind
- whether the condition matched
- timeout and elapsed milliseconds
- concise observed state

Recommended human output shape:

```text
wait: ABCD1234 condition=ready matched=true timed_out=false elapsed_ms=214 timeout_ms=5000 observed=complete
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "wait",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "wait": {
    "condition": "ready",
    "matched": true,
    "timed_out": false,
    "elapsed_ms": 214,
    "timeout_ms": 5000,
    "observed": {
      "ready_state": "complete"
    }
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `wait.condition`
- `wait.matched`
- `wait.timed_out`
- `wait.elapsed_ms`
- `wait.timeout_ms`
- `wait.observed`

`page.title` and `page.url_shape` may be `null` when unavailable. `wait.observed` is condition-specific but must remain concise:

- `ready`: `ready_state`
- `url`: `expected_url_shape`, `current_url_shape`
- `selector`: `selector`, `present`
- `text`: `selector`, `expected_text`, `current_text`
- `quiet`: `quiet_ms`, `event_count`, `last_event`

URL shape matching compares against the command's redaction mode. By default, `--url-shape` should be the redacted URL shape Lantern would print elsewhere, such as `https://example.test/reset/:redacted`. With `--no-redact`, URL shape matching uses the full observed URL.

Command-specific error cases:

- missing or invalid wait condition: exit code `2`, error code `usage`
- missing required condition flag: exit code `2`, error code `usage`
- invalid timeout or quiet-period bounds: exit code `2`, error code `wait_timeout_invalid`
- invalid ready state: exit code `2`, error code `usage`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP wait response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- page `url_shape`, URL-condition `current_url_shape`, and `expected_url_shape` follow the URL shape policy by default.
- selector strings and expected/current text snippets are echoed because they are operator-supplied or page-visible condition state; callers should avoid passing secrets in selectors or text.
- `--no-redact` may expose full URLs for URL matching and output for this invocation only.

### `lantern click --selector <CSS_SELECTOR> --timeout-ms <MS>`

Purpose: dispatch one user-like mouse click at the center of the selected element.

`lantern click` is a bounded interaction helper, not a crawler or test runner. It does not launch Chromium, navigate pages, evaluate operator-supplied JavaScript, perform complex gestures, retry application workflows, or infer selectors. The command requires an explicit selector and explicit timeout. If the selector is not found or the element cannot produce a clickable box before the timeout, the command exits successfully with `dispatched=false`, `timed_out=true`, and a concise `immediate_error`; endpoint, target, and CDP failures still use the normal non-zero error path.

Supported form:

- `lantern click --selector <CSS_SELECTOR> --timeout-ms <MS>`

Timeouts are explicit and bounded. `--timeout-ms` is required and must be from `1` through `30000`.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- Bounded `DOM.getDocument` and `DOM.querySelector` reads for the supplied selector.
- `DOM.describeNode` for concise node metadata when available.
- `DOM.getBoxModel` to compute a center point for the element's content quad.
- `Input.dispatchMouseEvent` for one `mouseMoved`, one `mousePressed`, and one `mouseReleased` event at that center point.

Target selection:

`lantern click` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern click` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Human output should include:

- selected target id short form
- selector
- whether the action was dispatched
- timeout and elapsed milliseconds
- concise observed node state
- immediate error when no action was dispatched

Recommended human output shape:

```text
click: ABCD1234 title="Example" url=https://example.test/path selector="[data-testid=save]" dispatched=true timed_out=false elapsed_ms=42 timeout_ms=1000 observed=node=BUTTON point=420,180 inserted_text_length=null key=null key_event_count=null error=null
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "click",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "interaction": {
    "action": "click",
    "selector": "[data-testid=save]",
    "dispatched": true,
    "timed_out": false,
    "elapsed_ms": 42,
    "timeout_ms": 1000,
    "observed": {
      "node_name": "BUTTON",
      "clickable_point": {
        "x": 420.0,
        "y": 180.0
      },
      "inserted_text_length": null,
      "key": null,
      "key_event_count": null,
      "pointer_start": null,
      "pointer_end": null,
      "delta_x": null,
      "delta_y": null,
      "duration_ms": null,
      "input_event_count": null
    },
    "immediate_error": null
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `interaction.action`
- `interaction.selector`
- `interaction.dispatched`
- `interaction.timed_out`
- `interaction.elapsed_ms`
- `interaction.timeout_ms`
- `interaction.observed.node_name`
- `interaction.observed.clickable_point`
- `interaction.observed.inserted_text_length`
- `interaction.observed.key`
- `interaction.observed.key_event_count`
- `interaction.observed.pointer_start`
- `interaction.observed.pointer_end`
- `interaction.observed.delta_x`
- `interaction.observed.delta_y`
- `interaction.observed.duration_ms`
- `interaction.observed.input_event_count`
- `interaction.immediate_error`

`page.title`, `page.url_shape`, `interaction.observed.node_name`, `interaction.observed.clickable_point`, and the pointer-specific fields may be `null` when unavailable or irrelevant. For `click`, `interaction.observed.inserted_text_length`, `interaction.observed.key`, `interaction.observed.key_event_count`, `interaction.observed.pointer_end`, `interaction.observed.delta_x`, `interaction.observed.delta_y`, `interaction.observed.duration_ms`, and `interaction.observed.input_event_count` are always `null`.

Command-specific error cases:

- missing `--selector` or `--timeout-ms`: exit code `2`, error code `usage`
- unsupported `--text` with `click`: exit code `2`, error code `usage`
- invalid timeout bounds: exit code `2`, error code `interaction_timeout_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP interaction response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- page `url_shape` follows the URL shape policy by default.
- selector strings are echoed because they are operator-supplied; callers should avoid passing secrets in selectors.
- click output does not capture DOM text, input values, cookies, storage, headers, request bodies, response bodies, or screenshots.
- `--no-redact` may expose the selected page's full URL metadata for this invocation only.

### `lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>`

Purpose: focus the selected element and insert operator-supplied text.

`lantern type` is a bounded text-entry helper. It does not clear existing values, submit forms, synthesize complex keyboard shortcuts, infer selectors, evaluate operator-supplied JavaScript, or validate application state. The command requires an explicit selector, explicit text, and explicit timeout. If the selector is not found before the timeout, the command exits successfully with `dispatched=false`, `timed_out=true`, and `immediate_error="selector_not_found"`; endpoint, target, and CDP failures still use the normal non-zero error path.

Supported form:

- `lantern type --selector <CSS_SELECTOR> --text <TEXT> --timeout-ms <MS>`

Timeouts are explicit and bounded. `--timeout-ms` is required and must be from `1` through `30000`.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- Bounded `DOM.getDocument` and `DOM.querySelector` reads for the supplied selector.
- `DOM.describeNode` for concise node metadata when available.
- `DOM.focus` on the selected node.
- `Input.insertText` with the supplied text.

Target selection, WebSocket requirements, and target-related error cases are identical to `lantern click`.

Human output should include:

- selected target id short form
- selector
- whether the action was dispatched
- timeout and elapsed milliseconds
- node name and inserted text length
- immediate error when no action was dispatched

Recommended human output shape:

```text
type: ABCD1234 title="Example" url=https://example.test/path selector="input[name=q]" dispatched=true timed_out=false elapsed_ms=39 timeout_ms=1000 observed=node=INPUT point=null inserted_text_length=12 key=null key_event_count=null error=null
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "type",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "interaction": {
    "action": "type",
    "selector": "input[name=q]",
    "dispatched": true,
    "timed_out": false,
    "elapsed_ms": 39,
    "timeout_ms": 1000,
    "observed": {
      "node_name": "INPUT",
      "clickable_point": null,
      "inserted_text_length": 12,
      "key": null,
      "key_event_count": null,
      "pointer_start": null,
      "pointer_end": null,
      "delta_x": null,
      "delta_y": null,
      "duration_ms": null,
      "input_event_count": null
    },
    "immediate_error": null
  }
}
```

Required JSON fields are the same as `lantern click`. For `type`, `interaction.observed.clickable_point`, `interaction.observed.key`, and `interaction.observed.key_event_count` are always `null`, and `interaction.observed.inserted_text_length` is the character count of the operator-supplied text when the action is dispatched.

Command-specific error cases:

- missing `--selector`, `--text`, or `--timeout-ms`: exit code `2`, error code `usage`
- invalid timeout bounds: exit code `2`, error code `interaction_timeout_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP interaction response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- page `url_shape` follows the URL shape policy by default.
- selector strings are echoed because they are operator-supplied; callers should avoid passing secrets in selectors.
- inserted text is sent to Chromium but not echoed back in stdout or stderr. Lantern reports only `inserted_text_length`.
- `type` output does not capture post-entry input values, DOM text, cookies, storage, headers, request bodies, response bodies, or screenshots.
- `--no-redact` may expose the selected page's full URL metadata for this invocation only.

### `lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>`

Purpose: focus the selected element and dispatch one keyboard key press as a `keyDown`/`keyUp` pair.

`lantern key` is a bounded keyboard interaction helper. It does not insert text, synthesize shortcuts, hold keys, repeat keys, infer selectors, evaluate operator-supplied JavaScript, validate application state, or run multi-step interaction sessions. The command requires an explicit selector, explicit key, and explicit timeout. If the selector is not found before the timeout, the command exits successfully with `dispatched=false`, `timed_out=true`, and `immediate_error="selector_not_found"`; endpoint, target, and CDP failures still use the normal non-zero error path.

Supported form:

- `lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>`

Timeouts are explicit and bounded. `--timeout-ms` is required and must be from `1` through `30000`.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- Bounded `DOM.getDocument` and `DOM.querySelector` reads for the supplied selector.
- `DOM.describeNode` for concise node metadata when available.
- `DOM.focus` on the selected node.
- `Input.dispatchKeyEvent` for exactly one `keyDown` and one `keyUp` event using the supplied key.

Target selection, WebSocket requirements, and target-related error cases are identical to `lantern click`.

Human output should include:

- selected target id short form
- selector
- whether the action was dispatched
- timeout and elapsed milliseconds
- node name, requested key, and key event count
- immediate error when no action was dispatched

Recommended human output shape:

```text
key: ABCD1234 title="Example" url=https://example.test/path selector="body" dispatched=true timed_out=false elapsed_ms=20 timeout_ms=1000 observed=node=BODY point=null inserted_text_length=null key=ArrowUp key_event_count=2 error=null
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "key",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "interaction": {
    "action": "key",
    "selector": "body",
    "dispatched": true,
    "timed_out": false,
    "elapsed_ms": 20,
    "timeout_ms": 1000,
    "observed": {
      "node_name": "BODY",
      "clickable_point": null,
      "inserted_text_length": null,
      "key": "ArrowUp",
      "key_event_count": 2,
      "pointer_start": null,
      "pointer_end": null,
      "delta_x": null,
      "delta_y": null,
      "duration_ms": null,
      "input_event_count": null
    },
    "immediate_error": null
  }
}
```

Required JSON fields are the same as `lantern click`. For `key`, `interaction.observed.clickable_point` and `interaction.observed.inserted_text_length` are always `null`; `interaction.observed.key` is the operator-supplied key; and `interaction.observed.key_event_count` is `2` when dispatched or `0` when the selector timed out before dispatch.

Command-specific error cases:

- missing `--selector`, `--key`, or `--timeout-ms`: exit code `2`, error code `usage`
- unsupported `--text` with `key`: exit code `2`, error code `usage`
- unsupported `--key` with commands other than `key`: exit code `2`, error code `usage`
- invalid timeout bounds: exit code `2`, error code `interaction_timeout_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP interaction response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- page `url_shape` follows the URL shape policy by default.
- selector strings and key strings are echoed because they are operator-supplied; callers should avoid passing secrets in selectors or keys.
- `key` output does not capture DOM text, input values, cookies, storage, headers, request bodies, response bodies, or screenshots.
- `--no-redact` may expose the selected page's full URL metadata for this invocation only.

### `lantern hover --selector <CSS_SELECTOR> --timeout-ms <MS>`

Purpose: move the mouse pointer to the center of the selected element without pressing a button.

`lantern hover` is a bounded pointer interaction helper for menus, tooltips, canvases, and visual hover states. It requires an explicit selector and timeout. If the selector is not found or the element cannot produce a content box before the timeout, the command exits successfully with `dispatched=false`, `timed_out=true`, and a concise `immediate_error`; endpoint, target, and CDP failures still use the normal non-zero error path.

Supported form:

- `lantern hover --selector <CSS_SELECTOR> --timeout-ms <MS>`

CDP inputs are identical to `lantern click` through point computation, then Lantern dispatches one `Input.dispatchMouseEvent` with `type=mouseMoved`, `button=none`, and the computed center coordinates.

Target selection, WebSocket requirements, timeout bounds, output redaction, and target-related error cases are identical to `lantern click`.

### `lantern wheel --selector <CSS_SELECTOR> [--dx <PX>] [--dy <PX>] --timeout-ms <MS>`

Purpose: dispatch one mouse-wheel event at the center of the selected element.

`lantern wheel` is a bounded pointer interaction helper for zoomable, scrollable, map-like, canvas, and WebGL surfaces. It requires an explicit selector, explicit timeout, and at least one non-zero finite delta. `--delta-x` and `--delta-y` are accepted as aliases for `--dx` and `--dy`.

Supported forms:

- `lantern wheel --selector <CSS_SELECTOR> --dy <PX> --timeout-ms <MS>`
- `lantern wheel --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --timeout-ms <MS>`

CDP inputs are identical to `lantern click` through point computation, then Lantern dispatches one `Input.dispatchMouseEvent` with `type=mouseWheel`, the computed center coordinates, and `deltaX`/`deltaY`.

Command-specific error cases:

- missing `--selector` or `--timeout-ms`: exit code `2`, error code `usage`
- missing, zero-only, non-finite, or invalid deltas: exit code `2`, error code `interaction_delta_invalid`
- invalid timeout bounds: exit code `2`, error code `interaction_timeout_invalid`
- target, WebSocket, and CDP errors are identical to `lantern click`.

### `lantern drag --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>`

Purpose: dispatch one left-button pointer drag starting at the center of the selected element and ending at the requested offset.

`lantern drag` is a bounded pointer interaction helper for panning canvases, maps, node graphs, timelines, and similar viewport surfaces. `lantern pointer-drag` is an alias. The command requires an explicit selector, explicit timeout, finite deltas, and an explicit duration from `0` through `30000` milliseconds. It does not infer a target surface, switch mouse buttons, use touch events, perform pinch gestures, or run multi-step interaction sessions.

Supported forms:

- `lantern drag --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>`
- `lantern pointer-drag --selector <CSS_SELECTOR> --dx <PX> --dy <PX> --duration-ms <MS> --timeout-ms <MS>`

CDP inputs are identical to `lantern click` through point computation, then Lantern dispatches:

- `mouseMoved` at the computed start point with no button pressed
- `mousePressed` at the start point with the left button
- one or more interpolated `mouseMoved` events with the left button pressed
- `mouseReleased` at the computed end point

Command-specific error cases:

- missing `--selector`, `--duration-ms`, or `--timeout-ms`: exit code `2`, error code `usage`
- non-finite or invalid deltas: exit code `2`, error code `interaction_delta_invalid`
- invalid duration bounds: exit code `2`, error code `interaction_duration_invalid`
- invalid timeout bounds: exit code `2`, error code `interaction_timeout_invalid`
- target, WebSocket, and CDP errors are identical to `lantern click`.

Pointer interaction JSON extends the shared `interaction.observed` object with nullable pointer fields:

- `pointer_start`: `{ "x": <number>, "y": <number> }` when a point is available
- `pointer_end`: drag endpoint, or `null` for click, hover, wheel, type, and key
- `delta_x` and `delta_y`: wheel or drag deltas, or `null`
- `duration_ms`: drag duration, or `null`
- `input_event_count`: number of CDP input events dispatched, or `0` for a timed-out pointer interaction

These fields are metadata only. Pointer interaction output does not capture DOM text, input values, cookies, storage, headers, request bodies, response bodies, or screenshots.

### `lantern flow`

Purpose: run one bounded browser-observation session over a single CDP attachment, so an agent can observe `open -> wait -> inspect` without losing console or network events between separate CLI invocations.

`lantern flow` is not a daemon and does not provide arbitrary test-runner automation. It attaches to the selected page target, enables Runtime, Log, Network, and Page domains, optionally navigates with `--open <URL>`, waits either for `document.readyState == complete` or for a quiet event window, collects console/runtime failures and network failures during that same attachment, reads final page title, URL shape, and ready state, then exits. It does not click, type, press keys, dispatch pointer gestures, evaluate operator-supplied JavaScript, persist artifacts, emit HAR output, read request or response bodies, capture screenshots, or keep a reusable browser session open after the command exits.

Required and optional flags:

- `--timeout-ms <MS>` is required and uses the same 1 through 30000 bound as `lantern wait`.
- `--quiet-ms <MS>` is optional. When present, `flow` waits for a quiet window across Runtime, Log, Network, and Page lifecycle events.
- `--open <URL>` is optional. When present, collection starts before this observed navigation.
- `--target-id <CDP_TARGET_ID>` is optional and uses the same selected-page target behavior as `lantern page`.

CDP inputs:

- `GET /json/list`
- `Runtime.enable`, `Log.enable`, `Network.enable`, and `Page.enable`
- optional `Page.navigate`
- event reads for Runtime, Log, Network, and Page events during the wait window
- `Runtime.evaluate` for final `document.title` and `document.readyState`
- `Page.getNavigationHistory` for final URL shape when available

When `--open` is supplied, `console.collection_gap` and `network.collection_gap` are `false` for the observed flow and use `collection_gap_reason=collection_started_before_observed_flow`. This means collection started before the navigation performed by `flow`; it does not make claims about page history before the command attached. When `--open` is omitted, `flow` attaches to an already-running page and reports the same collection gaps as `lantern console` and `lantern network`.

Human output should include:

- selected target short id
- title and URL shape
- whether navigation was performed
- whether one CDP attachment covered the flow
- wait condition, match status, timeout status, elapsed time, and timeout
- console and network failure counts
- console and network collection-gap flags
- bounded console and network entry lines using the same format as `lantern console` and `lantern network`

Recommended human output shape:

```text
flow: ABCD1234 title="Example" url=https://example.test/path opened=true single_attachment=true before_navigation=true wait_condition=quiet matched=true timed_out=false elapsed_ms=502 timeout_ms=5000 console_messages=0 console_exceptions=0 network_failed=0 network_http_errors=0 console_gap=false network_gap=false
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "flow",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path",
    "ready_state": "complete"
  },
  "flow": {
    "single_attachment": true,
    "opened_url": true,
    "observation_started_before_navigation": true,
    "wait": {
      "condition": "quiet",
      "matched": true,
      "timed_out": false,
      "elapsed_ms": 502,
      "timeout_ms": 5000,
      "observed": {
        "quiet_ms": 500,
        "event_count": 3,
        "last_event": "Network.loadingFinished"
      }
    }
  },
  "console": {
    "message_count": 0,
    "exception_count": 0,
    "max_entries": 20,
    "message_text_limit": 500,
    "truncated": false,
    "observed_clean": true,
    "collection_gap": false,
    "collection_gap_reason": "collection_started_before_observed_flow",
    "entries": []
  },
  "network": {
    "failed_count": 0,
    "http_error_count": 0,
    "max_entries": 20,
    "truncated": false,
    "observed_clean": true,
    "collection_gap": false,
    "collection_gap_reason": "collection_started_before_observed_flow",
    "entries": []
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `page.ready_state`
- `flow.single_attachment`
- `flow.opened_url`
- `flow.observation_started_before_navigation`
- `flow.wait`
- all required `console` fields from `lantern console`
- all required `network` fields from `lantern network`

`page.title`, `page.url_shape`, and `page.ready_state` may be `null` when unavailable. `flow.wait` reuses the `lantern wait` JSON shape. `console.entries` and `network.entries` must always be arrays.

Error cases:

- missing `--timeout-ms`: exit code `2`, error code `usage`
- invalid timeout or quiet window: exit code `2`, error code `wait_timeout_invalid`
- invalid `--open` URL: exit code `2`, error code `url_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- multiple page targets and no unique selected page: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- CDP navigation failure: exit code `1`, error code `navigation_failed`
- CDP command error or malformed CDP response: exit code `1`, error code `cdp_response_invalid`
- endpoint unreachable or invalid endpoint response: shared endpoint errors

Default output follows the flow, console, network, wait, and URL policies in `docs/product-specs/output-policy.md`.

### `lantern console`

Purpose: collect a bounded snapshot of recent console errors and page runtime exceptions from the selected page target.

`lantern console` is not a streaming log tail. It briefly enables the CDP Runtime and Log domains for the selected target, drains immediately available error events, then exits. It does not evaluate JavaScript, serialize arbitrary object graphs, read cookies or storage, persist logs, or keep a long-running WebSocket session open. Because CDP does not replay all console/runtime events that occurred before Lantern attached and enabled these domains, v1 always reports `collection_gap=true`.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- `Runtime.enable`
- `Log.enable`
- Bounded reads of `Runtime.consoleAPICalled`, `Runtime.exceptionThrown`, and `Log.entryAdded` events.

Target selection:

`lantern console` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern console` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Human output should include:

- selected target id short form
- selected page title, truncated by default
- URL shape, not the full URL by default
- console error message count
- runtime exception count
- explicit observed-clean status for the bounded attach window
- explicit collection-gap status
- explicit truncation status
- one bounded line per entry

Recommended human output shape:

```text
console: ABCD1234 title="Example" url=https://example.test/path messages=1 exceptions=1 observed_clean=false collection_gap=true truncated=false
1 Console https://example.test/app.js:42:7 args=2 frames=1 "Failed to load"
2 Exception https://example.test/app.js:9:3 args=0 frames=2 "Error: crashed"
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "console",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "console": {
    "message_count": 1,
    "exception_count": 1,
    "max_entries": 20,
    "message_text_limit": 500,
    "truncated": false,
    "observed_clean": false,
    "collection_gap": true,
    "collection_gap_reason": "events_before_runtime_log_enable_not_observed",
    "entries": [
      {
        "sequence": 1,
        "kind": "console",
        "severity": "error",
        "message": "Failed to load",
        "source_url_shape": "https://example.test/app.js",
        "line": 42,
        "column": 7,
        "argument_count": 2,
        "stack_frame_count": 1
      }
    ]
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `console.message_count`
- `console.exception_count`
- `console.max_entries`
- `console.message_text_limit`
- `console.truncated`
- `console.observed_clean`
- `console.collection_gap`
- `console.collection_gap_reason`
- `console.entries`
- `console.entries[].sequence`
- `console.entries[].kind`
- `console.entries[].severity`
- `console.entries[].message`
- `console.entries[].source_url_shape`
- `console.entries[].line`
- `console.entries[].column`
- `console.entries[].argument_count`
- `console.entries[].stack_frame_count`

`page.title`, `page.url_shape`, `console.entries[].source_url_shape`, `console.entries[].line`, and `console.entries[].column` may be `null` when unavailable. `console.entries` must be an array, including when empty.

Console feedback bounds:

- maximum entries: 20
- message text snippets: 500 Unicode scalar values by default
- event drain window: short and bounded; v1 does not provide a long-running streaming mode

`console.truncated` must be `true` when Lantern omitted matching entries because of the entry cap or truncated any console message text because of the text limit. `message_count` and `exception_count` count emitted entries, not the full browser history.

`console.observed_clean` is `true` only when the bounded collection window completes and emits no console errors or runtime exceptions. `console.collection_gap` is `true` in v1 because CDP does not reliably replay console/runtime events that occurred before `Runtime.enable` and `Log.enable`; consumers must not treat a clean result as proof that the whole page lifetime had no console failures.

Default output follows the console policy in `docs/product-specs/output-policy.md`. In particular, `lantern console` must not emit full serialized JavaScript objects, full stack traces, full source URLs, query strings, fragments, credentials, cookie or storage values, or raw CDP payloads.

Command-specific error cases:

- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP console event response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- `source_url_shape` uses the URL shape policy by default.
- message text is normalized, sensitive-looking assignment values are replaced with `:redacted`, URLs inside messages are converted to URL shapes, and text is truncated to 500 Unicode scalar values by default.
- `--no-redact` may expose full URLs and untruncated console message text for this invocation only.
- `--no-redact` does not cause Lantern to collect or print full object graphs, stack traces, cookies, local storage, or raw CDP payloads.

### `lantern network`

Purpose: collect a bounded snapshot of failed network requests and HTTP error responses from the selected page target.

`lantern network` is not a streaming network monitor and does not produce HAR output. It briefly enables the CDP Network domain for the selected target, records request metadata observed during that bounded window, reports `Network.loadingFailed` events and `Network.responseReceived` events with status `>= 400`, then exits. It does not read request bodies, response bodies, cookies, storage, raw headers, or full CDP payloads.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- `Network.enable`
- Bounded reads of `Network.requestWillBeSent`, `Network.responseReceived`, and `Network.loadingFailed` events.

Target selection:

`lantern network` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern network` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Human output should include:

- selected target id short form
- selected page title, truncated by default
- URL shape, not the full URL by default
- failed request count
- HTTP error response count
- observed-clean status
- collection-gap status
- explicit truncation status
- one bounded line per entry

Recommended human output shape:

```text
network: ABCD1234 title="Example" url=https://example.test/path failed=1 http_errors=1 observed_clean=false collection_gap=true truncated=false
1 failed_request GET https://example.test/api resource=fetch status=null reason=net::ERR_FAILED
2 http_error_response POST https://example.test/api/save resource=xhr status=500 reason=null
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "network",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "network": {
    "failed_count": 1,
    "http_error_count": 1,
    "max_entries": 20,
    "truncated": false,
    "observed_clean": false,
    "collection_gap": true,
    "collection_gap_reason": "events_before_network_enable_not_observed",
    "entries": [
      {
        "sequence": 1,
        "kind": "failed_request",
        "request_id": "1234.1",
        "method": "GET",
        "url_shape": "https://example.test/api",
        "resource_type": "fetch",
        "status": null,
        "failure_reason": "net::ERR_FAILED"
      }
    ]
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `network.failed_count`
- `network.http_error_count`
- `network.max_entries`
- `network.truncated`
- `network.observed_clean`
- `network.collection_gap`
- `network.collection_gap_reason`
- `network.entries`
- `network.entries[].sequence`
- `network.entries[].kind`
- `network.entries[].request_id`
- `network.entries[].method`
- `network.entries[].url_shape`
- `network.entries[].resource_type`
- `network.entries[].status`
- `network.entries[].failure_reason`

`page.title`, `page.url_shape`, `network.entries[].method`, `network.entries[].url_shape`, `network.entries[].resource_type`, `network.entries[].status`, and `network.entries[].failure_reason` may be `null` when unavailable. `network.entries` must be an array, including when empty.

Network feedback bounds:

- maximum entries: 20
- event drain window: short and bounded; v1 does not provide a long-running streaming mode
- URL metadata uses URL shape by default
- request and response bodies are never collected
- raw headers are not collected or printed

`network.truncated` must be `true` when Lantern omitted matching entries because of the entry cap. `failed_count` and `http_error_count` count emitted entries, not the full browser history.

`network.observed_clean` is `true` only when the bounded collection window completes and emits no failed requests or HTTP error responses. `network.collection_gap` is `true` in v1 because CDP does not replay requests that occurred before `Network.enable`; consumers must not treat a clean result as proof that the whole page lifetime had no network failures.

Default output follows the network policy in `docs/product-specs/output-policy.md`. In particular, `lantern network` must not emit full URLs, query strings, fragments, credentials, raw headers, cookies, authorization values, request bodies, response bodies, redirect chains, or raw CDP payloads.

Command-specific error cases:

- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP network event response: exit code `1`, error code `cdp_response_invalid`

Redaction behavior:

- `url_shape` uses the URL shape policy by default.
- failure reasons are normalized and bounded to the short-label limit.
- `--no-redact` may expose full request URLs in `url_shape` for this invocation only.
- `--no-redact` does not cause Lantern to collect headers, bodies, cookies, storage, or raw CDP payloads.

### `lantern screenshot`

Purpose: capture the selected page target's current visible viewport as a local PNG artifact for frontend feedback.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- `Page.getLayoutMetrics` for best-effort viewport dimensions.
- `Page.captureScreenshot` with PNG output, `fromSurface=true`, `captureBeyondViewport=false`, and an optional `clip` when a region is supplied.

Target selection:

`lantern screenshot` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern screenshot` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

Artifact path and overwrite behavior:

- `--output <PATH>` is required. Lantern never chooses a screenshot path implicitly and never writes image bytes to stdout.
- Relative output paths are resolved by the operating system relative to the current working directory. JSON and human output report the path as supplied by the operator.
- Parent directories must already exist. Lantern does not create artifact directories in v1.
- Existing files are not overwritten unless `--overwrite` is supplied.
- On success, Lantern writes exactly one PNG file to the requested path. It does not create screenshot history, sidecar metadata, visual diffs, thumbnails, galleries, or cleanup jobs.
- Cleanup is operator-owned: Lantern reports the output path and does not delete successful screenshots automatically.

Region/crop behavior:

- By default, `lantern screenshot` captures the full visible viewport.
- To crop the visible viewport, pass all of `--region-x`, `--region-y`, `--region-width`, and `--region-height`.
- `--crop-x`, `--crop-y`, `--crop-width`, and `--crop-height` are aliases for the matching region flags.
- Region `x` and `y` must be non-negative finite CSS-pixel coordinates; region `width` and `height` must be positive finite CSS-pixel dimensions.
- Region captures still write one PNG to the explicit `--output` path and still do not redact pixels.

Viewport assumptions:

- `lantern screenshot` captures the target's current visible viewport.
- The command does not resize the browser, alter device emulation, scroll, capture a full page, or stitch multiple images.
- `width` and `height` are best-effort viewport dimensions from `Page.getLayoutMetrics` when Chromium provides them; they may be `null`.

Human output should include:

- selected target id short form
- selected page title, truncated by default
- URL shape, not the full URL by default
- viewport dimensions when known
- requested region when supplied, otherwise `null`
- image byte count
- output path
- screenshot redaction caveat

Recommended human output shape:

```text
screenshot: ABCD1234 title="Example" url=https://example.test/path dimensions=1280x720 region=null bytes=123456 path=artifacts/page.png caveat=screenshot_contains_visible_page_pixels
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "screenshot",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "screenshot": {
    "format": "png",
    "width": 1280,
    "height": 720,
    "region": null,
    "byte_count": 123456,
    "path": "artifacts/page.png",
    "overwritten": false,
    "redaction_caveat": "screenshot_contains_visible_page_pixels"
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `screenshot.format`
- `screenshot.width`
- `screenshot.height`
- `screenshot.region`
- `screenshot.byte_count`
- `screenshot.path`
- `screenshot.overwritten`
- `screenshot.redaction_caveat`

`page.title`, `page.url_shape`, `screenshot.width`, `screenshot.height`, and `screenshot.region` may be `null` when unavailable or when no region is supplied.

Default output follows the screenshot policy in `docs/product-specs/output-policy.md`. In particular, `lantern screenshot` must not embed image bytes in JSON, print base64 image data to stdout, upload screenshots, keep screenshot history, or imply that visible page pixels are redacted. `--no-redact` may expose the selected page's full URL shape metadata only; it does not alter screenshot pixels or artifact persistence behavior.

Command-specific error cases:

- missing `--output`: exit code `2`, error code `usage`
- incomplete, non-finite, negative-origin, or non-positive-size region: exit code `2`, error code `screenshot_region_invalid`
- output path already exists without `--overwrite`: exit code `2`, error code `screenshot_output_exists`
- output path parent is missing or the file cannot be written: exit code `1`, error code `screenshot_write_failed`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error, malformed CDP screenshot response, or invalid screenshot base64: exit code `1`, error code `cdp_response_invalid`

### `lantern dom`

Purpose: summarize the selected page target's rendered DOM tree with concise, redacted, bounded node metadata useful to a coding agent.

Command shape:

```text
lantern dom [--depth <N>] [--max-nodes <N>]
```

`--depth` and `--max-nodes` are DOM-only flags. Defaults keep output compact for routine agent inspection; increase them when inspecting larger dashboards or nested application shells.

CDP inputs:

- `GET /json/list`
- The selected page target's WebSocket debugger endpoint.
- Read-only CDP DOM-domain commands needed to produce the summary, such as `DOM.getDocument` and narrowly scoped follow-up node reads.

Target selection:

`lantern dom` uses the same page target selection behavior as `lantern page`:

1. If `--target-id` is supplied and exactly matches the id of a `page` target, select it.
2. If `--target-id` is supplied but does not exactly match a `page` target, fail with exit code `1` and error code `target_not_found`.
3. If no explicit selector is supplied and exactly one `page` target exists, select it.
4. If no explicit selector is supplied, multiple `page` targets exist, and exactly one is attached or active according to CDP metadata, select it.
5. If no explicit selector is supplied and multiple candidates remain, fail with exit code `2` and error code `target_ambiguous`.
6. If no `page` target exists, fail with exit code `1` and error code `target_not_found`.

After target selection, `lantern dom` requires the selected target to expose a `webSocketDebuggerUrl`. If the selected target has no WebSocket debugger URL, the command fails with exit code `1` and the stable error code `target_websocket_missing`.

`--target-id` uses the same exact, case-sensitive CDP target id matching as `lantern page`. It does not accept short target id prefixes.

Human output should include:

- selected target id short form
- selected page title, truncated by default
- URL shape, not the full URL by default
- node count
- maximum emitted depth
- explicit truncation status
- a short indented tree of summarized nodes

Recommended human output shape:

```text
dom: ABCD1234 title="Example" url=https://example.test/path nodes=42 depth=3 truncated=true
html
  body
    main#app
      h1 "Dashboard"
      button[data-testid=save-button] "Save"
```

JSON output shape:

```json
{
  "schema_version": 1,
  "command": "dom",
  "ok": true,
  "page": {
    "target_id": "ABCD1234",
    "title": "Example",
    "url_shape": "https://example.test/path"
  },
  "dom": {
    "node_count": 42,
    "max_depth": 3,
    "truncated": true,
    "nodes": [
      {
        "node_id": "n1",
        "tag": "main",
        "role": null,
        "name": null,
        "text": null,
        "attributes": {
          "id": "app"
        },
        "child_count": 3,
        "children": []
      }
    ]
  }
}
```

Required JSON fields:

- `schema_version`
- `command`
- `ok`
- `page.target_id`
- `page.title`
- `page.url_shape`
- `dom.node_count`
- `dom.max_depth`
- `dom.truncated`
- `dom.nodes`
- `dom.nodes[].node_id`
- `dom.nodes[].tag`
- `dom.nodes[].role`
- `dom.nodes[].name`
- `dom.nodes[].text`
- `dom.nodes[].attributes`
- `dom.nodes[].child_count`
- `dom.nodes[].children`

`page.title`, `page.url_shape`, `dom.nodes[].role`, `dom.nodes[].name`, and `dom.nodes[].text` may be `null` when unavailable. `dom.nodes[].attributes` must be an object, including when empty. `dom.nodes[].children` must be an array, including when empty.

DOM summary bounds:

- default maximum depth: 4
- configurable maximum depth: `--depth <N>`, accepted range 1 through 12
- default maximum emitted nodes: 80
- configurable maximum emitted nodes: `--max-nodes <N>`, accepted range 1 through 500
- text snippets: 500 Unicode scalar values
- attribute values: 200 Unicode scalar values
- `class` attributes: at most 12 class tokens

`dom.node_count` is the number of nodes emitted in the response, not a count of the full page DOM when traversal was truncated. `dom.truncated` must be `true` when Lantern stopped adding nodes because the depth cap, node cap, child traversal cap, or another documented bound was reached.

Default output follows the DOM policy in `docs/product-specs/output-policy.md`. In particular, `lantern dom` must not emit full document HTML, full `textContent`, `<script>` contents, `<style>` contents, inline event-handler attribute values, hidden form values, cookies, local storage values, raw CDP payloads, full URLs, query strings, fragments, or password/token/secret/auth/session-like values.

Safe attributes by default:

- `id`
- `class`
- `role`
- `aria-label`
- `aria-labelledby`
- `name`, only when it does not look sensitive
- `type`
- `href` and `src` as URL shapes only
- `alt`
- `title`
- `data-testid`
- `data-test`
- `data-cy`

Command-specific error cases:

- invalid `--depth` or `--max-nodes`: exit code `2`, error code `dom_limit_invalid`
- no page target: exit code `1`, error code `target_not_found`
- explicit target id did not match a page target: exit code `1`, error code `target_not_found`
- ambiguous page target selection: exit code `2`, error code `target_ambiguous`
- selected page target has no `webSocketDebuggerUrl`: exit code `1`, error code `target_websocket_missing`
- invalid or non-local page WebSocket URL: exit code `1`, error code `cdp_unhealthy`
- WebSocket connection or command transport failure: exit code `1`, error code `endpoint_unreachable`
- CDP command error or malformed CDP DOM response: exit code `1`, error code `cdp_response_invalid`

## Redaction And Truncation

Default output must be safe enough for routine agent transcripts. Command implementations must follow `docs/product-specs/output-policy.md`.

Command-specific defaults:

- `endpoint.display` may include the local endpoint scheme, host, port, and configured path prefix, but must never include credentials, query strings, or fragments.
- `targets[].url_shape` and `page.url_shape` use the URL shape policy.
- `navigation.requested_url_shape` and `navigation.final_url_shape` use the URL shape policy for `http` and `https` URLs and preserve `about:blank`.
- `screenshot.path` is the operator-provided local artifact path; `screenshot.redaction_caveat` must state that screenshot pixels are not redacted.
- `targets[].title` and `page.title` use the page or target title limit of 120 Unicode scalar values.
- human output may abbreviate target ids, but JSON output must preserve exact CDP target ids.
- `console.entries[].source_url_shape` and URL-like substrings inside `console.entries[].message` use the URL shape policy by default.
- `console.entries[].message` is normalized, sensitive-looking assignment values are redacted, and text is truncated to 500 Unicode scalar values by default.
- `page.url_shape`, `dom.nodes[].text`, and string values under `dom.nodes[].attributes` use the URL, text, and DOM policies.
- `network.entries[].url_shape` uses the URL shape policy by default; failure reasons are normalized and bounded.
- `--no-redact` may expose full URLs and untruncated title, console message, DOM text, and safe attribute fields to stdout for the current invocation only.
- `--no-redact` must not print fields forbidden by the DOM policy and must not change persistence behavior; implemented commands do not persist browser artifacts.

## JSON Conventions

JSON output is designed for Smoogle and future UI adapters.

All successful JSON responses must:

- write one complete JSON object to stdout
- include `schema_version: 1`
- include `command`
- include `ok: true`
- use snake_case field names
- use `null` rather than omitting required nullable fields
- use arrays for collections, including empty collections
- keep numeric values as numbers and booleans as booleans
- avoid human prose except in explicit message fields

Field ordering should remain stable in serialized output for readability:

1. `schema_version`
2. `command`
3. `ok`
4. command-specific payload fields

Nested object field ordering, error JSON ordering, compatible-change rules, and future artifact output rules are defined in `docs/product-specs/output-policy.md`.

Compatible changes:

- adding optional nullable fields
- adding fields under new object keys
- adding enum variants when consumers are expected to tolerate unknown values

Breaking changes requiring a `schema_version` bump:

- removing required fields
- renaming fields
- changing field types
- changing redaction defaults for existing fields
- changing command selection behavior in a way that can alter successful results

## Error Output

Errors write to stderr for both human and JSON modes. Error output must include:

- a short stable error code
- a concise human message
- a remediation hint when practical

With `--json`, stderr should contain one JSON object and stdout should be empty:

```json
{
  "schema_version": 1,
  "ok": false,
  "error": {
    "code": "endpoint_missing",
    "message": "No CDP endpoint configured.",
    "hint": "Pass --endpoint http://127.0.0.1:9222 or set LANTERN_CDP_ENDPOINT."
  }
}
```

Without `--json`, stderr should be concise human text:

```text
error[endpoint_missing]: No CDP endpoint configured.
hint: Pass --endpoint http://127.0.0.1:9222 or set LANTERN_CDP_ENDPOINT.
```

## Exit Codes

- `0`: success, including help and version output
- `1`: runtime failure after valid invocation, such as unreachable endpoint, malformed CDP response, no page target, or transport failure
- `2`: usage or configuration error, such as unknown command, invalid flag, missing endpoint, invalid endpoint URL, or ambiguous page target selection
- `130`: interrupted by `SIGINT` when Lantern can observe and normalize the interruption

Lantern should not encode detailed failure categories into many exit codes. Use stable stderr error codes for detail.

## Stable Error Codes

Initial stable error codes:

- `usage`: unknown command, unsupported flag, invalid combination, or invalid value not covered below
- `endpoint_missing`: no endpoint source was provided
- `endpoint_invalid`: endpoint was syntactically invalid or outside the first-milestone local HTTP scope
- `endpoint_unreachable`: endpoint could not be reached
- `cdp_unhealthy`: endpoint responded but did not behave like a Chromium CDP endpoint
- `cdp_response_invalid`: endpoint returned malformed or unsupported CDP JSON
- `target_not_found`: no page target was available for `lantern page`, `lantern dom`, `lantern open`, `lantern wait`, `lantern console`, `lantern network`, `lantern flow`, `lantern screenshot`, `lantern click`, `lantern type`, `lantern key`, `lantern hover`, `lantern wheel`, or `lantern drag`, or `--target-id` did not match a page target
- `target_ambiguous`: multiple page targets matched and no deterministic selection was possible
- `target_websocket_missing`: the selected page target did not include a WebSocket debugger URL required by `lantern dom`, `lantern open`, `lantern wait`, `lantern console`, `lantern network`, `lantern flow`, `lantern screenshot`, `lantern click`, `lantern type`, `lantern key`, `lantern hover`, `lantern wheel`, or `lantern drag`
- `url_invalid`: the requested navigation URL was malformed or used an unsupported scheme
- `navigation_failed`: Chromium rejected or failed the requested page navigation
- `wait_timeout_invalid`: wait timeout or quiet-period bounds were outside the supported range
- `interaction_timeout_invalid`: interaction timeout bounds were outside the supported range
- `interaction_duration_invalid`: drag duration bounds were outside the supported range
- `interaction_delta_invalid`: wheel or drag deltas were missing or invalid
- `screenshot_region_invalid`: screenshot region/crop coordinates were incomplete or invalid
- `screenshot_output_exists`: screenshot output path already exists and `--overwrite` was not supplied
- `screenshot_write_failed`: screenshot output path could not be written
- `interrupted`: command was interrupted

## Environment

Supported environment variables:

- `LANTERN_CDP_ENDPOINT`: fallback CDP endpoint

No other environment variables are part of the first-milestone public contract.

## Examples

```bash
lantern doctor --endpoint http://127.0.0.1:9222
lantern targets --endpoint http://127.0.0.1:9222 --json
LANTERN_CDP_ENDPOINT=http://127.0.0.1:9222 lantern page
lantern page --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890
lantern open --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 http://localhost:5173/
lantern network --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --json
lantern flow --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --open http://localhost:5173/ --timeout-ms 5000 --quiet-ms 500 --json
lantern click --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --selector '[data-testid=save]' --timeout-ms 1000 --json
lantern type --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --selector 'input[name=q]' --text 'hello' --timeout-ms 1000 --json
lantern key --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --selector body --key ArrowUp --timeout-ms 1000 --json
lantern dom --endpoint http://127.0.0.1:9222 --target-id PAGE_ATTACHED_1234567890 --json
```
