# Initial Architecture for Lantern

## Chosen Starting Point

- implementation language: Rust
- primary interface: cli
- primary storage: sqlite
- future interfaces reserved: tui, web ui

## Why This Shape

This repo starts with a narrow shape because the hardest part of a greenfield project is getting off the blank page without overcommitting to architecture too early.

## Immediate Boundaries

- core domain logic belongs in `lantern-core`
- persistence belongs in `lantern-storage`
- operator interaction belongs in `lantern-cli`

## Deferred Decisions

- richer UI surfaces
- multi-user behavior
- sync or networking
- background services beyond what the first milestone requires

## Synthesized Rationale

- architecture style: Thin Rust CLI facade over Chromium CDP with explicit boundaries between command handling, protocol transport, domain summaries, output formatting, and redaction.
- persistence model: SQLite is reserved for local task, state, and future session metadata; v1 must not persist credentials, cookies, local storage, DOM dumps, screenshots, full URLs, or sensitive browser artifacts by default.

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
