# Review Rescue Prompt

You are repairing a task branch after agent review blocked landing.

## Selection

- Template: {{TEMPLATE_NAME}}
- Reason: {{SELECTION_REASON}}
- Rescue attempt: {{ATTEMPT}} of {{MAX_ATTEMPTS}}

## Run Context

- Run ID: {{RUN_ID}}
- Task ID: {{TASK_ID}}
- Task title: {{TASK_TITLE}}
- Branch: {{BRANCH_NAME}}
- Target branch: {{TARGET_BRANCH}}
- Worktree: {{WORKTREE_PATH}}
- Artifact dir: {{ARTIFACT_DIR}}
- Agent review report: {{AGENT_REVIEW_REPORT_PATH}}
- Landing review bundle: {{LANDING_REVIEW_PATH}}

## Blocking Review

```json
{{AGENT_REVIEW_REPORT}}
```

## Landing Bundle

```text
{{LANDING_REVIEW}}
```

## Last Implementation Message

```text
{{LAST_MESSAGE}}
```

## Operator Instructions

{{USER_INSTRUCTIONS}}

## Rescue Expectations

- Fix only the blocking review findings unless operator instructions explicitly expand scope.
- Do not weaken or remove tests to make the review pass.
- Do not land, merge, push, delete branches, mark tasks done, or edit tracker state.
- {{VALIDATION_HINT}}
- Prefer `cargo check` over `cargo build` when you only need compile feedback.
- Run focused validation for the changed area, then run the repo-standard validation entrypoint when practical.
- Leave the worktree ready for Smoogle to commit and send through a fresh agent review.
- If the review finding is invalid or already fixed, document that in your final message and make the smallest durable clarification needed.
