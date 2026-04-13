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
- `lantern-storage`: persistence boundary for sqlite
- `lantern-cli`: first operator surface for the initial workflow

## Local State

- checked-in docs define the system of record
- local runtime state can live under `.smoogle/` while using the harness
- product data should stay local-first by default

## Near-Term Extension Points

- richer domain behavior once the first flow is stable
- future interfaces such as tui, web ui
- stronger validation and smoke-test automation

## Synthesized Architecture

- style: Thin Rust CLI facade over Chromium CDP with explicit boundaries between command handling, protocol transport, domain summaries, output formatting, and redaction.
- persistence model: SQLite is reserved for local task, state, and future session metadata; v1 must not persist credentials, cookies, local storage, DOM dumps, screenshots, full URLs, or sensitive browser artifacts by default.

## Core Components

- CLI adapter
- configuration resolver
- CDP transport client
- browser target service
- page summary service
- redaction and truncation policy
- output formatter
- fixture test harness

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
