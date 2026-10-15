# Contributing to Lantern

Lantern is a Rust-first local CLI shim over Chromium CDP for agentic frontend
development. This guide is the repository's table of contents for contributors.
Keep it short and put durable detail in `docs/`.

## Start here

- `README.md`: project overview and quick start
- `PLANS.md`: ExecPlan policy and current active plans
- `ARCHITECTURE.md`: top-level architecture entrypoint
- `QUALITY_SCORE.md`: current repo quality scorecard
- `RELIABILITY.md`: operational reliability expectations
- `SECURITY.md`: local threat model and automation boundaries
- `docs/architecture.md`: module boundaries and state layout
- `docs/product-specs/lantern.md`: generated product spec for the first milestone
- `docs/workflows.md`: operator, validation, and quality workflows

## ExecPlans

- Use an ExecPlan for substantial work. See `PLANS.md`.
- Active ExecPlans live under `docs/exec-plans/active/`.
- Completed ExecPlans live under `docs/exec-plans/completed/`.
- Keep plans current while implementing, not just at the start.

## Core rules

- Keep this guide short. Put durable detail in `docs/`.
- Prefer adding reusable guidance to docs over embedding long prompts in code.
- Optimise first for the first milestone before expanding scope.
- Prefer the repo-standard validation entrypoint when one exists.
- Store local runtime state under `.lantern/` and keep it out of Git.
