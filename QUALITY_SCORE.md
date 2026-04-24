# Quality Score

This file is the repository's current quality scorecard. It is a contract for future agents, not a changelog.

## Current Grade

Overall grade: **C+**

Last reviewed: **2026-04-24**

Rationale: `Lantern` now has a Rust workspace with a narrow implemented CLI contract for local Chromium CDP inspection and frontend feedback: `doctor`, `targets`, `page`, `dom`, explicit `--target-id` selection, `open`, `wait`, `console`, `network`, `layout`, `screenshot`, `click`, `type`, session-oriented flow observation, and explicit opt-in managed browser containers. The command surface is documented, fixture-tested, redacted by default, and covered by the standard validation script. Two-instance Podman and Docker managed-browser smokes have passed, but the repo remains below `B` until real-browser smoke coverage, recovery behavior, and unattended landing confidence are proven across representative local setups.

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
| Architecture boundaries | C+ | Crates separate core CDP services, storage, and CLI presentation; managed browser registry/runtime command construction lives in `lantern-storage` while endpoint-based CDP commands remain in the CLI/core path. | Boundaries still need pressure from future non-CLI adapters and broader runtime behavior before they can be treated as stable. |
| Workflow clarity | B- | `AGENTS.md`, `PLANS.md`, `docs/workflows.md`, completed ExecPlans, CLI contract docs, and validation scripts describe the expected agent and operator loops. | Guidance should keep tightening as recovery, landing, and browser-matrix evidence accumulates. |
| Validation posture | C+ | `scripts/validate.sh` covers formatting, `cargo check`, workspace tests, and docs hygiene; fixture tests cover the implemented CLI feedback loop; two-instance managed-browser smokes have passed with rootless Podman and Docker. | Real Chromium smoke coverage and broader host/container matrix evidence remain limited because normal validation does not build the image or require a container daemon. |
| Maintainability | C+ | Product specs, design docs, completed ExecPlans, current-state scorecards, and the tech debt tracker provide durable context without rewriting milestone history. | Future command expansion needs the same docs-first discipline to avoid drifting from the narrow v1 contract. |
| Automation readiness | C | Checked-in prompt templates, conservative `smoogle.toml` defaults, and stable JSON/error contracts support assisted automation. | Auto landing should wait until validation, review, and real-browser signals are consistently clean. |

## Quality Bar For Changes

- Keep changes aligned with the first milestone before expanding scope.
- Preserve the core, storage, and CLI boundaries unless an ExecPlan justifies changing them.
- Update durable docs when behavior, workflow, validation, reliability, or security expectations change.
- Run the strongest affordable validation for the touched surface.
- Record non-blocking quality gaps in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.

## Maintenance Rules

Update this file when a change materially affects repo quality, validation expectations, architecture coherence, or unattended-work confidence.

Do not append every task or run. Keep chronology in ExecPlan progress notes, task notes, and run artifacts. Replace stale evidence with current evidence.
