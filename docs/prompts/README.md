# Prompt Templates

This directory stores reusable, checked-in prompt templates.

## Strategy

- Reusable prompt templates live in `docs/prompts/templates/`.
- Per-run prompt instances are generated into `.smoogle/runs/<run-id>/prompt.md`.
- The generated prompt instance always records where it came from and why it was selected.

## Current Templates

- `ad-hoc`: for runs without task context
- `bootstrap-synthesis`: for opt-in Codex-backed greenfield bootstrap synthesis runs that must emit only `BootstrapSynthesis` JSON
- `review-rescue`: for repairing a task branch after agent review blocks landing
- `task-execution`: default for task-driven work
- `task-recovery`: default when the latest run for a task failed
- `ui-review`: for separate UI evaluator runs that score browser evidence and return structured review JSON

## Selection Rules

- Explicit `--prompt` or `--prompt-file` wins over all templates.
- Explicit `--template` wins over automatic template selection.
- If a task has a recent failed run, use `task-recovery`.
- Otherwise, task-driven runs use `task-execution`.
- Runs without task context use `ad-hoc`.
- Blocked agent reviews use `review-rescue` only through `run rescue-review` or an explicit `review_block_policy = "rescue-once"` auto-landing policy.
- UI evaluator runs use `ui-review` only through the UI review path, not normal task execution.
