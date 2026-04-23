# Managed Browser CDP Containers

Status: active

## Goal

Add an explicit, opt-in Lantern-managed browser lifecycle MVP for disposable
Chromium/CDP containers. Existing inspection and interaction commands remain
endpoint-based and never auto-start a browser.

## Context

Lantern currently documents operator-owned Chromium/CDP endpoints. That works
for a single manual browser but is fragile for concurrent agents because a
shared fixed `127.0.0.1:9222` endpoint and shared profile can collide. The
reference runtime at `/nvme/development/pythia/ops/browser-cdp` provides a
Chrome-for-Testing container with Xvfb, fluxbox, x11vnc, noVNC, loopback CDP,
an internal CDP proxy, and health/readiness scripts.

## Scope

- Add `lantern browser start|list|status|endpoint|stop|prune`.
- Start disposable managed containers with unique instance ids, unique
  container names, isolated local profile directories, labels, and random
  host-loopback CDP ports.
- Store managed-instance records under
  `.smoogle/lantern/browser-instances/`.
- Keep CDP publication loopback-only by default.
- Provide concise human output and stable JSON output with resolved endpoints.
- Add deterministic tests for parsing, state behavior, and runtime command
  construction without requiring a real container daemon.
- Document build/start/list/stop/concurrent-agent/security workflows and a
  manual smoke path.

## Non-Goals

- No implicit browser startup from `doctor`, `page`, `flow`, or other CDP
  commands.
- No public `0.0.0.0` CDP binding by default.
- No shared default Chromium profile between managed instances.
- No authenticated persistent profile as the default.
- No broad browser automation, arbitrary JavaScript evaluation, or daemon mode.

## Implementation Notes

- Put registry and runtime command construction in `lantern-storage` so the CLI
  stays mostly orchestration and output formatting.
- Prefer a runtime adapter shape that can support podman and docker. The first
  pass may default to podman while keeping command construction explicit and
  testable.
- Use atomic writes for JSON state files and a simple local lock directory for
  state mutations.
- Reconcile record status from container labels/inspection where practical, but
  keep normal validation independent of a running runtime.

## Progress

- 2026-04-23: Created active ExecPlan before implementation.
- 2026-04-23: Added JSON-file registry and runtime command construction in
  `lantern-storage`, including podman/docker command shapes, loopback random
  port publishing, labels, and isolated profile mounts.
- 2026-04-23: Added `lantern browser start|list|status|endpoint|stop|prune`
  CLI path. Existing endpoint commands still bypass lifecycle logic.
- 2026-04-23: Added in-repo Chrome-for-Testing/Xvfb/fluxbox/x11vnc/noVNC CDP
  container scaffold under `ops/browser-cdp`.
- 2026-04-23: Updated CLI, setup, workflow, security, and reliability docs for
  explicit managed lifecycle behavior.

## Validation Plan

- `cargo test -p lantern-storage`
- focused CLI parser tests if useful
- `scripts/validate.sh fast` during iteration
- `scripts/validate.sh` before completion if feasible
- `smoogle docs check` after docs and plan edits

## Deferred Work

- Add automated real-runtime smoke coverage once the validation environment can
  reliably provide podman or docker and image build time is acceptable.
