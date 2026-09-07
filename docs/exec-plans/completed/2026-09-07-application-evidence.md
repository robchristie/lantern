# Application evidence adapter

Status: completed

Current phase: implementation and local qualification complete; conductor review and landing pending

## Outcome and boundaries

Programme row P consumes Polyorama-owned semantic, text and pixel evidence through
an additive CLI adapter outside general CDP services. The programme conductor owns
publication, independent review, CI and landing. Polyorama is a read-only owner;
protected data and profiles are excluded. Existing physical input remains the
interaction boundary. No internal action dispatch, renderer or state mirror.

## Calibration

Question: which current owner artefacts and hooks expose enough information for
an honest bounded offline/live observation? Probe the application-shell gallery
baseline and fixed snapshot hook at Polyorama
`3c1c51f873de70162629a7fc4c17896f89bf7125`. This note owns calibration evidence.
Exit when a narrow supported contract and actual qualification route are selected.

Observed: version 1 `metadata.json`, `semantic.json`, `text.json` and `visual.png`
are current owner artefacts. Canonical semantic evidence deliberately removes
frame; metadata has no source revision. Text coverage counts submitted components
and excludes ordinary labels/native text. The live gallery snapshot retains frame
and nested `ui_snapshot`; the owner capture configures stories internally as
fixture setup, which does not establish physical interaction.

## Acceptance and next action

- Bound file/response sizes and collection counts; reject invalid version, geometry,
  coverage, malformed/truncated JSON and PNG.
- Report unavailable source, application revision, readiness and screenshot frame
  explicitly. Preserve differing frame observations instead of claiming simultaneity.
- Consume actual current owner artefacts and a representative live snapshot with
  associated pixels from a clean committed Lantern build.
- Run canonical `scripts/validate.sh` and browser contracts; hand exact candidate,
  artefact identities and PNG paths to conductor for independent review.

Selected: `polyorama --evidence-dir <DIR>` consumes the version 1 bundle;
`polyorama --endpoint <URL> --timeout-ms <MS> [--output <PNG>]` calls only the
fixed gallery snapshot hook. All application logic lives in the CLI adapter.
Source/application revision and runtime completion remain explicitly unavailable.
Capture brackets record same/differing observed frames and unknown pixel frame.

Focused probes: actual application-shell-dark baseline consumed (11 semantic
nodes, six measured text components, 26 native controls, zero observed native
controls). Eight malformed/coverage/geometry/PNG/correlation/redaction tests pass.
A headless Chrome 151 probe returned valid frame 11 but black pixels with a
WebGPU shared-image backing error: reject for visual qualification. The owner's
private Xvfb/bwrap route with Chromium sandbox retained produced opened nonblank
application-shell pixels. This is software WebGPU evidence only. No source,
baseline, protected material or profile was modified in Polyorama.

Canonical validation passes: format, workspace checks, Rust 1.85 locked check,
226 tests and documentation hygiene. The browser suite passes 44 cases including
six synthetic adapter cases; input audit proves the adapter dispatches no input.
Owner-standard `cargo xtask build-web` completed on clean source, with its log and
WASM/JS/browser/Lantern hashes retained in the qualification record.

Qualification artefacts live under `.smoogle/qualification-tools/polyorama/`:
`identity.json`, `owner-build.log`, `live-shell.json`, `live-shell.png`,
`offline-shell.json` and `validate.log`. Browser evidence is
`.smoogle/p-browser-contracts/evidence.json`. Final source/build identity is in
those records and the package PR, avoiding a self-referential committed hash.

Visual expectation: dark gallery chrome, selected reference/application-shell,
two dock panes with deterministic text, and intentionally truncated tab labels.
The accepted live route yielded 1440×900 pixels, matching the owner fixture,
with equal application/semantic/bracketing counters; pixel frame remains unknown.
A synthetic trailing-hook exception confirms that the initial semantic frame and
captured PNG/hash survive an unavailable second observation.
The image is newly captured live evidence, separate from the illustrative baseline.

Clean committed-source qualification passes; repeatable final artefacts identify
the review candidate through `capabilities.build.commit` and `dirty=false`.
The newly captured image hash matches the illustrative owner baseline; this is
repeatability evidence for the pinned software renderer, not design approval or
hardware coverage. The adapter itself still reports source revision unavailable.

Next: conductor independently reviews the exact candidate and final PNG, publishes
and lands the package when review/CI gates pass, then reconciles programme row P.

## CI transport boundary calibration

CI `34086692985` on `b39b01e2c3aeab61b92887414ffeafbe79effc77`
retained the expected runtime exception and HTTP 500 but reported `incomplete`:
one complete 617-byte event was dropped at a polling slice boundary. Evidence:
`.smoogle/qualification-tools/polyorama/ci-34086692985/browser-contract-evidence/evidence.json`.
Question: can a fully parsed event cross the short polling slice without losing
usable evidence while the absolute operation budget remains? The smallest probe
is a deterministic transport completion test with an expired socket slice and
separate live/expired operation budgets. The transport regression owns mechanism
evidence; this package owns the CI disposition. Exit requires retained delivery
inside the operation budget, unchanged loss accounting after actual expiry,
canonical verification and the unchanged 44-case browser suite.

Selected repair: the poll slice bounds further socket work; completed events use
only the absolute operation deadline for delivery. This creates no new queue or
read iteration and preserves absolute deadline drops, byte accounting, existing
partial-frame loss semantics and bounded irrelevant-response handling.

Calibration regression and canonical verification pass: formatting, workspace and
locked Rust 1.85 checks, all 227 tests and documentation hygiene. The exact repair
candidate's clean-build browser results and final calibration acceptance are
retained with the package PR and ignored qualification artefacts.
