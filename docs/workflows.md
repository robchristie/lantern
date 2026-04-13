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
