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
4. Keep CDP bound to loopback wherever possible, or publish container CDP ports to host loopback only.
5. Verify the endpoint with `lantern doctor --endpoint <ENDPOINT>`.
6. Use `lantern targets` and `lantern page` only after `doctor` confirms the endpoint behaves like Chromium CDP.

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
2. Use `scripts/validate.sh` before landing Rust code. It runs formatting, `cargo check`, the workspace test suite using `cargo nextest` when available or `cargo test` otherwise, and docs hygiene.
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
