# ExecPlan: Add pointer viewport interactions and screenshot regions

Status: completed

## Summary

Add first-class viewport interaction helpers for canvas and map-like UIs: `lantern hover`, `lantern wheel`, and `lantern drag` with `pointer-drag` as an alias. Extend `lantern screenshot` with explicit visible-viewport crop coordinates.

## Problem

Lantern currently covers ordinary app chrome with `click`, `type`, and `key`, but many frontend development loops depend on pointer movement, wheel zoom, and drag panning inside a canvas or viewer. Screenshot capture is also all-or-nothing, which makes it harder to inspect focused viewport regions without carrying extra image processing outside Lantern.

## Approach

Reuse the existing selected-page and selected-element interaction flow. Resolve the requested selector, compute the element center from `DOM.getBoxModel`, then dispatch bounded CDP input events:

- `hover`: one `Input.dispatchMouseEvent` `mouseMoved`.
- `wheel`: one `Input.dispatchMouseEvent` `mouseWheel` at the resolved point with operator-supplied deltas.
- `drag` / `pointer-drag`: `mouseMoved`, `mousePressed`, interpolated `mouseMoved` steps from start to end over the requested duration, then `mouseReleased`.

For screenshots, keep PNG output path semantics unchanged and pass an optional `clip` to `Page.captureScreenshot` when `--region-x`, `--region-y`, `--region-width`, and `--region-height` are supplied.

## Non-Goals

- Do not add arbitrary JavaScript evaluation or a multi-step gesture runner.
- Do not infer canvases or targets when no selector is supplied.
- Do not add touch gestures, pinch zoom, modifier keys, or non-left-button drag variants.
- Do not write screenshot bytes to stdout or create screenshot histories.

## Validation

- Add focused WebSocket fixture tests for hover, wheel, drag, alias parsing, screenshot region capture, and validation failures.
- Run `cargo fmt`.
- Run relevant Rust tests, then the repo validation script if focused tests pass.

## Progress

- Created the active ExecPlan and scoped the implementation to existing interaction and screenshot command surfaces.
- Implemented `hover`, `wheel`, `drag`, and the `pointer-drag` alias through the existing selector polling and page-target selection flow.
- Added optional screenshot regions through explicit `--region-*` flags with `--crop-*` aliases.
- Added fixture coverage for pointer CDP events, screenshot clipping, alias parsing, and validation failures.
- Updated CLI contract, operator docs, scorecards, and skill guidance for the expanded interaction and screenshot surface.
