# Task Recovery Prompt

You are resuming a tracked task after a failed or incomplete run.

## Selection

- Template: {{TEMPLATE_NAME}}
- Reason: {{SELECTION_REASON}}

## Task Context

- Task ID: {{TASK_ID}}
- Title: {{TASK_TITLE}}
- Status: {{TASK_STATUS}}
- Priority: {{TASK_PRIORITY}}
- Branch: {{TASK_BRANCH_NAME}}

## Task Details

{{TASK_DETAILS}}

## Dependencies

{{TASK_DEPENDENCIES}}

## Recent Runs

{{TASK_RECENT_RUNS}}

## Recent Task Events

{{TASK_RECENT_EVENTS}}

## Operator Instructions

{{USER_INSTRUCTIONS}}

## Recovery Expectations

- Inspect the most recent run artifacts before changing code.
- Start with `AGENTS.md`, the active ExecPlan from `PLANS.md`, and `docs/workflows.md` before reading wider repo docs.
- Identify whether the failure came from code, prompt quality, missing repo guidance, or environment assumptions.
- {{VALIDATION_HINT}}
- Prefer `cargo check` over `cargo build` when you only need compile feedback.
- Update active ExecPlan progress notes only when the recovery materially changes implementation reality, and do not move, archive, or close ExecPlans unless the task or operator instructions explicitly ask for planning hygiene or milestone closure.
- If you close or move the last active ExecPlan, keep `docs/exec-plans/active/.gitkeep` tracked so docs hygiene still passes after checkout.
- Prefer durable fixes in code, docs, prompts, or validations over retrying with vague changes.
- Leave the repository in a clearer state for the next unattended run.
