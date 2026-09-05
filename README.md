# Lantern

Rust-first local CLI shim over Chromium CDP for agentic frontend development.

## Purpose

Agents need concise browser feedback during frontend work, but broad DevTools integrations add too much tool context, compaction pressure, and setup brittleness.

## Initial Shape

- primary user: A technically fluent local operator using Codex or Smoogle on headless Linux.
- primary interface: cli
- storage model: local-first; disposable managed-browser state lives under
  untracked `.smoogle/`, while explicitly named persistent browser profiles
  live under the operator's private Lantern state home.
- current milestone: Keep endpoint-based browser inspection stable and provide
  explicit opt-in managed Chromium/CDP lifecycles for disposable agents and
  dedicated authenticated profiles.
- future interfaces reserved: tui, web ui

## Repo Guide

- `AGENTS.md`: minimal table of contents for humans and agents
- `PLANS.md`: ExecPlan policy and active plan links
- `ARCHITECTURE.md`: top-level architecture entrypoint
- `QUALITY_SCORE.md`, `RELIABILITY.md`, `SECURITY.md`: current scorecards and safety contracts
- `smoogle.toml`: checked-in harness defaults and operation profiles
- `docs/architecture.md`: detailed architecture
- `docs/headless-chromium.md`: operator-owned Chromium, container, and VNC-compatible CDP setup
- `docs/authenticated-browser-testing.md`: safety checklist and workflow for dedicated logged-in browser profiles
- `docs/skills/lantern-ui-inspection/`: tracked Codex skill for dogfooding Lantern during UI work
- `docs/product-specs/lantern.md`: generated product spec
- `docs/product-specs/cli-contract.md`: first-milestone CLI contract
- `docs/product-specs/output-policy.md`: shared output, JSON ordering, redaction, and truncation policy
- `docs/exec-plans/completed/2026-04-14-frontend-feedback-loop-v1.md`: completed frontend feedback loop ExecPlan

## Quick Start

1. Read the generated docs contract before changing code.
2. Review the seeded `.smoogle/` tasks.
3. Tune `smoogle.toml` before long unattended runs.
4. Start an operator-owned Chromium CDP endpoint using
   `docs/headless-chromium.md`, explicitly start a disposable managed instance
   with `lantern browser start`, or create a dedicated authenticated profile
   with `lantern browser profile create <NAME>` and start it using
   `lantern browser start --profile <NAME>`.
   For a trusted hardware-WebGPU application, use `--graphics webgpu` with an
   explicit `--gpu-device`; Lantern never selects a host device implicitly.
5. Use `lantern browser endpoint <ID>` to retrieve the endpoint for a managed browser, then pass it to existing commands with `--endpoint` or `LANTERN_CDP_ENDPOINT`.
6. Use `lantern flow --open <URL> --timeout-ms <MS> --quiet-ms <MS>` when one coherent navigation/wait/console/network observation is more reliable than separate snapshot commands.
7. Use `lantern hover`, `lantern wheel`, and `lantern drag` for explicit canvas or viewport pointer checks after reviewing the target and selector.
8. Use `scripts/validate.sh fast` for tight loops and `scripts/validate.sh` before landing code changes.
9. Build new browser-inspection capability in bounded steps rather than broad scaffolding.
