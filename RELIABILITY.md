# Reliability

This file records the repository's current reliability contract. Keep it focused on operational expectations, known weak points, and recovery guidance.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-04-13**

Rationale: `Lantern` starts with local-first docs, a generated Rust workspace, seed tasks, prompt templates, validation profile scripts, and conservative harness defaults. The grade starts at `C` until real workflows prove state durability, validation, and recovery behavior.

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

- The generated product assumptions still need operator review.
- Feature-specific tests and smoke paths have not been written yet.
- The first milestone has not yet proven recovery and landing behavior.
- Optional quality-sweep tools may not be installed on every machine.

- Synthesized risk: CDP implementation complexity could grow beyond the intended tiny command vocabulary.; Redaction mistakes could leak sensitive URLs, storage data, DOM content, or screenshots.; Headless Linux and container setups may vary enough to make endpoint diagnostics brittle.; Stable JSON contracts may be difficult to preserve if early command shapes are underspecified.; Minimal dependency choices could increase protocol maintenance burden.

## Maintenance Rules

Update this file when reliability expectations, validation budgets, recovery paths, landing confidence, or known flaky surfaces change. Do not use it as a run log.
