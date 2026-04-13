# ExecPlan: DOM Summary Command

Status: active

## Progress Notes

- 2026-04-13: created as the first follow-on command slice after the initial `doctor`, `targets`, and `page` inspection surface.
- 2026-04-13: completed Slice 1 by updating `docs/product-specs/cli-contract.md` with the `lantern dom` command contract, including bounded human/JSON output, page target selection, and command-specific error cases.
- 2026-04-13: completed Slice 2 by adding typed DOM summary serialization types in `lantern-core` and moving browser-derived text, URL, and DOM attribute sanitization into a reusable core redaction policy.

## Goal

Add one read-only DOM inspection command that gives agents a concise, redacted page structure summary from the selected Chromium page target.

The intended CLI surface is:

- `lantern dom`

This plan should only broaden Lantern from target/page metadata into bounded DOM summaries. It should not pull in navigation, page mutation, JavaScript evaluation, screenshots, console logs, network traffic, browser launch, daemon mode, or persistence.

## Why This Matters

The first milestone proves that Lantern can connect to an operator-provided CDP endpoint and summarize browser/page metadata. The next useful frontend-development loop is to answer "what is on the page?" without forcing an agent to open broad DevTools context or dump full HTML into a transcript.

A small DOM summary command gives agents enough structure to reason about rendered UI while preserving the local, read-only, redacted v1 posture.

## Constraints

- keep Chromium lifecycle operator-owned
- keep endpoint resolution unchanged: `--endpoint` then `LANTERN_CDP_ENDPOINT`
- keep target selection aligned with `lantern page` until an explicit target selector is designed
- use the existing local `ws://` CDP command boundary
- default to redacted, bounded output that follows `docs/product-specs/output-policy.md`
- avoid durable storage of DOM data or browser artifacts
- keep command output stable enough for fixture tests

## Non-Goals

- launching or managing Chromium
- navigation, clicking, typing, form submission, or other page mutation
- JavaScript evaluation as a user-facing command
- full HTML dumps, full `textContent`, script/style contents, hidden form values, cookies, local storage, or credential-like attributes
- screenshots or binary artifacts
- console or network inspection
- persistent sessions, SQLite storage, daemon mode, MCP, TUI, or web UI
- cross-browser support beyond Chromium CDP

## Command Contract Direction

`lantern dom` should select the same single page target as `lantern page`, connect to its WebSocket debugger endpoint, and return a compact DOM tree summary.

Human output should be short and scan-friendly, for example:

```text
dom: target=ABCD1234 nodes=42 depth=3 truncated=true
html
  body
    main#app
      h1 "Dashboard"
      button[data-testid=save-button] "Save"
```

JSON output should use one complete object with stable field ordering:

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

The exact JSON shape may be refined during implementation, but it must preserve:

- `schema_version`
- `command`
- `ok`
- selected page identity
- bounded node summaries
- explicit truncation metadata
- nullable fields instead of placeholder prose

## DOM Collection Direction

Prefer CDP DOM-domain reads over a user-facing JavaScript evaluation command. The implementation may use the minimum CDP calls needed to produce the summary, such as:

- `DOM.getDocument`
- `DOM.describeNode`
- `DOM.getOuterHTML` only for narrowly scoped fallback parsing if a safer structured API is insufficient
- `Accessibility.getFullAXTree` only if accessible role/name support can stay bounded and deterministic

If the first implementation cannot reliably derive role or accessible name without too much complexity, leave those fields nullable and track richer accessibility data as a follow-up.

## Redaction And Bounds

Default output must follow the DOM policy in `docs/product-specs/output-policy.md`.

Initial bounds:

- maximum depth: 4
- maximum nodes: 80
- text snippets: 500 Unicode scalar values
- attribute values: 200 Unicode scalar values
- class tokens: at most 12 tokens
- child collections: stop adding children once the node or depth cap is reached

Default safe attributes:

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

Sensitive attribute names or values must be omitted or replaced with a clear redacted marker. The command must not emit script contents, style contents, hidden form values, password/token/secret/auth/session-like values, full URLs, query strings, fragments, cookies, local storage, or raw CDP payloads by default.

`--no-redact` may relax truncation and URL-shape redaction only where the output policy allows it. It must not cause Lantern to collect or print values that the DOM policy forbids by design.

## Implementation Slices

### Slice 1: Contract Update

Status: completed

- update `docs/product-specs/cli-contract.md` to add `lantern dom` as the next implemented command
- keep other reserved future commands out of scope
- document command-specific JSON and human output
- document target-selection behavior and error cases

### Slice 2: Domain Summary Types

Status: completed

- add DOM summary structs in the appropriate Rust boundary
- keep browser-derived redaction and truncation logic reusable rather than duplicating ad hoc output code
- preserve stable JSON field ordering through typed serialization

### Slice 3: CDP DOM Read Path

Status: pending

- select the page target using the existing page-selection rules
- require a page WebSocket debugger URL and return a clear error when unavailable
- issue the minimal read-only CDP calls needed for a bounded DOM summary
- map CDP transport, command, and malformed-response failures into existing CLI error conventions

### Slice 4: CLI Output

Status: pending

- add `lantern dom` parsing and help text
- emit concise human output
- emit stable JSON output
- apply default redaction and bounded traversal
- ensure `--json`, `--no-redact`, and endpoint handling behave consistently with existing commands

### Slice 5: Fixtures And Validation

Status: pending

- add deterministic fixture tests for successful human and JSON output
- cover unavailable page target, missing WebSocket URL, ambiguous page target, truncation, safe attributes, sensitive attributes, URL-shape handling, and CDP command errors
- run `scripts/validate.sh fast` during implementation
- run `scripts/validate.sh` before landing Rust changes
- run `smoogle docs check` after contract or ExecPlan edits

## Acceptance Criteria

- `lantern dom` works against an operator-provided local Chromium CDP endpoint
- the command is read-only and does not mutate page state
- human output is concise enough for an agent transcript
- JSON output has stable field ordering and bounded collections
- default output avoids full URLs, full HTML, raw CDP payloads, script/style contents, sensitive attributes, hidden form values, cookies, local storage, and credential-like values
- `--no-redact` follows the output policy without bypassing forbidden collection rules
- fixture tests cover the command path and key redaction behavior
- docs clearly keep console, network, screenshots, navigation, and interactions reserved for later plans

## Open Questions

- Should the first DOM command support a selector flag, or should it always summarize the document root until a later target/selector design exists?
- Should accessible role and name be included in the first implementation, or deferred until an accessibility-specific CDP slice?
- Should node ids be response-local stable identifiers, CDP node ids, or omitted from human output until follow-up commands need addressable nodes?
