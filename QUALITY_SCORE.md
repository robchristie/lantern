# Quality Score

This file is the repository's current quality scorecard. It is a contract for future agents, not a changelog.

## Current Grade

Overall grade: **B-**

Last reviewed: **2026-09-07**

Rationale: Lantern has bounded CDP transport, guarded semantic/CSS interactions,
explicit action postconditions, heuristic layout, opened-image workflows and
source-correlated owner application evidence. Canonical verification covers 231
Rust tests plus four adjudication contracts; 64 real-Chromium cases audit actual
input and application state. A paired agent case study and repeated software-
rendered Polyorama shell qualify representative use. Known gaps remain bounded:
broader host/container and hardware coverage, app-wide readiness, non-atomic
capture correlation and agent choices that can leave successful actions unverified.
See `docs/qualification/observable-ui-assessment.md` for the evidence envelope.

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
| Validation posture | B | `scripts/validate.sh` covers formatting, `cargo check`, workspace tests, and docs hygiene; fixture tests cover the implemented CLI feedback loop, including click/type/key/pointer interaction metadata and screenshot regions; two-instance managed-browser smokes have passed with rootless Podman and Docker. | A separate real-Chromium interaction CI job verifies fixture state, strict exits and blockers; action-flow and bounded heuristic layout have real-browser contracts; the paired five-task case study and actual Polyorama shell are qualified; the broader host/container/hardware matrix remains incomplete. |
| Maintainability | C+ | Product specs, design docs, completed ExecPlans, current-state scorecards, and the tech debt tracker provide durable context without rewriting milestone history. | Future command expansion needs the same docs-first discipline to avoid drifting from the narrow v1 contract. |
| Automation readiness | B- | Checked-in prompt templates, conservative `smoogle.toml` defaults, and stable JSON/error contracts support assisted automation. | Exact-revision review and passing CI remain landing gates. Agent verification is independently adjudicated; the comparison retained one owner interruption and conductor-directed rerun, plus missed acknowledgements. |

## Quality Bar For Changes

- Keep changes aligned with the first milestone before expanding scope.
- Preserve the core, storage, and CLI boundaries unless an ExecPlan justifies changing them.
- Update durable docs when behavior, workflow, validation, reliability, or security expectations change.
- Run the strongest affordable validation for the touched surface.
- Record non-blocking quality gaps in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.

## Maintenance Rules

Update this file when a change materially affects repo quality, validation expectations, architecture coherence, or unattended-work confidence.

Do not append every task or run. Keep chronology in ExecPlan progress notes, task notes, and run artifacts. Replace stale evidence with current evidence.
