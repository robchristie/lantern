---
name: lantern-ui-inspection
description: Inspect and dogfood local web UIs with Lantern, a narrow CLI over Chromium CDP. Use when Codex is doing frontend, dashboard, web observability, or browser-facing UI work and should verify the result through a local browser without MCP overhead; especially for Smoogle web dashboard work, Leptos/Rust web UI changes, local app servers, flow/layout/DOM/screenshot checks, managed disposable browser sessions, or when repeated browser-inspection friction should become Lantern product work.
---

# Lantern UI Inspection

## Purpose

Use Lantern as a small, text-first browser inspection loop for agentic frontend development. Prefer it over broad browser MCP surfaces when the task needs stable command output, one-shot flow observation, compact DOM summaries, console/network checks, layout audits, screenshots, or repeated dashboard dogfooding.

Do not use this skill for non-UI backend tasks. Do not expand Lantern's command surface during a UI task unless the same inspection gap repeats and cannot be solved with the current commands.

## Preconditions

- Confirm the app server is running or start it yourself when safe.
- Use plain `lantern` first; in Smoogle child runs a run-local shim is injected when Lantern is discoverable. If unavailable, try the command path in `$LANTERN_BIN`, `/usr/local/bin/lantern`, `/nvme/development/lantern/target/release/lantern`, then `/nvme/development/lantern/target/debug/lantern`.
- Confirm a Chromium or Chrome instance exposes a local CDP HTTP endpoint, or explicitly start a disposable managed instance with `lantern browser start`.
- If the browser runs in a container, open the app through a container-reachable URL such as `http://host.docker.internal:<port>/`, not host loopback.
- Keep Smoogle dashboard non-loopback binding explicit and local/trusted.

## Browser Setup

Use an operator-owned endpoint when one already exists:

```bash
ENDPOINT=http://127.0.0.1:9222
lantern doctor --endpoint "$ENDPOINT" --json
```

For an isolated disposable browser, use Lantern's managed lifecycle. Build the browser image first if the repo has not already done so, then start an instance and pass its endpoint explicitly:

```bash
ID="$(lantern browser start --json | jq -r .instance.id)"
lantern browser list --json
ENDPOINT="$(lantern browser endpoint "$ID" --json | jq -r .instance.endpoint)"
lantern doctor --endpoint "$ENDPOINT" --json
```

Stop disposable instances when finished:

```bash
lantern browser stop "$ID" --json
lantern browser prune --json
```

## Smoogle Dashboard Loop

For Smoogle UI work, start the dashboard like this when a containerized browser needs to reach it:

```bash
/nvme/development/smoogle/target/debug/smoogle-cli web serve --host 0.0.0.0 --port 7879 --allow-non-loopback
```

Then inspect with Lantern. Prefer `flow` for the first navigation because it observes navigation, wait state, console, and network in one attachment:

```bash
ENDPOINT=http://127.0.0.1:9222
URL=http://host.docker.internal:7879/

lantern doctor --endpoint "$ENDPOINT" --json
lantern flow --endpoint "$ENDPOINT" --open "$URL" --timeout-ms 5000 --quiet-ms 500 --json
lantern page --endpoint "$ENDPOINT" --json
lantern layout --endpoint "$ENDPOINT" --json
lantern dom --endpoint "$ENDPOINT" --json
lantern dom --endpoint "$ENDPOINT" --depth 8 --max-nodes 220 --json
lantern screenshot --endpoint "$ENDPOINT" --output /tmp/smoogle-dashboard.png --overwrite --json
```

Use a different port/path for other local web apps, but keep the same inspection pattern.

Use `console` and `network` as ad hoc follow-ups after later UI changes or interactions:

```bash
lantern console --endpoint "$ENDPOINT" --json
lantern network --endpoint "$ENDPOINT" --json
```

## Inspection Heuristics

- Run `doctor` first if CDP connectivity is uncertain.
- Prefer `flow --open` for a fresh navigation when judging page-load errors. Treat `flow.console.collection_gap=false` and `flow.network.collection_gap=false` as evidence that collection started before the flow navigation, not as proof that no earlier browser history exists.
- Use separate `open`, `wait`, `console`, and `network` commands for ad hoc snapshots or after interactions; prefer `flow` when one coherent `open -> wait -> inspect` observation is needed.
- Run `page` before judging DOM output.
- Run `layout` when checking viewport sizing, horizontal overflow, clipped text, or other layout regressions.
- Start with default `dom` for compact structure; increase to `--depth 8 --max-nodes 220` when an app shell hides meaningful content below the default cap.
- Check `console` and `network` after navigation and after UI changes; do not rely only on screenshots.
- Use screenshots as supporting evidence for layout and visual regressions, not as the primary inspection artifact.
- Prefer JSON output for precise assertions and stable summaries; use human output only for quick local reading.
- If Lantern reports a browser/tool failure but the page clearly changed, inspect the Lantern output and CDP state before treating the app as broken.

## Interactions

Use interaction commands only for explicit, bounded UI checks. Keep them separate from `flow` until a later Lantern workflow defines multi-step interaction sessions:

```bash
lantern click --endpoint "$ENDPOINT" --selector '[data-testid=save]' --timeout-ms 1000 --json
lantern type --endpoint "$ENDPOINT" --selector 'input[name=q]' --text 'hello' --timeout-ms 1000 --json
lantern key --endpoint "$ENDPOINT" --selector body --key ArrowUp --timeout-ms 1000 --json
lantern hover --endpoint "$ENDPOINT" --selector '[data-testid=viewer-canvas]' --timeout-ms 1000 --json
lantern wheel --endpoint "$ENDPOINT" --selector '[data-testid=viewer-canvas]' --delta-y -400 --timeout-ms 1000 --json
lantern drag --endpoint "$ENDPOINT" --selector '[data-testid=viewer-canvas]' --dx 160 --dy -80 --duration-ms 250 --timeout-ms 1000 --json
```

After an interaction, re-check the focused surface with `page`, `dom`, `layout`, `console`, `network`, or a screenshot as appropriate.

## Feedback Rule

When UI inspection is awkward because Lantern lacks a narrow affordance, record or implement that as Lantern work. Keep the browser shim small: add commands or output fields only when they remove repeated friction in real frontend loops.

## Safety

- Do not expose local dashboards broadly; use non-loopback binding only for trusted local/container setups.
- Do not use `eval`-style browser operations unless the task explicitly requires it and the code is small, local, and inspectable.
- Do not add web mutation routes just because Lantern can dispatch click, type, key, or pointer interactions. Keep mutation design separate from read-only observability until the repo has explicit confirmation, audit, and recovery policy.
