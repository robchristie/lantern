# Lantern

Rust-first local CLI shim over Chromium CDP for agentic frontend development.

## Purpose

Agents need concise browser feedback during frontend work, but broad DevTools integrations add too much tool context, compaction pressure, and setup brittleness.

## Initial Shape

- primary user: A technically fluent local operator using Codex or Smoogle on headless Linux.
- primary interface: cli
- storage model: sqlite
- first milestone: Build a CLI core that connects to an operator-provided Chromium CDP endpoint and proves a small browser-inspection loop.
- future interfaces reserved: tui, web ui

## Repo Guide

- `AGENTS.md`: minimal table of contents for humans and agents
- `PLANS.md`: ExecPlan policy and active plan links
- `ARCHITECTURE.md`: top-level architecture entrypoint
- `QUALITY_SCORE.md`, `RELIABILITY.md`, `SECURITY.md`: current scorecards and safety contracts
- `smoogle.toml`: checked-in harness defaults and operation profiles
- `docs/architecture.md`: detailed architecture
- `docs/product-specs/lantern.md`: generated product spec
- `docs/exec-plans/active/2026-04-13-bootstrap-lantern.md`: current bootstrap plan

## Quick Start

1. Read the generated docs contract before changing code.
2. Review the seeded `.smoogle/` tasks.
3. Tune `smoogle.toml` before long unattended runs.
4. Use `scripts/validate.sh fast` for tight loops and `scripts/validate.sh` before landing code changes.
5. Build the first milestone in bounded steps rather than broad scaffolding.
