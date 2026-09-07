# PLANS.md

This repository uses ExecPlans for substantial work.

## When To Use An ExecPlan

Create or update an ExecPlan before starting work that spans multiple sessions, crosses major modules, changes architecture or storage, or is risky enough that a later agent should understand it first.

## Locations

- Active plans: `docs/exec-plans/active/`
- Completed plans: `docs/exec-plans/completed/`
- Tech debt tracker: `docs/exec-plans/tech-debt-tracker.md`

## Quality And Hygiene

- Keep `QUALITY_SCORE.md`, `RELIABILITY.md`, and `SECURITY.md` current when changes affect quality, validation, reliability, threat model, or automation boundaries.
- Put non-blocking findings in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.
- Run `smoogle docs check` after changing scorecards, ExecPlan links, prompt templates, or core docs structure. In Codex child runs, use the injected `smoogle` shim or `"$SMOOGLE_BIN" docs check` fallback.

## Current Active Plans

- `docs/exec-plans/active/2026-09-07-observable-ui-programme.md`
- `docs/exec-plans/active/2026-09-07-trustworthy-interactions.md`

- `docs/exec-plans/active/2026-08-16-persistent-authenticated-browser-profiles.md`

## Recently Completed Plans

- `docs/exec-plans/completed/2026-09-07-cli-foundation.md`

- `docs/exec-plans/completed/2026-09-07-bounded-transport.md`

- `docs/exec-plans/completed/2026-09-05-recover-pointer-graphics.md`
- `docs/exec-plans/completed/2026-08-20-secret-safe-form-entry.md`
- `docs/exec-plans/completed/2026-07-27-managed-browser-hardware-webgpu.md`
- `docs/exec-plans/completed/2026-06-28-managed-browser-webgl-graphics-mode.md`
- `docs/exec-plans/completed/2026-04-29-key-press-interaction.md`
- `docs/exec-plans/completed/2026-05-28-pointer-wheel-screenshot-region.md`
- `docs/exec-plans/completed/2026-04-23-managed-browser-cdp-containers.md`
- `docs/exec-plans/completed/2026-04-15-session-observation-flow.md`
- `docs/exec-plans/completed/2026-04-14-frontend-feedback-loop-v1.md`
