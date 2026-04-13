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
- document headless Chromium setup

## Non-Goals

- multi-user sync
- cloud hosting
- auth
- MCP server in the first milestone
- full DevTools replacement
- browser launch or container orchestration in the CLI
- cross-browser abstraction beyond Chromium CDP

## Acceptance Direction

- deliver one end-to-end `cli` workflow that proves the core product loop
- keep the first release local-first and single-user
- prefer durable local data through `sqlite`

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

- Should SQLite be initialized in the first milestone or deferred until session and task state need durable storage?
- How should Lantern choose the current page target when multiple page targets are open?
- What exact URL-shape format is safe and useful enough for agent feedback?
- Which local fixture approach should be used for deterministic CDP HTTP and WebSocket tests?
- Should config fallback read from a standard file path in v1, and if so what path?
