# Project Brief: Lantern

Lantern is a Rust-first, local-first browser shim optimized for agentic frontend development. It should provide a tiny, predictable CLI over Chromium's Chrome DevTools Protocol (CDP), replacing broad MCP-style DevTools integrations with a small command vocabulary that agents can learn and use reliably.

## Product Summary

Lantern exposes an opinionated, text-first CLI for inspecting and manipulating a browser during frontend development. The goal is not to reproduce all of Chrome DevTools or Playwright. The goal is to give coding agents a compact, stable interface for the common loop: open a page, inspect DOM/layout/state, read console and network errors, interact with a few elements, make a code change, and re-check.

## Primary User

A technically fluent local operator using Codex/Smoogle on headless Linux who wants frontend development feedback without MCP token overhead, broad tool-surface bloat, or client-specific DevTools integration problems.

## Problem To Solve

Agentic frontend work is currently hard because agents need browser feedback, but broad MCP integrations inject too much tool context, create compaction pressure, and can be brittle across Codex/headless/WSL/container setups. A narrow CLI can expose the small browser affordance set that frontend agents actually need, in concise text and JSON shapes controlled by the harness.

## Design Direction

Lantern should be a very thin agent-native facade over CDP:

- tiny command vocabulary, ideally 8-12 commands in v1
- concise human output by default
- stable JSON output with predictable field ordering for scripts and future harness/UI integration
- aggressive truncation and redaction rules
- text-first summaries of browser state, with screenshots as supporting artifacts rather than the primary interface
- direct CDP usage under the hood where it helps, but internal boundaries should prevent raw CDP complexity from leaking through the CLI

## Candidate V1 CLI Surface

The exact surface can evolve, but the intended shape is:

- `lantern doctor`
- `lantern targets`
- `lantern open <url>`
- `lantern page`
- `lantern dom [selector]`
- `lantern click <selector>`
- `lantern type <selector> <text>`
- `lantern console`
- `lantern network`
- `lantern screenshot`
- `lantern eval <js>`
- `lantern wait <condition>`

## First Milestone: CDP CLI Core

Build a Rust CLI that can connect to an operator-provided Chromium CDP endpoint and prove a small browser-inspection loop without becoming a full automation framework.

Acceptance criteria:

- initialize a Rust workspace and CLI crate
- accept a CDP endpoint via `--endpoint`, `LANTERN_CDP_ENDPOINT`, or config fallback
- implement `lantern doctor` to verify endpoint reachability and browser/version metadata
- implement `lantern targets` to list open targets with sanitized metadata
- implement `lantern page` for the currently selected page target, returning concise title/origin/url-shape/loading state without leaking full sensitive URLs by default
- support `--json` on implemented commands
- define redaction/truncation rules for URLs, text, console messages, and future DOM/network output
- include deterministic local HTTP/WebSocket fixture tests where practical
- document how to run Chromium in a headless Linux/container/VNC setup, but do not own container lifecycle in the first milestone

## Non-Goals For First Milestone

- no MCP server
- no Playwright dependency unless a later ExecPlan proves it is worth the cost
- no full DevTools replacement
- no provider-specific UI automation
- no screenshots as the primary feedback path
- no browser launch/container orchestration in the CLI
- no persistent daemon unless later evidence shows startup cost or session state requires it
- no credential, cookie, local storage, DOM dump, screenshot, or full URL persistence by default
- no cross-browser abstraction beyond Chromium CDP

## Future Capabilities To Preserve Seams For

- `dom`, `layout`, and accessibility-tree summaries optimized for agents
- console and network error summaries with source locations and truncation
- screenshot capture plus optional ASCII/layout summaries
- simple interaction commands: click, type, press, wait
- small JavaScript evaluation escape hatch with explicit safety warnings
- session selection and target pinning for multiple tabs
- optional daemon mode if repeated CLI startup becomes too slow
- Smoogle integration so frontend tasks can call Lantern from Codex child runs
- possible Pythia integration for browser-backed AI web UI workflows

## Operational Constraints

- Rust implementation
- Linux-only initially
- local-first and single-user
- headless-server friendly
- operator-managed Chromium/CDP endpoint
- container/VNC-compatible operating model
- strong validation through rustfmt, cargo check/test, clippy where useful
- minimal dependencies unless they clearly reduce protocol risk or implementation complexity
- no hidden network dependency beyond connecting to the explicit CDP endpoint

## Harness Context

This project exists partly to advance Smoogle dogfooding. It should use the standard Smoogle docs contract, small ExecPlans, task review, run summaries, and eventually `smoogle next`/queue execution. The first useful outcome is a cleanly bootstrapped repo with a narrow first milestone and tasks that are small enough for unattended Codex runs.
