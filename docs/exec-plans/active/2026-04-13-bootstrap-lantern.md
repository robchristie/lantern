# ExecPlan: Bootstrap Lantern

Status: active

## Progress Notes

- 2026-04-13: generated the initial docs contract, Rust workspace skeleton, validation profile scripts, and seed tasks from the bootstrap interview.

## Goal

Move from a rough idea to a usable repo that can implement the first milestone: Build a CLI core that connects to an operator-provided Chromium CDP endpoint and proves a small browser-inspection loop.

## Why This Matters

Greenfield projects stall when the repo has no docs contract, no clear milestone, and no bounded first slice. This plan exists to create enough structure to start moving without forcing the operator to write the first spec from scratch.

## Constraints

- local-first by default
- primary interface: cli
- primary storage: sqlite
- constraints: rust-first, local-first, linux-only initially, headless linux, single-user, operator-managed Chromium/CDP endpoint, minimal dependencies, no hidden network dependency beyond the explicit CDP endpoint

## Non-Goals

- multi-user sync
- cloud hosting
- auth
- MCP server in the first milestone
- full DevTools replacement
- browser launch or container orchestration in the CLI
- cross-browser abstraction beyond Chromium CDP

## Implementation Slices

### Slice 1: Assumption Review

- confirm the generated product spec and architecture assumptions
- correct only the parts that are materially wrong

### Slice 2: Core Foundations

- implement domain types and persistence boundaries
- keep the first slice aligned to the first milestone

### Slice 3: First End-to-End Flow

- deliver one usable `cli` workflow
- validate it with a small smoke path

## Validation

- `scripts/validate.sh fast` during tight edit loops
- `scripts/validate.sh` before landing code changes
- `scripts/quality-sweep.sh` periodically once the milestone has enough behavior to score
- one end-to-end manual or scripted smoke flow for the first milestone

## Synthesized Product Direction

- Agentic frontend feedback loop
- V1 command vocabulary and output expectations
- Sensitive data handling and default redaction
- First milestone acceptance criteria
- Explicit first-milestone non-goals

## Synthesized Success Criteria

- operator can point Lantern at an existing Chromium CDP endpoint
- operator can run doctor, targets, and page commands locally
- implemented commands support predictable --json output
- sensitive URLs and large text fields are redacted or truncated by default
- local tests cover CDP fixture behavior where practical

## Initial Task Queue

- `task_review_contract` (ready, priority 10, depends on: none): Review generated contract
- `task_initialize_rust_workspace` (inbox, priority 9, depends on: task_review_contract): Initialize Rust workspace
- `task_define_cli_contract` (inbox, priority 9, depends on: task_initialize_rust_workspace): Define CLI contract
- `task_design_output_policy` (inbox, priority 9, depends on: task_define_cli_contract): Design output policy
- `task_implement_endpoint_config` (inbox, priority 8, depends on: task_define_cli_contract): Implement endpoint config
- `task_implement_cdp_transport` (inbox, priority 8, depends on: task_implement_endpoint_config): Implement CDP transport
- `task_implement_doctor` (inbox, priority 8, depends on: task_implement_cdp_transport, task_design_output_policy): Implement doctor command
- `task_implement_targets` (inbox, priority 8, depends on: task_implement_cdp_transport, task_design_output_policy): Implement targets command
- `task_implement_page` (inbox, priority 8, depends on: task_implement_targets): Implement page command
- `task_add_fixture_tests` (inbox, priority 7, depends on: task_implement_doctor, task_implement_targets, task_implement_page): Add CDP fixture tests
- `task_document_headless_setup` (inbox, priority 6, depends on: task_implement_doctor): Document headless setup
- `task_add_quality_gates` (inbox, priority 6, depends on: task_add_fixture_tests): Add quality gates
- `task_write_next_execplan` (inbox, priority 5, depends on: task_add_quality_gates, task_document_headless_setup): Write next ExecPlan

## Open Questions

- Should SQLite be initialized in the first milestone or deferred until session and task state need durable storage?
- How should Lantern choose the current page target when multiple page targets are open?
- What exact URL-shape format is safe and useful enough for agent feedback?
- Which local fixture approach should be used for deterministic CDP HTTP and WebSocket tests?
- Should config fallback read from a standard file path in v1, and if so what path?
