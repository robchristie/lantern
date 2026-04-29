# ExecPlan: Add keyboard key press interaction

Status: active

Consultation: `consult_20260429T103715Z_add-keyboard-key-press-primitive-using-c`

## Summary

Add a first keyboard interaction primitive, `lantern key`, for dispatching one keyDown/keyUp pair to a selected element through Chromium CDP.

## Problem

Lantern currently has `lantern type` for text insertion and `lantern click` for mouse interaction, but it has no semantic key press primitive. Keyboard-driven browser UIs such as games, command palettes, accessible widgets, terminal panes, and canvas/WebGL controls usually listen for keydown/keyup events rather than text insertion, so `Input.insertText` is the wrong primitive for those workflows.

## Recommended Approach

Implement a narrow `lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>` command beside the existing click/type interaction path. Reuse page target selection, endpoint handling, selector polling, DOM focus, timeout validation, redaction, and interaction output conventions. In `lantern-core`, add a single-key dispatch function that resolves and focuses the selected node, then sends `Input.dispatchKeyEvent` with `type=keyDown` followed by `type=keyUp`. Extend the interaction observed metadata with nullable key-specific fields so click/type remain compatible while key output can report the requested key and the dispatched event count. Update CLI parsing, usage validation, fixture tests, and product docs for the new command. Keep multi-key sequences, modifiers, held keys, chords, and game-loop helpers out of this slice.

## Non-Goals

- Do not add `lantern keys` sequence dispatch in the first slice.
- Do not add modifiers, shortcut chords, key hold duration, repeat events, or long-press behavior.
- Do not infer selectors, auto-target canvases, or add game-specific controls.
- Do not replace `lantern type`; text entry should continue to use `Input.insertText`.
- Do not add arbitrary JavaScript evaluation or a multi-step interaction/session runner.

## Risks

- CDP key event payloads need enough fields for real Chromium behavior; sending only `key` may work for fixture tests but fail in some browser/game handlers if code/key/text fields are missing or wrong.
- Extending the shared interaction observed schema can accidentally perturb click/type JSON consumers unless nullable additions are documented and tested.
- A too-permissive `--key` parser could produce confusing browser behavior; a too-strict parser could block useful keyboard workflows.

## Validation Strategy

- Add focused fixture tests for successful key dispatch and validation failures.
- Run a focused Rust test filter for key interaction during implementation.
- Run `scripts/validate.sh` before landing.
- If a local Chromium endpoint is available, manually smoke `lantern key --selector body --key ArrowUp --timeout-ms 1000 --json` against a simple page that records keydown/keyup counts.

## Open Questions

- The first implementation should use a conservative key allowlist or validator for common CDP key names and printable single-character keys, but the exact mapping details should be decided during implementation against Chromium CDP behavior.

## Slices

### Implement `lantern key`

<!-- smoogle-task: slice-key-command -->
Status: completed

Add a single-key selected-element interaction command using CDP `Input.dispatchKeyEvent`, with CLI validation, fixture coverage, and CLI contract documentation.

Progress:

- Implemented `lantern key --selector <CSS_SELECTOR> --key <KEY> --timeout-ms <MS>` through the existing selected-page interaction flow.
- Core now focuses the selected node and dispatches exactly one `Input.dispatchKeyEvent` `keyDown` and one `keyUp`, with metadata-only observed output for the requested key and event count.
- CLI validation covers missing selector, missing key, missing timeout, invalid timeout bounds, unsupported `--text` with `key`, and unsupported `--key` on other commands before target access.
- Fixture tests cover successful dispatch, selector timeout, JSON metadata, usage failures, and existing click/type compatibility.
- Product docs now include `key` in the command surface, target and timeout support, interaction JSON field ordering, redaction policy, stable error-code references, and examples.
