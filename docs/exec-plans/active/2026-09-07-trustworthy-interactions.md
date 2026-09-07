# Trustworthy interactions

Status: active

Current phase: locally qualified candidate; independent review pending.

## Outcome and ownership

Programme I adds strict unique targeting, scrolled and stable usable geometry,
action-specific enabled/editable/focus checks, hit testing and bounded failure
reasons to the existing six interaction commands. `--strict` makes unsuccessful
interaction results exit 1 while retaining legacy result exit behaviour. Input
acknowledgement never claims an application outcome. No mutation replay, general
script command, action flow or broad Playwright parity is included.

The executor owns core/CLI and this plan; the browser-contract helper owns the
fixture, runner, CI job and testing document. The programme conductor owns exact
head review, publication and landing.

## Calibration

Question: can one bounded same-socket target/scroll/stability/hit-test path give
truthful results on real Chromium? The smallest probe is a unique button, an
offscreen button and an occluded button. Package I and the browser fixture own
evidence. Exit calibration when actual hits, prevented hits and retained bounded
diagnostics agree, then qualify the complete candidate. The source baseline is
`e6171f136e2418eb1d52eb146fb8eb445862d1fe`; provisional iterations are not separate
landing candidates.

## Acceptance

- Real browser fixtures cover duplicates, disabled/native fieldset/ARIA controls,
  scrolling, occlusion, moving/delayed controls, focus/editability and global keys.
- Rust/fake-CDP checks preserve exact dispatch/error/secret-redaction contracts,
  shared deadlines, bounded cleanup and no automatic replay.
- Canonical standard validation plus the browser runner pass on the committed
  candidate; record source, fixture and browser identities with results.
- The conductor independently reviews and lands the exact candidate.

## Evidence and next action

Calibration passed on Chrome `151.0.7922.34`: unique and offscreen controls
received clicks and changed the independently inspected fixture title; the
occluded target received no input and retained `element_occluded` at expiry.
Retain the same-socket target-handle mechanism. The expanded working-tree suite
passed all 22 cases at viewport `1000x557`, including disabled hover, read-only
and non-editable input, focus-listener state changes, global keys preserving
focus, and an empty padded control. Fixture SHA-256:
`bbb0c718bb5b77a507b9f050cb05105417e47590b2b24e537adbeafbe461cdeb`.

The runner records exact source/build identities and fixture/browser inputs in
ignored `.smoogle/browser-contracts/evidence.json`; copy the final candidate
identity and summary to the package PR. Canonical Rust verification passed:
formatting, workspace/all-target checks,
locked Rust 1.85 compatibility and all 206 tests. The docs checker passed. The exact committed-head browser
rerun precedes independent review. No application outcome,
shadow-root/frame targeting, continuous-motion proof, visual acceptance or
hardware coverage is claimed by this package. Q is partial until F and final
programme qualification.

The official actionability
reference is <https://playwright.dev/docs/actionability>, checked 7 September 2026.
Native disabled state includes fieldset semantics; hover can target disabled
controls. Existing font/zero-content probe notes are retained under ignored
`.smoogle/qualification-tools/interaction-probe-notes.md`.
