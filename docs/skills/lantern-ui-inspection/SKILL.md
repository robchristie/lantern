---
name: lantern-ui-inspection
description: Inspect local web interfaces with Lantern and judge them from bounded structural, runtime, layout and opened-image evidence. Use for functional UI checks, responsive visual review, canvas inspection, or Lantern dogfooding against a local Chromium CDP endpoint.
---

# Lantern UI Inspection

## Purpose and ownership

Use Lantern to collect bounded evidence from a local Chromium page while
developing or reviewing a web interface. The task or product contract owns the
expected behaviour and appearance. Lantern owns observation and reports its
limits; the inspecting agent owns the final judgement.

A screenshot command only captures pixels. It is not a visual review until the
resulting image has been opened and judged against explicit expectations.
Persist screenshots only through an explicit output path. Pixels are unredacted
even when command metadata uses default redaction.

## Establish the inspection surface

Confirm the application is running and identify the browser owner before
navigating or interacting. Prefer an existing operator-owned local CDP endpoint.
Read [browser-sessions.md](references/browser-sessions.md) for managed,
container-reachable or authenticated sessions; [gpu-canvas.md](references/gpu-canvas.md)
for WebGL/WebGPU; and [smoogle-dashboard.md](references/smoogle-dashboard.md)
only for Smoogle.

```bash
command -v lantern
lantern capabilities --json
lantern doctor --endpoint "$ENDPOINT" --json
```

Use command help to confirm task-specific flags when revisions may differ. A
package version alone does not identify a local build.

For ordinary DOM controls, `accessibility` provides bounded computed roles and
names; use exact semantic targets as described in the functional-actions reference.

## Choose evidence for the task

- For a fresh navigation, use `flow --open` so console and network collection
  starts before the observed navigation.
- For page state or semantics, use `page`, `dom` and an explicit `wait`
  condition. Increase DOM depth and node limits only when the default summary
  omits a relevant component.
- For one click with an explicit postcondition, use `action-flow`. It observes
  the baseline, dispatch, condition, console, network and optional capture on
  one attachment.
- For other bounded interactions, use `click`, `type`, `key`, `hover`, `wheel`
  or `drag` with `--strict`, then collect evidence for the application
  postcondition.
- For layout risk, use `layout --container-selector <CSS>`. The selector
  defaults to `[data-layout-container]`; override it only with the page's real
  layout-container contract.
- For appearance, responsive layout or a canvas, capture the visible viewport
  with `screenshot`, then open the PNG with the environment's image viewer.

Use JSON output when evidence will support a finding or assertion. Human output
is suitable for a quick exploratory read.

Read [functional-actions.md](references/functional-actions.md) when the task
needs command recipes or the detailed `action-flow` flag and result contract.
`ok: true` means structured completion, not application success. Use `--strict`
for acknowledged standalone input and a passed action-flow verdict.

Never automatically replay input whose dispatch is uncertain or may have
partly executed. Inspect current state first. Preserve the distinction between
pre-input setup failure, not-dispatched input, uncertain dispatch, failed or
timed-out postcondition, observed runtime/network failure, incomplete evidence
and capture failure when reporting the result.

## Inspect visual, layout and canvas results

Before capturing pixels, state the component and state that should be visible.
For each relevant viewport, define expectations for component presence, state
clarity, information hierarchy, spacing, alignment, clipping, overlap, unwanted
overflow, and intentional scrolling, wrapping or ellipsis.

Use both desktop and narrow viewports for responsive work unless the task
defines a different set. Arrange each through the available browser or
application harness, record actual PNG dimensions (screenshot metadata reports
best-effort viewport dimensions), and collect both
`layout --container-selector <CSS> --json` and `screenshot --output <PNG>
--json` evidence.

Open every relevant PNG and inspect its pixels. Do not infer hierarchy,
spacing, alignment, state clarity or visual correctness from DOM or layout JSON.
Treat `layout.heuristic=true` findings as leads and confirm
`layout.container_selector` matches the intended contract. Use each finding's
`overflow_behaviour` (`scroll`, `ellipsis`, `clipped` or `visible`) to
distinguish the informational `intentional-horizontal-scroll` and
`intentional-text-ellipsis` cases from suspected defects, then verify material
findings in the opened image. A clean layout audit does not establish visual
quality.

For canvas work, also require application readiness and visibly useful,
nonblank pixels. Check the expected canvas content, overlays, controls and
loading/empty/error state as applicable, plus console and network evidence.
Read [gpu-canvas.md](references/gpu-canvas.md) before selecting a graphics mode
or interpreting GPU coverage. For Polyorama, consume its existing semantic/text
bundles or fixed live snapshot using [polyorama.md](references/polyorama.md).

## Report the result

Tie each conclusion to the task expectation and the evidence that supports it.
Report the executable build identity, target and actual viewport dimensions
when they affect reproducibility; name structured artefacts and opened image
paths; state observed behaviour and visual findings; and retain material
uncertainty such as collection gaps, evidence loss, truncation or an ambiguous
target.

Do not turn a bounded clean observation into a claim that no earlier or later
failure occurred. When Lantern itself lacks a small observation needed across
real UI tasks, record that repeated friction as potential Lantern product work
rather than broadening the current application task.
