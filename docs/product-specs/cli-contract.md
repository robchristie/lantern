# CLI Contract

This contract defines the first implemented Lantern command surface. It is the source of truth for `lantern-cli` until a later ExecPlan intentionally broadens the product scope.

The shared human-output, JSON ordering, redaction, truncation, DOM, console, network, screenshot, persistence, and test policy lives in `docs/product-specs/output-policy.md`. This contract defines command-specific fields and first-milestone behavior.

## Scope

The first milestone is read-only browser inspection against an operator-provided Chromium DevTools Protocol endpoint. Lantern does not launch Chromium, manage browser lifetime, navigate pages, mutate page state, evaluate JavaScript, capture screenshots, inspect DOM trees, inspect console logs, inspect network requests, or persist browser artifacts in this milestone.

## Command Surface

Implemented commands:

- `lantern doctor`
- `lantern targets`
- `lantern page`

Reserved for future milestones and not implemented yet:

- browser launch or attach lifecycle commands
- navigation and page mutation commands
- JavaScript evaluation
- screenshot capture
- DOM, console, and network inspection commands
- daemon, MCP, TUI, or web UI commands

Each implemented command accepts the shared flags below. Unknown commands, unknown flags, invalid flag values, and unsupported combinations must fail with exit code `2`.

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

Disables URL shape redaction and text truncation for fields that are otherwise redacted by default. This flag is intended for local debugging only. It must never change which browser artifacts Lantern persists, because first-milestone commands do not persist browser artifacts.

When `--no-redact` is absent, commands must avoid full URLs and other high-risk browser data by default.

### `-h`, `--help`

Prints command or subcommand help and exits with code `0`.

### `-V`, `--version`

Prints the Lantern CLI version and exits with code `0`.

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

1. If exactly one `page` target exists, select it.
2. If multiple `page` targets exist and exactly one is attached or active according to CDP metadata, select it.
3. If multiple candidates remain, fail with exit code `2` and tell the operator to close extra pages or use the future explicit target selector.
4. If no `page` target exists, fail with exit code `1`.

The first milestone intentionally does not define a `--target` flag. That flag should be added only when the target-selection behavior above proves insufficient.

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

## Redaction And Truncation

Default output must be safe enough for routine agent transcripts. Command implementations must follow `docs/product-specs/output-policy.md`.

First-milestone command-specific defaults:

- `endpoint.display` may include the local endpoint scheme, host, port, and configured path prefix, but must never include credentials, query strings, or fragments.
- `targets[].url_shape` and `page.url_shape` use the URL shape policy.
- `targets[].title` and `page.title` use the page or target title limit of 120 Unicode scalar values.
- human output may abbreviate target ids, but JSON output must preserve exact CDP target ids.
- `--no-redact` may expose full URLs and untruncated title fields to stdout for the current invocation only.
- `--no-redact` must not change persistence behavior; first-milestone commands do not persist browser artifacts.

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
- `target_not_found`: no page target was available for `lantern page`
- `target_ambiguous`: multiple page targets matched and no deterministic selection was possible
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
```
