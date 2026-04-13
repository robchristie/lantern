# Quality Score

This file is the repository's current quality scorecard. It is a contract for future agents, not a changelog.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-04-13**

Rationale: `Lantern` has a generated docs contract, an initial Rust workspace, prompt templates, validation profile scripts, and a bootstrap ExecPlan. Codex synthesis also provided project-specific success criteria and task seeds: operator can point Lantern at an existing Chromium CDP endpoint, operator can run doctor, targets, and page commands locally, implemented commands support predictable --json output, sensitive URLs and large text fields are redacted or truncated by default, local tests cover CDP fixture behavior where practical. The grade starts at `C` until the first milestone proves the architecture, tests, and validation path with real behavior.

## Grade Scale

- `A`: clean architecture, strong tests, deterministic validation, current docs, and unattended landing confidence.
- `B`: coherent architecture and validation with known gaps that are tracked and bounded.
- `C`: usable but dependent on operator review while product behavior, tests, and docs mature.
- `D`: unclear ownership boundaries, stale docs, or weak validation signals.
- `F`: unsafe to run unattended because basic build, tests, docs, or repository contracts are missing.

Use `+` or `-` only when the repo is clearly between two grades.

## Rubric

| Dimension | Current | Evidence | Known gaps |
| --- | --- | --- | --- |
| Architecture boundaries | C | Initial crates separate core, storage, and CLI concerns. | Boundaries are not yet proven by real feature pressure. |
| Workflow clarity | C+ | `AGENTS.md`, `PLANS.md`, `docs/workflows.md`, prompt templates, and the bootstrap ExecPlan provide a starting path. | Guidance should be refined after the first real implementation slices. |
| Validation posture | C+ | The workspace has `scripts/validate.sh` for fast and standard loops, plus `scripts/quality-sweep.sh` for slower periodic checks. | Feature-specific tests and smoke paths still need to mature with real behavior. |
| Maintainability | C | Product specs, design docs, ExecPlans, and the tech debt tracker exist from the start. | Generated assumptions need operator review before they become durable truth. |
| Automation readiness | C | The repo has checked-in prompt templates and conservative `smoogle.toml` defaults. | Auto landing should wait until validation and review signals are consistently clean. |

## Quality Bar For Changes

- Keep changes aligned with the first milestone before expanding scope.
- Preserve the core, storage, and CLI boundaries unless an ExecPlan justifies changing them.
- Update durable docs when behavior, workflow, validation, reliability, or security expectations change.
- Run the strongest affordable validation for the touched surface.
- Record non-blocking quality gaps in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.

## Maintenance Rules

Update this file when a change materially affects repo quality, validation expectations, architecture coherence, or unattended-work confidence.

Do not append every task or run. Keep chronology in ExecPlan progress notes, task notes, and run artifacts. Replace stale evidence with current evidence.
