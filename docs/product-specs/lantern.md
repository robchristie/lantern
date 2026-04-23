# Lantern Product Spec

## Summary

Rust-first local CLI shim over Chromium CDP for agentic frontend development.

## Primary User

A technically fluent local operator using Codex or Smoogle on headless Linux.

## Problem

Agents need concise browser feedback during frontend work, but broad DevTools integrations add too much tool context, compaction pressure, and setup brittleness.

## First Milestone

Build a CLI core that connects to an operator-provided Chromium CDP endpoint and proves a small browser-inspection loop.

## Core Capabilities

- verify CDP endpoint reachability
- list Chromium targets with sanitized metadata
- summarize the current page target
- emit concise human output
- emit stable JSON output
- apply redaction and truncation rules
- document operator-owned headless, container, and VNC-compatible Chromium CDP setup in `docs/headless-chromium.md`

## First-Milestone CLI Surface

The current milestone keeps browser inspection endpoint-based while adding an explicit opt-in managed disposable Chromium/CDP container lifecycle. The original inspection surface started with:

- `lantern doctor`
- `lantern targets`
- `lantern page`

Later ExecPlans have added bounded navigation, interaction, screenshots,
DOM/layout/console/network summaries, one session-observation flow, and the
explicit managed browser lifecycle. JavaScript evaluation and broad DevTools
replacement behavior remain out of scope.

The exact command, shared flag, endpoint resolution, and exit-code contract lives in `docs/product-specs/cli-contract.md`. The shared human-output, stable JSON ordering, redaction, truncation, and future artifact output policy lives in `docs/product-specs/output-policy.md`.

## Non-Goals

- multi-user sync
- cloud hosting
- auth
- MCP server in the first milestone
- full DevTools replacement
- implicit browser launch or container orchestration from endpoint-based inspection commands
- cross-browser abstraction beyond Chromium CDP
- JavaScript evaluation and broad DOM/console/network tooling
- persistence of credentials, cookies, local storage, DOM dumps, screenshots, full URLs, or other sensitive browser artifacts by default

## Acceptance Direction

- deliver one end-to-end `cli` workflow that proves the core product loop
- keep the first release local-first and single-user
- keep endpoint-based inspection usable without durable product storage; store managed browser runtime records only under untracked `.smoogle/` and reserve SQLite for local task, state, and future session metadata once a real need appears

## Value Proposition

Lantern gives coding agents a compact, predictable, text-first browser interface without the overhead and brittleness of broad DevTools integrations.

## Synthesized Spec Outline

- Agentic frontend feedback loop
- V1 command vocabulary and output expectations
- Sensitive data handling and default redaction
- First milestone acceptance criteria
- Explicit first-milestone non-goals

## Success Criteria

- operator can point Lantern at an existing Chromium CDP endpoint
- operator can run doctor, targets, and page commands locally
- implemented commands support predictable --json output
- sensitive URLs and large text fields are redacted or truncated by default
- local tests cover CDP fixture behavior where practical

## Open Questions

- Which local fixture approach should be used for deterministic CDP HTTP and WebSocket tests?
