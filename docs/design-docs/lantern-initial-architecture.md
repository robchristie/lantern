# Initial Architecture for Lantern

## Chosen Starting Point

- implementation language: Rust
- primary interface: cli
- storage posture: local-first; SQLite is reserved for later local task, state, and session metadata rather than required for the first CDP inspection loop
- future interfaces reserved: tui, web ui

## Why This Shape

This repo starts with a narrow shape because the hardest part of a greenfield project is getting off the blank page without overcommitting to architecture too early.

## Immediate Boundaries

- core domain logic belongs in `lantern-core`
- persistence belongs in `lantern-storage` once there is a concrete first-party state need
- operator interaction belongs in `lantern-cli`
- browser launch, container orchestration, and the Chromium lifecycle belong to the operator in the first milestone

## Deferred Decisions

- richer UI surfaces
- multi-user behavior
- sync or networking
- background services beyond what the first milestone requires
- navigation, page mutation, screenshots, JavaScript evaluation, DOM traversal, console summaries, and network summaries

## Synthesized Rationale

- architecture style: Thin Rust CLI facade over Chromium CDP with explicit boundaries between command handling, protocol transport, domain summaries, output formatting, and redaction.
- persistence model: SQLite is reserved for local task, state, and future session metadata after the inspection loop proves a durable-state need; v1 must not persist credentials, cookies, local storage, DOM dumps, screenshots, full URLs, or sensitive browser artifacts by default.

## First-Milestone Command Boundary

- `lantern doctor`
- `lantern targets`
- `lantern page`

The first milestone should prove a read-oriented CDP inspection loop. Interaction commands and broader browser tooling are future work unless a later ExecPlan explicitly changes scope.

## Component Notes

- CLI adapter
- configuration resolver
- CDP transport client
- browser target service
- page summary service
- redaction and truncation policy
- output formatter
- fixture test harness

## Workflow Notes

- How to run Chromium with a CDP endpoint on headless Linux
- How to configure --endpoint and LANTERN_CDP_ENDPOINT
- How to run rustfmt, cargo check, cargo test, and clippy
- How Smoogle ExecPlans and task review should drive implementation
