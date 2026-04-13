# Task Execution Prompt

You are executing a tracked task in this repository.

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

## Execution Expectations

- Start with `AGENTS.md`, the active ExecPlan from `PLANS.md`, and `docs/workflows.md`.
- Read additional product, design, or architecture docs only when they materially affect this task.
- Inspect the relevant code before editing.
- {{VALIDATION_HINT}}
- Prefer `cargo check` over `cargo build` when you only need compile feedback.
- Update active ExecPlan progress notes when this task materially changes implementation reality, but do not move, archive, or close ExecPlans unless the task or operator instructions explicitly call for planning hygiene or milestone closure.
- If you close or move the last active ExecPlan, keep `docs/exec-plans/active/.gitkeep` tracked so docs hygiene still passes after checkout.
- After successful validation, summarize the shipped behavior with key file references and stop unless repo guidance or the diff still looks inconsistent.
- Prefer changes that reduce future operator involvement.
