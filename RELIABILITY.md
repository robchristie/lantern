# Reliability

This file records the repository's current reliability contract. Keep it focused on operational expectations, known weak points, and recovery guidance.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-04-15**

Rationale: `Lantern` has local-first docs, a Rust workspace, prompt templates, validation profile scripts, conservative harness defaults, fixture coverage for the implemented frontend feedback loop, and session-oriented flow observation that keeps console/network evidence tied to one browser lifecycle. The grade remains `C` until real-browser smoke coverage, recovery behavior, and landing behavior are proven across representative local setups.

## Grade Scale

- `A`: routine unattended operation, deterministic recovery paths, strong validation, and low flake rate.
- `B`: reliable for normal local use with documented manual recovery and bounded known gaps.
- `C`: useful but still dependent on operator review while product behavior and validation mature.
- `D`: frequent manual intervention, unclear state transitions, or weak artifact capture.
- `F`: basic task/run state cannot be trusted.

Use `+` or `-` only when the repo is clearly between two grades.

## Operational Expectations

- Keep runtime state under `.smoogle/` and out of Git.
- Create an initial Git commit before task worktrees are used.
- Treat checked-in docs and prompt templates as the system of record.
- Prefer local-first workflows and deterministic local validation.
- Keep recovery guidance in checked-in docs, task notes, or run artifacts instead of chat history.

## Validation Budget

- Docs-only changes: inspect the diff and run `smoogle docs check`.
- Tight Rust edit loops: run `scripts/validate.sh fast`, using `FAST_TEST_ARGS` when a focused test filter is known.
- Standard Rust validation: run `scripts/validate.sh`, which composes formatting, `cargo check`, workspace tests, and docs hygiene.
- Periodic quality sweeps: run `scripts/quality-sweep.sh`; set `SMOOGLE_COVERAGE=1` when coverage evidence is worth the runtime cost.
- Prompt/template changes: verify template paths and inspect rendered prompt behavior where practical.
- Workflow changes: include a command-level smoke path or test when possible.

## Recovery Guidance

1. Inspect `smoogle run show <run-id>` before editing prompts or code after a failed run.
2. Use task notes for durable context that should travel with a task.
3. Prefer fixing repo-local guidance, checks, templates, or code over repeating one-off operator instructions.
4. Put real but deferred reliability work in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.

## Known Reliability Gaps

- Feature-specific fixture tests cover the implemented CLI loop and flow observation path, but real-browser smoke coverage is still limited.
- Recovery and landing behavior have not yet been proven by repeated assisted or unattended runs.
- Headless Linux, container, and VNC-compatible Chromium setup is documented in `docs/headless-chromium.md`, but the matrix has not been smoke-tested across representative hosts and images yet.
- Optional quality-sweep tools may not be installed on every machine.
- CDP implementation complexity could grow beyond the intended small command vocabulary if future tasks skip the CLI contract and ExecPlan gates.
- Stable JSON contracts require compatibility discipline as command output evolves.
- Minimal dependency choices could increase protocol maintenance burden.

## Maintenance Rules

Update this file when reliability expectations, validation budgets, recovery paths, landing confidence, or known flaky surfaces change. Do not use it as a run log.
