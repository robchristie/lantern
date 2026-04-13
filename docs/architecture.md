# Architecture

## Goals

- Rust-first local CLI shim over Chromium CDP for agentic frontend development.
- Keep the first milestone narrow and end-to-end.
- Preserve a clean seam for future interfaces such as tui, web ui.
- Stay aligned with these constraints: rust-first, local-first, linux-only initially, headless linux, single-user, operator-managed Chromium/CDP endpoint, minimal dependencies, no hidden network dependency beyond the explicit CDP endpoint.

## Documentation Layout

- `AGENTS.md`: minimal repo table of contents
- `PLANS.md`: ExecPlan policy and current active plan links
- `ARCHITECTURE.md`: top-level architecture entrypoint
- `QUALITY_SCORE.md`, `RELIABILITY.md`, `SECURITY.md`: current quality, reliability, and security scorecards
- `docs/exec-plans/`: substantial work plans and tech debt tracking
- `docs/prompts/templates/`: reusable run prompt templates
- `docs/design-docs/`: design rationale
- `docs/product-specs/`: behavior-level specs
- `docs/generated/`: generated summaries and interview output

## Workspace Shape

- `lantern-core`: domain types and business rules
- `lantern-storage`: reserved persistence boundary for SQLite-backed local task, state, and future session metadata; first-milestone CDP inspection should not require it
- `lantern-cli`: first operator surface for the initial workflow

## Local State

- checked-in docs define the system of record
- local runtime state can live under `.smoogle/` while using the harness
- product data should stay local-first by default
- first-milestone commands should avoid durable product storage unless required for explicit operator configuration

## Near-Term Extension Points

- richer domain behavior once the first flow is stable
- future interfaces such as tui, web ui
- stronger validation and smoke-test automation

## Synthesized Architecture

- style: Thin Rust CLI facade over Chromium CDP with explicit boundaries between command handling, protocol transport, domain summaries, output formatting, and redaction.
- persistence model: SQLite is reserved for local task, state, and future session metadata after the inspection loop proves it needs durable state. The first milestone should run without product persistence by default, and Lantern must not persist credentials, cookies, local storage, DOM dumps, screenshots, full URLs, or sensitive browser artifacts unless a later design explicitly opts in with redaction rules.

## Core Components

- CLI adapter
- configuration resolver
- CDP transport client
- browser target service
- page summary service
- redaction and truncation policy
- output formatter
- fixture test harness

## First-Milestone Command Boundary

The initial CLI contract is limited to:

- `lantern doctor`: confirm the configured CDP endpoint is reachable and report browser/version metadata.
- `lantern targets`: list Chromium targets using sanitized, bounded metadata.
- `lantern page`: summarize one page target with title, origin or URL shape, and loading state without exposing the full URL by default.

Navigation, page mutation, JavaScript evaluation, screenshots, DOM traversal, console summaries, network summaries, browser launch, container orchestration, daemon mode, and non-Chromium adapters remain outside this milestone.

The exact first-milestone command contract is maintained in `docs/product-specs/cli-contract.md`. Shared output formatting, JSON ordering, redaction, truncation, and future artifact rules are maintained in `docs/product-specs/output-policy.md`.

## Integration Boundaries

- CLI commands consume domain services rather than raw CDP responses
- JSON output shapes remain stable for future Smoogle and UI integration
- future TUI and web UI adapters reuse the same services and redaction policies
- operator-owned Chromium lifecycle stays outside Lantern in the first milestone
- future daemon mode remains optional and evidence-driven

## Architecture Notes

- CDP transport boundary and error model
- Command-to-service layering
- Stable JSON field ordering expectations
- SQLite local state boundary
- Future seams for DOM, console, network, screenshots, and interactions
