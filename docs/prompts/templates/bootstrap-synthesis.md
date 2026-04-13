# Bootstrap Synthesis Prompt

You are synthesizing a Smoogle greenfield bootstrap contract from raw operator answers.

Return only one JSON object. Do not include Markdown fences, comments, prose, or trailing explanation.

## Input

The harness will provide a raw bootstrap brief containing:

- project name and summary
- primary user
- problem statement
- first milestone
- primary interface
- storage model
- future interfaces
- desired capabilities
- non-goals
- constraints
- optional additional freeform brief loaded from `--brief-file`

If the structured answers are sparse or generic but an additional freeform brief is present, treat the freeform brief as authoritative project context while preserving explicit structured answers where they are clearly intentional.

## Output Contract

Emit JSON matching this first-version `BootstrapSynthesis` shape:

```json
{
  "schema_version": 1,
  "project": {
    "name": "Project Name",
    "slug": "project-name",
    "summary": "One-sentence project summary.",
    "primary_user": "Primary operator or user.",
    "problem_statement": "The concrete problem this project solves.",
    "first_milestone": "The first useful milestone.",
    "primary_interface": "cli",
    "storage_model": "sqlite",
    "future_interfaces": ["tui", "web ui"],
    "constraints": ["rust-first", "local-first"],
    "non_goals": ["cloud sync"]
  },
  "product": {
    "value_proposition": "Why this should exist.",
    "v1_capabilities": ["capture work", "list work"],
    "success_criteria": ["operator can complete the first workflow locally"]
  },
  "architecture": {
    "style": "Short architecture style statement.",
    "core_components": ["domain model", "persistence adapter", "cli adapter"],
    "persistence_model": "Where durable state lives and why.",
    "integration_boundaries": ["future UI adapters use storage services"]
  },
  "docs": {
    "product_spec_outline": ["Behavior area to cover"],
    "architecture_notes": ["Boundary or state-layout note"],
    "workflow_notes": ["Operator workflow note"]
  },
  "tasks": [
    {
      "id": "task_review_contract",
      "title": "Review generated contract",
      "details": "Read generated docs and correct incorrect assumptions.",
      "status": "ready",
      "priority": 10,
      "depends_on": []
    }
  ],
  "risks": ["Primary delivery or design risk."],
  "open_questions": ["Question that should be resolved before deeper implementation."]
}
```

## Validation Rules

- Use `schema_version: 1`.
- Fill every required string with concrete project-specific content.
- Keep `project.summary` and task titles concise.
- Keep short lists bounded: prefer 3-7 items, never more than 12.
- Emit 1-24 tasks.
- Task IDs must start with `task_` and use only lowercase ASCII letters, digits, hyphens, or underscores.
- Task status must be `ready` or `inbox`.
- Use `ready` only for root tasks with no dependencies.
- Use `inbox` for any task that depends on another synthesized task.
- Task priority must be an integer from 0 to 10.
- Task dependencies must reference task IDs in the same `tasks` list.
- Do not create self-dependencies or dependency cycles.
- Keep dependencies sparse and ordered around real implementation prerequisites.
- Preserve operator constraints and non-goals instead of overriding them.
- Do not invent network, hosted-service, authentication, or multi-user requirements unless the operator asked for them.

Return only the JSON object.
