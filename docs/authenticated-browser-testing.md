# Authenticated Browser Testing

Lantern can connect to a Chromium profile that is already logged into a service, but authenticated browser state is sensitive. Treat cookies, local storage, session storage, page text, conversation contents, account metadata, URLs, console output, network metadata, screenshots, typed text, selectors, and interaction traces as local secrets.

Use this workflow for smoke testing against a dedicated browser profile, such as a Chromium profile logged into ChatGPT through an operator-owned VNC session.

## Safety Checklist

- Use a throwaway or dedicated Chromium profile created only for Lantern testing.
- Do not attach Lantern to a daily browser profile, personal profile, or profile with saved credentials outside the test account.
- Keep CDP loopback-only: use `--remote-debugging-address=127.0.0.1` on the same host, or publish containers with `-p 127.0.0.1:9222:9222`.
- Verify CDP with `lantern doctor --endpoint http://127.0.0.1:9222` before running page commands.
- Start with read-only commands: `doctor`, `targets`, `page`, `dom`, `console`, and `network`.
- Avoid `--no-redact` during authenticated testing unless the operator explicitly needs a local debugging dump and has reviewed where stdout/stderr will be recorded.
- Do not run `lantern open`, `lantern click`, or `lantern type` until navigation or submission is explicitly intended.
- Do not submit text with `lantern type` plus `lantern click` unless the operator has reviewed the target page and selector.
- Review screenshot output paths before capture. Screenshots are visible page pixels and are not redacted.
- Keep screenshots, terminal transcripts, Smoogle run logs, and copied command output out of Git and external chat unless manually reviewed.

## Dedicated Profile Setup

### Lantern-managed persistent profile

For repeated managed-browser journeys, prefer Lantern's explicit named profile
instead of a temporary profile under a source checkout:

```sh
lantern browser profile create geometis-review
lantern browser start --profile geometis-review
```

For a local application whose public hostname must resolve back to the host
from a rootless browser container, add the narrow mapping explicitly on every
start:

```sh
lantern browser start \
  --profile geometis-review \
  --host-gateway app.example.test
```

This maps only the validated hostname to the runtime's host-gateway target. It
does not broaden CDP exposure or allow a caller-selected gateway address.

Open the returned noVNC URL and log into the dedicated test account manually.
This is the only expected interactive login until the service expires or
revokes the session. Lantern stores the Chromium profile privately outside the
source checkout; it does not store the password, extract cookies, or determine
login state.

For later checks:

```sh
ID="$(lantern browser start \
  --profile geometis-review \
  --host-gateway app.example.test \
  --json | jq -r .instance.id)"
ENDPOINT="$(lantern browser endpoint "$ID" --json | jq -r .instance.endpoint)"
lantern doctor --endpoint "$ENDPOINT"
```

Stop and prune instance metadata after the check. Both commands preserve the
profile. Persistent stop asks Chromium to close cleanly before releasing the
profile so session databases are flushed to the owner-private data directory:

```sh
lantern browser stop "$ID"
lantern browser prune
lantern browser profile status geometis-review
```

### Operator-managed profile

Create a profile directory that is not your normal browser profile:

```sh
mkdir -p /tmp/lantern-auth-profile
```

For a visible browser controlled through VNC or noVNC, start Chromium in that display with CDP bound to loopback:

```sh
export DISPLAY=:1

chromium \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/lantern-auth-profile \
  --no-first-run \
  --no-default-browser-check \
  about:blank
```

Log into the test account manually in the visible browser. Prefer a throwaway account or a dedicated test account with minimal data.

Then verify Lantern from the same host:

```sh
lantern doctor --endpoint http://127.0.0.1:9222
lantern targets --endpoint http://127.0.0.1:9222
lantern page --endpoint http://127.0.0.1:9222
```

When Chromium runs in a container and Lantern runs on the host, publish CDP to host loopback only:

```sh
docker run --rm -it \
  -p 127.0.0.1:9222:9222 \
  --shm-size=1g \
  chromium-vnc-image
```

Inside that container or supervisor, Chromium may need `--remote-debugging-address=0.0.0.0` so the container port can be published. The host publish rule must still be loopback-only.

## Read-Only Smoke Commands

Use these commands first. They inspect or summarize browser state without intentionally navigating, clicking, typing, or writing page-visible data:

```sh
lantern doctor --endpoint http://127.0.0.1:9222
lantern targets --endpoint http://127.0.0.1:9222
lantern page --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID>
lantern dom --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID>
lantern console --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID>
lantern network --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID>
```

Use `--target-id` once multiple tabs exist. It prevents accidental inspection or mutation of the wrong authenticated page.

`lantern dom`, `lantern console`, and `lantern network` are bounded summaries, not full exports. They can still reveal sensitive page text, account labels, redacted URL shapes, error text, request paths, or resource names, so treat output as sensitive.

## Interaction Commands

Use interaction commands only after the target and selector have been reviewed:

```sh
lantern wait --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID> selector --selector '#composer'
lantern click --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID> --selector '#composer'
lantern type --endpoint http://127.0.0.1:9222 --target-id <PAGE_TARGET_ID> --selector '#composer' --text 'draft text'
```

`lantern type` does not echo the typed text in its output, but the text is still sent to the selected browser page. A later click, Enter key behavior in the page, or site autosave may submit or persist it. Lantern does not provide ChatGPT-specific safeguards or automatic submit prevention.

For a test-only password or another local secret, keep the value out of process
arguments and use an owner-private file:

```sh
lantern type \
  --endpoint http://127.0.0.1:9222 \
  --target-id <PAGE_TARGET_ID> \
  --selector '#password' \
  --text-file /operator/private/test-password \
  --timeout-ms 1000
```

The file must be a regular UTF-8 file no larger than 64 KiB. On Unix, its mode
must grant no group or other access and the final path must not be a symlink.
Lantern preserves the input exactly and reports only the inserted character
count; it never prints the path or contents, including with `--no-redact`.
Lantern does not create, rotate or delete this operator-owned secret.

## Screenshots

Screenshots require an explicit path:

```sh
mkdir -p .smoogle/artifacts
lantern screenshot \
  --endpoint http://127.0.0.1:9222 \
  --target-id <PAGE_TARGET_ID> \
  --output .smoogle/artifacts/chatgpt-smoke.png
```

Screenshots are not redacted. They may include conversation contents, account metadata, prompt text, workspace names, avatars, and browser UI. Store them under untracked local state such as `.smoogle/artifacts/` unless the operator intentionally chooses another local path.

## Redaction Expectations

Default output is safer for transcripts than `--no-redact`, but it is not a guarantee that authenticated content is harmless.

- Endpoint metadata: Lantern accepts only local HTTP endpoints and does not print credentials, query strings, or fragments from endpoint configuration.
- Page and target metadata: titles are normalized and truncated; URL output uses URL shapes that omit credentials, query strings, fragments, and sensitive-looking path segments.
- DOM output: Lantern emits bounded node summaries, safe attributes, and text snippets. It omits script/style contents, inline event handlers, hidden form values, and sensitive attribute names. Default DOM text snippets redact URL-shaped values, sensitive-looking assignments, and bearer/JWT-like tokens, but normal visible page text may still be sensitive.
- Console output: Lantern emits bounded error and exception summaries only. Default message text redacts URL-shaped values, sensitive-looking assignments, and bearer/JWT-like tokens. It does not serialize full object graphs, stack traces, cookies, local storage, or raw CDP payloads.
- Network output: Lantern reports failed requests and HTTP error responses as bounded metadata. It does not collect request bodies, response bodies, headers, cookies, authorization values, redirect chains, or HAR data. URL output uses URL shapes by default.
- Screenshot output: Lantern writes PNG bytes only to the explicit `--output` path. Screenshot pixels are not redacted.
- Interaction output: `click` and `type` report metadata about dispatch and selected page state. They do not print DOM text, input values, headers, bodies, cookies, storage, screenshots, or a `--text-file` path/source. `type` still sends the supplied text to the page.
- `--no-redact`: this may expose full URLs and untruncated text fields for the current invocation. It does not make Lantern collect cookies, storage, headers, bodies, full DOM HTML, screenshot bytes in JSON, or raw CDP payloads.

## Cleanup

For an operator-managed temporary profile, after the smoke test:

```sh
rm -rf /tmp/lantern-auth-profile
```

Only delete the dedicated profile after confirming it is not a daily browser profile. If the profile contains a real account session, log out manually before deleting it when the service requires server-side session revocation.

For a Lantern-managed persistent profile, ordinary cleanup is only stop plus
prune. To retire it permanently, log out manually when required, stop its
browser, then use:

```sh
lantern browser profile delete geometis-review --yes
```
