# Workflows

## Planning Loop

1. Create or update an ExecPlan before substantial work.
2. Keep the active plan current while implementation changes reality.
3. Move plans to `completed/` once the milestone is materially done.
4. If no active plans remain, keep `docs/exec-plans/active/.gitkeep` tracked so the required active-plan directory survives checkout.

## Product Loop

1. Review generated specs and assumptions.
2. Refine only the parts that are materially wrong.
3. Prefer narrow end-to-end milestones over broad speculative scaffolding.

## Browser Setup Loop

1. Choose either an operator-owned endpoint or an explicit managed instance; existing inspection commands never auto-start a browser.
2. For operator-owned Chromium, use the headless, container, or VNC-compatible pattern in `docs/headless-chromium.md`.
3. For a disposable managed instance, build `ops/browser-cdp/Containerfile`, then run `lantern browser start`.
4. For a repeated authenticated journey, create one dedicated profile with
   `lantern browser profile create <NAME>` and reuse it through
   `lantern browser start --profile <NAME>`; do not use a daily browser profile.
5. Keep CDP bound to loopback wherever possible, or publish container CDP ports to host loopback only.
6. Verify the endpoint with `lantern doctor --endpoint <ENDPOINT>`.
7. Use `lantern targets` and `lantern page` only after `doctor` confirms the endpoint behaves like Chromium CDP.

## Hardware WebGPU Browser Loop

1. Use only a disposable managed profile and a trusted application URL; the
   mode opts into Chrome's unsafe WebGPU boundary.
2. Select the device explicitly, for example
   `lantern browser start --graphics webgpu --gpu-device nvidia.com/gpu=0 --json`.
3. Confirm `browser start` output reports `graphics=webgpu`, the intended
   device, and `unsafe_webgpu=true`.
4. Use `flow` so console and network collection begin before navigation.
5. Require application readiness and visibly nonblank WebGPU canvas pixels;
   CDP readiness and an empty console alone do not prove rendering.
6. Inspect interactions and capture only the bounded screenshots the task
   needs, then explicitly run `browser stop` and `browser prune`.
7. Treat the result as selected Linux/Vulkan hardware evidence, not a
   replacement for production Chrome, Metal, D3D12, or other-device coverage.

## Browser Session Observation Loop

1. Use `lantern flow --open <URL> --timeout-ms <MS> --quiet-ms <MS>` when the agent needs one coherent `open -> wait -> inspect` observation rather than separate snapshot commands.
2. Treat `flow.console.collection_gap=false` and `flow.network.collection_gap=false` as evidence that collection started before the observed navigation, not as proof that no unobserved browser history exists outside the flow.
3. Use `lantern console` and `lantern network` for quick ad hoc snapshots, but prefer `flow` when judging whether a code change introduced fresh page-load errors.
4. Keep interaction commands separate from `flow` until a later ExecPlan defines multi-step interaction sessions explicitly.

## Authenticated Browser Loop

1. Follow `docs/authenticated-browser-testing.md` before connecting Lantern to a logged-in profile.
2. Use a throwaway or dedicated Chromium profile, never a daily browser profile.
3. Keep the CDP endpoint loopback-only, including container port publishing.
4. Run read-only smoke commands before navigation or interaction commands.
5. Avoid `--no-redact` and review screenshot output paths because authenticated page output and pixels remain sensitive.
6. Stop and prune the managed instance after inspection, but preserve its named
   profile. Use explicit confirmed deletion only when retiring the session.

## Task Review Loop

1. Keep speculative or dependent work in `inbox` until it has been semantically reviewed.
2. Use `task review <task-id> --apply` when prior landed work may have satisfied or reshaped an unblocked task.
3. Prefer `run queue --review-inbox` over `--allow-inbox` for unattended dependent chains.
4. Treat `promote`, `already-satisfied`, `reshape`, and `block` as durable task-state decisions, not chat-only guidance.

## Codex Loop

1. Use the generated docs as the system of record.
2. Start with `AGENTS.md`, the active ExecPlan, and `docs/workflows.md` before reading wider repo docs.
3. Read additional product or design docs only when the task needs them.
4. Work in bounded loops and validate each slice.
5. Use `--instructions-file -` with a quoted heredoc when operator instructions contain shell-sensitive text such as backticks or angle brackets.

## Validation Loop

1. Use `scripts/validate.sh fast` for tight edit loops. It runs formatting, `cargo check`, optional focused tests from `FAST_TEST_ARGS`, and docs hygiene.
2. Use `scripts/validate.sh` before landing Rust code. It runs formatting,
   current-toolchain and declared Rust 1.85 workspace checks, the workspace test
   suite using `cargo nextest` when available or `cargo test` otherwise, and
   docs hygiene. Install the `1.85.0` rustup toolchain before running the
   standard profile.
3. Use `scripts/quality-sweep.sh` periodically for slower checks: clippy, dependency advisory posture, typo scanning, TOML formatting, dependency cleanup, and optional coverage evidence.
4. Prefer `cargo check` over `cargo build` when you only need compile feedback.
5. Run `smoogle docs check` after changing scorecards, ExecPlan links, prompt templates, or core docs structure. In Codex child runs, use the injected `smoogle` shim or `"$SMOOGLE_BIN" docs check` fallback.
6. Use ad hoc commands only when isolating a specific failure or narrowing a validation step.

## Quality Scorecard Loop

1. Use `QUALITY_SCORE.md`, `RELIABILITY.md`, and `SECURITY.md` as current-state scorecards, not chronological logs.
2. Update scorecards when a change materially affects quality, reliability, security, validation expectations, recovery guidance, or automation safety.
3. Put deferred work in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.
4. For docs-only scorecard changes, inspect the diff and run `smoogle docs check` before landing.

## Review And Landing Loop

1. Use `review codex --run-id <run-id>` when you want an agent-written landing recommendation before applying a change.
2. Prefer `landing_policy = "assisted"` while a repo is still stabilizing.
3. Use `landing_policy = "auto"` only when validation, review prompts, and task hygiene are consistently clean.
4. Treat `already-satisfied` as a valid review outcome for tasks that no longer require a code change.
5. Inspect dependent-task reconciliation after landing and use task review before executing newly unblocked inbox work.

## Transport budgets and incomplete evidence

`--timeout-ms` is one deadline shared by HTTP target selection, WebSocket
attachment, domain setup, polling, input dispatch, flow collection and flow
finalisation. It starts before target selection; it is not restarted after
navigation or each event. A short budget can therefore fail during setup before
an observation result is available. Browser readiness polling also shares its
`--wait-ms` deadline with HTTP requests. Commands without a caller-supplied
budget retain a ten-second bound per HTTP request, attachment or CDP call;
console and network attachment/collection share a ten-second budget.

Transport accepts local numeric addresses and resolves `localhost` through
literal IPv6 and IPv4 loopback addresses without DNS. HTTP redirects are rejected.
HTTP bodies are limited to 4 MiB; WebSocket frames and assembled messages are
limited to 16 MiB. An oversized response is a transport failure, including an
oversized screenshot response. The pending event queue retains at most 1024
events and 4 MiB of encoded messages. Flow drains yield after 1024 events or
20 ms. Console/network collectors reject individual payloads over 256 KiB;
network request correlation retains at most 1024 requests and 4 MiB of encoded
request payloads and releases metadata when loading finishes or fails.

Incomplete collection adds `evidence_loss` to the relevant summary (or wait
output): `dropped_events`, `dropped_event_bytes`, `drain_limit_reached` and
`collection_deadline_reached`. The field is omitted when no loss is recorded.
Human output includes an `evidence_loss:` line when present. These are
conservative completeness indicators: reaching a drain or collection deadline
means remaining traffic was not ruled out. An event read slice that times out
after consuming part of a frame also marks the collection deadline indicator.
Console and network set `truncated`
and clear `observed_clean` when evidence is lost; quiet waits cannot report a
match from lost transport evidence. Historical `collection_gap` remains a
separate statement about when observation began. A missing loss field does not
remove that historical gap or prove application correctness.

A command sent without a definitive response returns `cdp_command_uncertain`.
The command may have executed; inspect state before deciding whether another
action is needed, and never automatically replay it. A drag failure attempts
one best-effort pointer release, including an unacknowledged press, using an
additional budget of at most 100 ms. Release acknowledgement is not guaranteed.
CPU processing and operating-system scheduling can add bounded-work overhead to
these I/O deadlines; this is not a hard real-time guarantee. Existing `ok` and
exit-status semantics for completed condition results remain unchanged.
