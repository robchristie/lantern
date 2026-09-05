# ExecPlan: Recover pointer and graphics work onto current main

Status: active

Phase: graphics candidate verification

## Outcome and provenance

Recover the useful pointer/crop and graphics work from the preserved remote
`codex/archive-pointer-graphics-2026-09-05` at
`3a418f254f81789bd88db83460900d6af5b33db0` onto current `main`, retaining named
profiles, host-gateway routing and secret-safe file-backed input. Keep the
installed binary unchanged until final integration.

The archive is the source of implementation intent, not a qualified consumer
revision. Pointer source commits are `eec104b`, `4c395e8`, `bf17a45` and
`db72039`; graphics source commits `9792f2b` and `3a418f2` belong to the later
work package. Base main for pointer recovery: `2d55db7`.

## Checkpoints

| Increment | Owner revision | Consumer revision | Aggregate result | Status | Evidence |
| --- | --- | --- | --- | --- | --- |
| Pointer, wheel, crop and cleanup guidance | Archive pointer commits above | `75d5173` | Canonical validation passed 162 tests; real-browser smoke passed; independent review PASS at `df58d48` | Landed | Recovered historical plan: `../completed/2026-05-28-pointer-wheel-screenshot-region.md`; current PR and local smoke evidence |
| Graphics configuration and diagnostics | Archive graphics commits above | Pending reviewed graphics landing | Canonical validation passed 173 tests; hardware WebGPU, profile WebGL restart and pointer integration passed | Candidate | `../completed/2026-07-27-managed-browser-hardware-webgpu.md#recovery-qualification--2026-09-05` |
| Final integration and binary refresh | Both landed increments | Pending | Pending | Queued | Final canonical checks and installed-binary provenance |

## Acceptance and next action

Each deliverable requires canonical `scripts/validate.sh`, preservation of the
newer main contracts, an independent review of the exact candidate revision and
ordinary reviewed landing. Pointer additionally requires a local real Chromium
smoke of hover, wheel, duration-bound drag, and viewport crop including scroll.
`scripts/pointer-smoke.py` serves the bounded synthetic fixture and records
untracked results under `.smoogle/artifacts/pointer-smoke/`; the caller supplies
an explicit disposable target and owns managed-browser cleanup. It requires a
browser reachable endpoint and fixture server reachable from that browser.

Graphics acceptance keeps parsing/configuration fixtures separate from actual
hardware runtime proof. Record the exact candidate revision, browser image,
GPU/runtime identity and observed graphics result with the graphics evidence
owner before calling hardware acceleration proven. If runtime behaviour needs
calibration, define the smallest probe and exit condition there first.

Next: independently review and land the qualified graphics candidate, then
refresh the installed binary from merged main and complete recovery. Detailed
review and CI evidence belong to their pull requests.

## Pointer calibration evidence

Question: do viewport crops retain their origin after document scrolling?
The smallest probe uses a fixed blue surface and compares identical crop PNGs
before and after a synthetic 500 CSS-pixel scroll. Evidence owner: this pointer
work package and `scripts/pointer-smoke.py`. Exit condition: canonical checks
and a real Chromium run pass with unchanged crop pixels after scrolling.

The initial probe used recovered core revision `2508ea2`, with fixture and
runner content preserved in checkpoint `615fe5e`. Hover, wheel and duration-bound
drag passed, but the scrolled PNG differed. Reject the unchanged archive crop
implementation: it passed viewport-relative coordinates directly as CDP
page-relative clip coordinates. The repair adds the selected viewport's page
origin while retaining requested region metadata and viewport dimensions.
The fixture regression covers non-zero horizontal and vertical page offsets.
Runtime qualification passed on `1bb0e00e065a9f2b57be81ce422915bc21688a0d`
using Chrome for Testing 151.0.7922.47, image
`sha256:b0e3d80abba6a43e9a9790664d10109c70452dbc98a00859a0e2e36cbe3d7b3f`.
Hover, wheel, a 1200 ms drag, and identical 120 x 80 crop pixels before/after
scroll all passed. Untracked evidence: `.smoogle/artifacts/pointer-smoke/`.
The subsequent drag-error release repair passed canonical and runtime checks
on `df58d48501e0f419f4df7a7cbb29b503e23d48b9`, independently reviewed PASS
and landed in PR #1.


## Graphics recovery boundary

The recovered implementation unions graphics/device metadata with current profile
and gateway metadata and retains the current reservation and failed-start cleanup
lifecycle. `webgpu` with `--profile` fails validation before state or runtime
access; disabled, SwiftShader and generic GPU modes support named profiles.
The archived completed graphics plans below remain historical evidence, not
qualification of this recovered candidate.

`scripts/graphics-smoke.py` requires a caller-selected page and fixture URL for
`scripts/fixtures/graphics-smoke.html`. It requires rendered application status
and red/green screenshot pixels. For WebGPU, serve the fixture inside the browser
container at `http://localhost:8765/graphics-smoke.html` using a loopback-bound
Python HTTP server; do not expand insecure-origin browser flags. Runtime and
image qualification evidence remains owned by the graphics delivery work package.
