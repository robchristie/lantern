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
- disposable managed-browser state lives under
  `.smoogle/lantern/browser-instances/` in the current repository and remains
  pruneable runtime state
- persistent browser profiles and their instance records live outside source
  worktrees under `LANTERN_STATE_HOME`, then `XDG_STATE_HOME/lantern`, then
  `$HOME/.local/state/lantern`
- persistent profile directories are owner-private and contain Chromium-owned
  session state; Lantern metadata stores only the name, timestamps and bounded
  current attachment, including an opaque per-start reservation token, never
  extracted cookies, credentials or storage values
- every persistent-profile lifecycle command holds an owner-private advisory
  operation lock for its complete start, stop, prune or delete transition;
  process exit releases the lock, while suspended and concurrent commands fail
  closed rather than sharing or deleting the profile
- instance identifiers and records are bound to direct, non-symlink children
  of their registry roots before any recursive cleanup
- product data should stay local-first by default
- first-milestone commands should avoid durable product storage unless required
  for explicit operator configuration

## Near-Term Extension Points

- richer domain behavior once the first flow is stable
- future interfaces such as tui, web ui
- stronger validation and smoke-test automation

## Synthesized Architecture

- style: Thin Rust CLI facade over Chromium CDP with explicit boundaries between command handling, protocol transport, domain summaries, output formatting, and redaction.
- persistence model: SQLite is reserved for local task, state, and future
  session metadata after the inspection loop proves it needs durable state.
  Disposable managed-browser instance metadata is stored as local JSON runtime
  state under `.smoogle/lantern/browser-instances/`. Explicitly named
  persistent profiles use private JSON metadata and Chromium data below the
  operator's Lantern state home. Lantern does not read, export or reproduce
  credentials, cookies or browser storage from those profiles. It must not
  persist DOM dumps, screenshots, full URLs or sensitive browser artefacts
  outside the profile unless a command explicitly opts in under the output
  policy.

## Core Components

- CLI adapter
- configuration resolver
- CDP transport client
- browser target service
- page summary service
- redaction and truncation policy
- output formatter
- fixture test harness

## Command Boundary

The endpoint-based CLI contract includes:

- `lantern doctor`: confirm the configured CDP endpoint is reachable and report browser/version metadata.
- `lantern targets`: list Chromium targets using sanitized, bounded metadata.
- `lantern page`: summarize one page target with title, origin or URL shape, and loading state without exposing the full URL by default.

The broader implemented contract also includes bounded navigation, waiting,
DOM/layout/console/network summaries, screenshots, click/type interactions,
flow observation, and explicit managed browser lifecycle commands.

JavaScript evaluation, full DevTools replacement behavior, daemon mode, and
non-Chromium adapters remain outside this milestone. Browser lifecycle support
is limited to the explicit managed container and named-profile commands
documented in the CLI contract; endpoint-based inspection remains independent.

The exact command contract is maintained in `docs/product-specs/cli-contract.md`. Shared output formatting, JSON ordering, redaction, truncation, and future artifact rules are maintained in `docs/product-specs/output-policy.md`.

## Integration Boundaries

- CLI commands consume domain services rather than raw CDP responses
- JSON output shapes remain stable for future Smoogle and UI integration
- future TUI and web UI adapters reuse the same services and redaction policies
- operator-owned Chromium endpoints remain valid; managed disposable containers
  and named persistent profiles are explicit opt-in lifecycle commands
- future daemon mode remains optional and evidence-driven

## Architecture Notes

- CDP transport boundary and error model
- Command-to-service layering
- Stable JSON field ordering expectations
- SQLite local state boundary
- Future seams for DOM, layout, console, network, screenshots, and interactions
