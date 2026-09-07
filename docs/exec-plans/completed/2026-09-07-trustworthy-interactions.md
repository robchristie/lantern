# Trustworthy interactions

Status: completed

Implementation and local qualification are complete. Focus-delivery calibration
selected real tab activation; final independent review, CI qualification and
landing evidence belong to the package pull request.

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

Calibration asked whether one bounded same-socket target/scroll/stability/hit-test
path gives truthful results on real Chromium. The smallest probe used a unique
button, an offscreen button and an occluded button. Package I and the browser
fixture own evidence. Actual hits, prevented hits and retained bounded diagnostics
agreed, completing calibration and selecting the candidate. The source baseline is
`e6171f136e2418eb1d52eb146fb8eb445862d1fe`; provisional iterations are not separate
landing candidates.

## Acceptance

- Real browser fixtures cover duplicates, disabled/native fieldset/ARIA controls,
  scrolling, occlusion, moving/delayed controls, focus/editability and global keys.
- Rust/fake-CDP checks preserve exact dispatch/error/secret-redaction contracts,
  shared deadlines, bounded cleanup and no automatic replay.
- Canonical standard validation and the browser runner passed on the committed
  implementation, with source, fixture and browser identities in the evidence.
- The package pull request owns final independent review and landing evidence.

## Qualification evidence and limits

Calibration passed on Chrome `151.0.7922.34`: unique and offscreen controls
received clicks and changed the independently inspected fixture title; the
occluded target received no input and retained `element_occluded` at expiry.
The same-socket target-handle mechanism was retained. Initial committed
implementation `a4cadb95b38d102e6ec54c60d94358e3ca5c7fae` passed 22 provisional
contracts at viewport `1000x557`, including disabled hover, read-only
and non-editable input, focus-listener state changes, global keys preserving
focus, and an empty padded control. Fixture SHA-256:
`bbb0c718bb5b77a507b9f050cb05105417e47590b2b24e537adbeafbe461cdeb`.

The runner records exact source/build identities and fixture/browser inputs in
ignored `.smoogle/browser-contracts/evidence.json`. The package pull request owns
the final reviewed candidate identity and qualification summary. Canonical
validation passed formatting, workspace/all-target checks, locked Rust 1.85
compatibility, all 206 tests and docs hygiene. Exact committed-head browser run
`f33d8c04-e8b8-41ea-a5b8-a6c055d4071e` passed in 12.272 seconds with matching
clean source/build identities.

Independent review blocked `82e33a1ed1b96c25d782f8b407ed85d8c3edd550`;
[PR #10 records the findings](https://github.com/robchristie/lantern/pull/10#issuecomment-5564474611).
The repair retains only actually observed blockers, distinguishes incomplete
sampling from instability, and keeps stalled probes without a blocker on the
runtime-failure exit path. Regression fixtures exercise both legacy and strict
modes with a stalled second sample and insufficient time to take it; a separate
regression retains an actual earlier blocker across a later stalled sample.
Repair Rust qualification passed all 208 tests. The repaired browser cohort
passed 22 cases on local Linux/Chrome `151.0.7922.34`; the loopback CDP audit
observed no `Input.*` commands for all 12 non-dispatched cases. Real positive
controls exercised mouse, text and key auditing. Three deliberately injected
audit records were rejected by the same negative assertion; these guard checks
do not inject forbidden input into the browser. Missing-browser configuration
and invalid executable probes replaced stale passes with fresh failed evidence.
Replacement fixture SHA-256:
`3867826ecbae67483f6bb774ce5c3d20b09d2e4fa947e97b60e2163733e5ca99`.

The CI browser job uses `macos-15-intel` and a job-local pinned browser to retain
the sandbox after the Ubuntu AppArmor startup failure. The package PR owns that
platform's CI result, replacement exact-head evidence and final review verdict.

Interaction responses do not verify application outcomes. Independent fixture
assertions qualify the tested outcomes, including successful input and prevented
input on blocked controls. Shadow-root/frame targeting, continuous-motion proof,
visual acceptance and hardware coverage remain outside this package. Q is partial
until F and final programme qualification.

The official actionability
reference is <https://playwright.dev/docs/actionability>, checked 7 September 2026.
Native disabled state includes fieldset semantics; hover can target disabled
controls. Existing font/zero-content probe notes are retained under ignored
`.smoogle/qualification-tools/interaction-probe-notes.md`.


## Focus-delivery calibration decision

[PR #10 retains the calibration evidence and decision](https://github.com/robchristie/lantern/pull/10#issuecomment-5564703564).
CI run `34079568084` acknowledged text input without observing the disabling
focus listener; that acknowledgement alone did not prove text delivery. The
bounded comparison used a fresh synthetic focus-disables fixture and fresh
browser for each baseline, `Page.bringToFront`, `DOM.focus` and diagnostic
focus-emulation arm. The package owned actual document focus, active element,
focus events and disabled state before and after each operation.

The selected checkpoint was `ef3a11abd3a51c1a3f7c992d60bf8409a54fd544`;
macOS CI `34080179198` tested merge reference
`0cd58eadf7a51e3de2d40b078f28fef924d9699d` against fixture SHA-256
`3867826ecbae67483f6bb774ce5c3d20b09d2e4fa947e97b60e2163733e5ca99`.
Both Linux and macOS Chrome `151.0.7922.34` showed the same mechanism: baseline
and `DOM.focus` changed the retained active element without document focus,
focus events or disabled state. `Page.bringToFront` restored real document focus
and delivered the disabling listener. Diagnostic emulation also worked but was
rejected as unnecessary. An earlier preactivated Linux probe was rejected as the
baseline comparison.

The repair activates the selected page before text/key preparation, within the
shared deadline, and requires actual `document.hasFocus()` as well as target
focus. Failed activation prevents input; unavailable document focus remains a
conservative blocker. No focus emulation is adopted. Fake-CDP tests cover failed
and stalled activation for both type and global keys. The provisional script
and emulation CI arms were removed after selecting the mechanism.

The durable browser suite now runs a focus-listener-disables text action as its
first mutation, before a pointer action can activate the fresh browser. Local
qualification passed all 23 cases, including zero input commands for that first
blocked action. Canonical validation passed formatting, workspace/all-target
checks, locked Rust 1.85 compatibility, all 209 tests and docs hygiene. The PR
owns the exact final candidate and macOS CI verdict;
these remain ordinary final gates rather than another calibration decision.
