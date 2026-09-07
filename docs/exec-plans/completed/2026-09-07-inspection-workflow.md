# Task-dependent inspection and heuristic layout

Status: completed

Current phase: coherent implementation candidate selected; conductor review and landing remain.

## Outcome and scope

Programme V/L/S: make visual review require inspected pixels and explicit
expectations; provide escaped, uniquely resolving layout selectors and explicit
container configuration; shorten the skill through selective references. Preserve
additive CLI compatibility, bounded output, metadata redaction and opt-in capture.
The unrelated persistent-profile plan and installed skill remain untouched.

## Selected contract

`layout --container-selector <CSS>` replaces app-specific assumptions, defaulting
to `[data-layout-container]`. Report geometry as heuristic evidence, distinguish
CSS scroll/ellipsis intent from suspected defects, and never infer visual quality
from a clean audit. Generated selectors must uniquely identify their observed
node; if bounded construction cannot prove that, omit the finding and mark the
audit truncated. Configuration is length-bounded and browser syntax-validated.

## Acceptance and evidence

- Real Chromium exercises punctuation identifiers, duplicate paths, configured
  versus ordinary app classes, intentional scrolling/ellipsis, defects and bounds.
- Capture the layout fixture at desktop and narrow viewports; the conductor opens
  both PNGs and judges hierarchy, spacing, alignment, clipping and state clarity.
- Run `scripts/validate.sh`, skill `quick_validate.py`, then browser contracts from
  a clean committed source and matching build; retain exact identities in ignored
  qualification evidence and send them to the conductor.
- The conductor owns independent review, PR, CI, landing and installed-skill
  reconciliation. No release, deployment or hardware qualification is included.

## Progress

| Phase | State | Evidence / next action |
| --- | --- | --- |
| Contract and ownership | Selected | This plan; programme V/L/S |
| Core layout and browser contracts | Implemented; 38-case probe passed | `crates/lantern-core/src/layout.rs`, browser suite |
| Progressive skill and consumers | Implemented; skill validation passed | Short core and four selective references |
| Candidate verification | Candidate selected | 218 Rust tests passed; clean committed rebuild/browser evidence and actual image review supplied to the conductor |

## Qualification boundary

Browser probes establish all 38 interaction/action/layout contracts. The viewport
probe initially caught reset emulation after CDP detach; the retained emulation
attachment now measures and preserves 1000×800 and 390×844 through navigation and
capture. These findings are owned by `scripts/test-browser-contracts.py` and its
`layout.html` fixture; the harness itself leaves visual judgement pending.

Canonical verification log: `.smoogle/inspection-workflow-validate.log`. Final
clean-source browser evidence: `.smoogle/inspection-workflow-qualified/evidence.json`
with source/build identity, fixture digest, browser identity, input audits,
measured viewports and PNG digests. The conductor opens the final PNGs, independently
reviews the exact candidate, and owns CI, landing and programme reconciliation.
This completed package plan records the implementation candidate, not a claim
that programme V/L/S has landed.

Installed skill reconciliation is deliberately deferred to the conductor after
landing. Compare the installed skill to its saved pre-change identity before
replacing reviewed files and adding the four reference files; preserve unrelated
local files or new edits. Useful existing local binary, Geometis host/profile,
software WebGL, hardware WebGPU and cleanup recipes are preserved in the tracked
references. Validate the installed directory after reconciliation.

## Bounded capture calibration

Question: do the requested layout viewport, measured browser viewport and PNG
compositor dimensions agree? Probe only the existing fixture at 1000×800 and
390×844. The browser contract harness owns the detailed evidence. Exit when both
measured viewport and decoded PNG dimensions match and actual opened images agree
in one run, followed by clean-source qualification.

Rejected capture qualification: source `3252f6b56a41dbf37f83f0b104d70b5d819579ca`,
fixture SHA-256 `99e6328574d8861af8fbacfb3d7a1e9fd81205678093301936da7a0fcecaf7fd`,
run `b8c249ec-43ae-4d37-bf97-67c4b61281f0`. All 38 functional contracts passed,
but actual image inspection found the desktop PNG at 1000×557 despite a measured
1000×800 viewport; the narrow image matched 390×844. Retained evidence and images
live in `.smoogle/inspection-workflow-rejected-3252f6b/`. This is not accepted
visual qualification. Determine whether compositor settling resolves the mismatch
before changing screenshot core behaviour; the selected hypothesis is harness
setup timing, not an established diagnosis.

Calibration selected: Chrome's device metrics are visible globally, but the
separate capture session retained the original physical surface. The diagnostic
same-session capture was 1000×800 while Lantern's separate-session capture was
1000×557; explicit visible-surface sizing after navigation aligned both. Resizing
before navigation was rejected because navigation restored the original surface.
No screenshot-core change is selected: its existing width/height contract is
best-effort viewport metadata. The CLI contract and skill now distinguish actual
PNG dimensions from that metadata.

The selected harness retains emulation, navigates, sets visible size, independently
measures viewport and PNG dimensions, and requires agreement. Aligned probe run
`c7767ac7-fb04-4d60-b5a8-8038ccfb178c` passed all 38 contracts and both dimension checks; opened images showed
the intended desktop two-column and narrow one-column fixture, intentional
scroll/ellipsis, hard clipping and container escapes. Harness SHA-256 at this
probe: `9bb4e3053f84d4c149ff354fd253571a7e0b47fe4a0c11ce1eb55461e7f6b104`. Evidence lives in
`.smoogle/inspection-workflow-aligned-probe/evidence.json`; clean committed
qualification follows in the final evidence location above. The rejected
checkpoint remains in candidate history for recoverability.
