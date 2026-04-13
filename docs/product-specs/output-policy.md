# Output Policy

This policy defines how Lantern writes human and JSON output, and how it redacts or truncates browser-derived data. It applies to first-milestone commands and to reserved future commands for DOM, console, network, and screenshot inspection.

Lantern implements `lantern doctor`, `lantern targets`, `lantern page`, `lantern dom`, and `lantern open`. Future commands must reuse this policy unless a later design explicitly changes it with a `schema_version` bump where needed.

## Goals

- Keep human output short enough for agent transcripts.
- Keep JSON output stable enough for scripts, Smoogle, and future UI adapters.
- Avoid leaking sensitive browser data by default.
- Preserve enough shape for debugging frontend work.
- Make truncation and redaction deterministic so tests can assert exact output.

## Human Output

Human output is for quick operator and agent reading, not durable machine parsing.

Rules:

- Write successful human output to stdout.
- Write errors to stderr.
- Prefer one summary line plus a short bounded list.
- Avoid progress text for commands that perform one bounded inspection.
- Include only fields that help decide the next frontend-development step.
- Use labels that match the JSON field names where practical.
- Do not print raw CDP responses.
- Do not print full URLs, full DOM, full text bodies, full console payloads, raw request or response headers, cookies, local storage values, or screenshot bytes by default.

Recommended first-milestone shape:

```text
ok: Chrome/123.0.0.0 protocol=1.3 endpoint=http://127.0.0.1:9222 websocket=available
```

```text
ABCD1234 page attached title="Example" url=https://example.test/path
```

```text
page: ABCD1234 title="Example" url=https://example.test/path loading=complete
```

Human output may abbreviate identifiers when the full identifier is not needed for a follow-up command. JSON must keep exact identifiers.

## JSON Output

JSON output is the compatibility contract. Implementations should serialize typed response structs rather than ad hoc maps so field ordering is stable.

All successful JSON responses must:

- write exactly one complete JSON object to stdout
- write no human progress text to stdout
- include `schema_version`
- include `command`
- include `ok`
- use snake_case field names
- use `null` instead of omitting required nullable fields
- use arrays for collections, including empty collections
- preserve source ordering only when the command contract says source ordering is the tiebreaker
- keep numbers as numbers and booleans as booleans
- avoid human prose except in explicit `message`, `summary`, or `hint` fields

Top-level field order:

1. `schema_version`
2. `command`
3. `ok`
4. command-specific payload fields
5. optional `metadata`

Nested object field order:

1. stable identity fields, such as `id`, `target_id`, `request_id`, or `node_id`
2. type or category fields
3. primary display fields, such as `title`, `url_shape`, `text`, or `message`
4. status fields, such as `attached`, `loading_state`, `severity`, `method`, `status`, or `mime_type`
5. count, size, and timing fields
6. optional child collections

Error JSON goes to stderr and uses this top-level order:

1. `schema_version`
2. `ok`
3. `error`

Within `error`, use this order:

1. `code`
2. `message`
3. `hint`
4. optional `details`

Compatible JSON changes:

- adding optional nullable fields
- adding new nested objects
- adding new enum variants when consumers are expected to tolerate unknown values
- adding `metadata` fields that do not affect command success semantics

Breaking JSON changes that require a `schema_version` bump:

- removing required fields
- renaming fields
- changing field types
- changing default redaction or truncation for an existing field
- changing command selection behavior in a way that changes successful results

## Redaction Modes

Default mode is safe for routine agent transcripts. It returns shapes, summaries, counts, and bounded snippets.

`--no-redact` is a local debugging escape hatch. It may expose full URLs and untruncated fields to stdout for the current invocation only. It must not cause Lantern to persist browser artifacts, and it must not expose values that Lantern intentionally never collects, such as cookies or local storage.

Future commands that can capture high-risk artifacts should support a second explicit confirmation flag before writing files, even when `--no-redact` is present. For example, screenshot capture should require an output path or another explicit artifact-writing option rather than writing by default.

## URL Policy

Default URL fields must use `url_shape`, not `url`, unless a command explicitly documents otherwise.

URL shape format:

- include scheme
- include host
- include port only when non-default
- include path segments when they do not look sensitive
- omit query strings
- omit fragments
- omit username and password
- replace sensitive-looking path segments with `:redacted`
- preserve a trailing slash only when it is the whole path

Sensitive-looking path segments include:

- segments longer than 64 Unicode scalar values
- UUIDs
- long hex-like tokens, with 24 or more hex characters
- base64-like or base64url-like tokens, with 32 or more token characters
- email addresses
- bearer-token-like values beginning with `eyJ`
- segments following sensitive labels: `token`, `key`, `secret`, `session`, `auth`, `password`, `passwd`, `invite`, `reset`, `code`, `otp`, `jwt`, `access_token`, `refresh_token`

Examples:

```text
https://example.test/users/123/settings?tab=billing#token
=> https://example.test/users/123/settings

https://example.test/reset/4a7f9c0e2d1b4c6a8e9f0123456789ab
=> https://example.test/reset/:redacted

https://user:pass@example.test/path
=> https://example.test/path
```

When the input URL cannot be parsed, output `null` for `url_shape` and include a command error only if the URL is required for the command to succeed.

## Text Policy

Truncation counts Unicode scalar values, not bytes. Truncation appends `...`, and the ellipsis marker counts outside the configured limit.

Default limits:

| Field class | Limit |
| --- | ---: |
| Page or target titles | 120 |
| Short labels and names | 120 |
| Console messages | 500 |
| DOM text snippets | 500 |
| Attribute values | 200 |
| Network header values | 120 |
| Network request or response body snippets | 500 |
| Error messages from Lantern itself | 240 |

Before truncation:

- normalize CRLF and CR to LF
- replace ASCII control characters except tab and LF with a single space
- collapse runs of more than two blank lines to two blank lines
- preserve meaningful internal whitespace within code-like snippets

Text fields that came from the browser should be nullable when unavailable. They should not use placeholder prose such as `unknown` unless `unknown` is an actual browser value.

## DOM Policy

DOM inspection is outside the first milestone, but future DOM commands must default to summaries and snippets.

Default DOM output may include:

- node identifiers generated by Lantern for the response
- tag names
- stable roles or accessible names when available
- bounded text snippets
- selected safe attributes
- child counts
- match counts

Default DOM output must not include:

- full document HTML
- full textContent for large nodes
- `<script>` contents
- `<style>` contents
- inline event-handler attribute values
- hidden form values
- password, token, secret, or authorization fields

Safe attributes by default:

- `id`
- `class`, truncated and token-count bounded
- `role`
- `aria-label`, truncated
- `aria-labelledby`, truncated
- `name`, only when it does not look sensitive
- `type`
- `href` and `src` as URL shapes only
- `alt`, truncated
- `title`, truncated
- `data-testid`, `data-test`, and `data-cy`

Sensitive attribute names include any name containing `token`, `secret`, `password`, `passwd`, `auth`, `session`, `cookie`, `key`, `jwt`, `credential`, `csrf`, `nonce`, or `signature`.

## Console Policy

Console inspection is outside the first milestone, but future console commands must summarize messages without dumping arbitrary object graphs.

Default console output may include:

- severity
- timestamp or monotonic order
- source URL shape
- line and column when available
- message text truncated to 500 Unicode scalar values
- argument count
- stack frame count

Default console output must not include:

- full serialized JavaScript objects
- full stack traces unless a command explicitly requests stack detail
- full source URLs
- cookie, authorization, storage, or credential values

When console arguments are objects, output a bounded type summary such as `object`, `array(3)`, `error`, or `node`, plus a truncated primitive preview only for strings, numbers, booleans, and null.

## Network Policy

Network inspection is outside the first milestone, but future network commands must default to request and response metadata rather than payload capture.

Default network output may include:

- request id
- method
- URL shape
- resource type
- status code
- MIME type
- timing summary
- transfer size
- initiator URL shape when available
- redacted request and response header names with bounded safe values

Default network output must not include:

- cookies
- authorization headers
- proxy authorization headers
- raw request bodies
- raw response bodies
- full query strings
- full redirect URLs
- security tokens embedded in headers or paths

Header handling:

- Always redact `cookie`, `set-cookie`, `authorization`, `proxy-authorization`, `x-api-key`, `x-auth-token`, `x-csrf-token`, and `x-xsrf-token` values.
- Preserve common low-risk values when truncated: `content-type`, `content-length`, `cache-control`, `etag`, `last-modified`, `location` as URL shape, and `server`.
- Treat unknown headers as name-only by default unless a future command explicitly opts into values.

Body snippets require an explicit command option in a future design. When enabled, body snippets must be text-only, MIME-filtered, truncated, and marked with byte counts.

## Screenshot Policy

Screenshot capture is outside the first milestone. Future screenshot commands must treat screenshots as sensitive artifacts.

Default screenshot behavior:

- do not capture screenshots unless the command explicitly asks for one
- do not write screenshot files without an explicit output path or artifact flag
- do not embed image bytes in JSON
- do not print base64 image data to stdout
- return metadata only by default, such as width, height, format, byte count, and output path when one was explicitly requested

Screenshot paths in JSON should be operator-provided paths or paths under Lantern-managed local state. They should not be uploaded or linked to external services by default.

## Persistence

Output redaction is separate from persistence. First-milestone commands do not persist browser artifacts.

Future persistence must not store credentials, cookies, local storage, DOM dumps, screenshots, full URLs, console payloads, network bodies, or sensitive browser artifacts unless a later design explicitly defines:

- storage location
- retention behavior
- redaction behavior
- operator opt-in
- cleanup workflow

## Test Expectations

Implementation tests should cover:

- stable top-level JSON field ordering
- stable nested JSON field ordering for each response type
- default URL shape redaction
- `--no-redact` behavior for fields it is allowed to expose
- Unicode scalar truncation
- control-character normalization
- null handling for unavailable fields
- concise human output that avoids raw CDP payloads
- error output split between stdout and stderr
