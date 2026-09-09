# Reliability

This file records the repository's current reliability contract. Keep it focused on operational expectations, known weak points, and recovery guidance.

## Current Grade

Overall grade: **B-**

Last reviewed: **2026-09-07**

Rationale: Bounded deadlines, explicit evidence loss, guarded input and typed
postconditions are covered by canonical and 64 real-Chromium contracts. The
paired CLI case study verifies ordinary form, failure, layout, canvas and
owned-browser reset journeys while retaining an unattributed owner interruption,
agent observation gaps and a conductor-directed rerun. A source-correlated
Polyorama shell is repeatable on its declared software renderer. Broader hardware,
host/container coverage and app-wide readiness remain outside this qualification;
see `docs/qualification/comparative-ui.md`.

## Grade Scale

- `A`: routine unattended operation, deterministic recovery paths, strong validation, and low flake rate.
- `B`: reliable for normal local use with documented manual recovery and bounded known gaps.
- `C`: useful but still dependent on operator review while product behavior and validation mature.
- `D`: frequent manual intervention, unclear state transitions, or weak artifact capture.
- `F`: basic task/run state cannot be trusted.

Use `+` or `-` only when the repo is clearly between two grades.

## Observed action outcomes

`action-flow` retains acknowledged or uncertain dispatch through later assertion,
capture and observation failures. A pass requires an explicit false-to-true
postcondition, complete collection and no observed runtime/network failure; an
already matched baseline remains incomplete and strict mode rejects it before
input. Requested diagnostics reserve a quarter of the remaining post-input
budget (at most 500 ms), including when an assertion stalls or times out, on the
same attachment. Capture follows the assertion and
does not claim visual correctness or an atomic frame. Independent browser input
audits and fake-CDP deadline/loss regressions cover the no-replay contract.

## Layout and visual evidence

Layout output explicitly reports heuristic geometry and configured containers.
Generated selectors uniquely resolve only in the observed DOM; bounded path or
scan omissions set `truncated`. CSS scrolling and ellipses are intent signals,
not usability verdicts. A successful capture or zero layout findings cannot
establish appearance: visual tasks require opening images at relevant viewports
and judging the task's explicit expectations.

## Application evidence

Polyorama evidence validates bounded owner data without implying application
acceptance. Text failures/exclusions, unavailable revisions/readiness and differing
frames remain explicit. Captured pixels survive an unavailable trailing snapshot;
equal snapshot counters do not establish atomic pixel correlation. Owner graphics
qualification remains separate from synthetic adapter/browser contracts.

## Operational Expectations

- Keep runtime state under `.smoogle/` and out of Git.
- Store managed browser records under `.smoogle/lantern/browser-instances/` and treat them as reconstructable local runtime state.
- Store named persistent profiles under the operator state home, never under a
  source checkout; treat their Chromium data as durable sensitive state rather
  than reconstructable runtime metadata.
- Permit at most one starting or running managed browser to own a persistent
  profile. A stale stopped or missing attachment may be recovered explicitly
  during the next start.
- Create an initial Git commit before task worktrees are used.
- Treat checked-in docs and prompt templates as the system of record.
- Prefer local-first workflows and deterministic local validation.
- Keep recovery guidance in checked-in docs, task notes, or run artifacts instead of chat history.

## Validation Budget

- Docs-only changes: inspect the diff and run `smoogle docs check`.
- Tight Rust edit loops: run `scripts/validate.sh fast`, using `FAST_TEST_ARGS` when a focused test filter is known.
- Standard Rust validation: run `scripts/validate.sh`, which composes formatting, `cargo check`, workspace tests, and docs hygiene.
- Periodic quality sweeps: run `scripts/quality-sweep.sh`; set `SMOOGLE_COVERAGE=1` when coverage evidence is worth the runtime cost.
- Prompt/template changes: verify template paths and inspect rendered prompt behavior where practical.
- Workflow changes: include a command-level smoke path or test when possible.

## Recovery Guidance

1. Inspect `smoogle run show <run-id>` before editing prompts or code after a failed run.
2. Use task notes for durable context that should travel with a task.
3. Prefer fixing repo-local guidance, checks, templates, or code over repeating one-off operator instructions.
4. Put real but deferred reliability work in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.
5. Use `lantern browser profile status <NAME>` before recovering a persistent
   profile. Delete it only after stopping its browser and deciding whether a
   service-side logout is also required.

## Bounded Transport

- Bounded commands share an absolute deadline across target selection, attachment,
  collection and finalisation. Slow partial traffic and continuous events cannot
  renew the deadline. See `docs/workflows.md` for limits and cleanup semantics.
- Resource-limited collection reports additive evidence-loss metadata and cannot
  claim an observed-clean window. HTTP and WebSocket size limits can reject very
  large target lists, DOM results or screenshots.
- Transport uncertainty after sending a command does not establish whether an
  input executed. Inspect state before any further action; never replay mutations
  automatically after a missing acknowledgement.

## Interaction Reliability

- All six interaction commands require unique selectors and action-specific
  preparation before input. Geometry and hit testing use the same target object;
  the latest blocker survives timeout. `--strict` opts into failed interaction
  exit status; application outcomes remain explicitly unverified.
- Local installation delivery requires a clean qualified merged build and
  provenance plus behaviour checks against the installed executable; see
  `docs/workflows.md` for the closeout sequence.
- Focus-time state changes are diagnosed before resulting blur, so disabling a
  text/key target reports `element_disabled` on the first preparation sample.
- Text/key preparation first activates the selected page and requires actual
  document focus; a retained active element alone cannot establish focus-event
  delivery. Failed activation prevents input without focus emulation.
- Click, key and drag failures attempt one bounded release without input replay.
  Partial or unacknowledged input is reported as uncertain, with no second JSON
  envelope and no raw remote error or entered text.
- `scripts/test-browser-contracts.py` and its CI job exercise real Chromium with
  fixture postconditions. See `docs/testing/browser-contracts.md` for inputs and
  limits. Sampling cannot rule out changes between preparation and input, and
  ordinary main-document CSS targeting does not pierce frames or shadow roots.

## Known Reliability Gaps

- Feature-specific fixture tests cover the implemented CLI loop, click/type/key/pointer interactions, screenshot regions, and flow observation path, and two-instance Podman and Docker browser smokes have passed, but real-browser smoke coverage is still limited.
- Recovery and landing behavior have not yet been proven by repeated assisted or unattended runs.
- Headless Linux, container, and VNC-compatible Chromium setup is documented in `docs/headless-chromium.md`, but the broader host/runtime/image matrix has not been smoke-tested across representative setups yet.
- Managed browser lifecycle commands have deterministic parser, registry, and runtime-command tests, but normal validation does not build the image or require a real podman/docker daemon.
- Hardware WebGPU is proven only for the repository Chrome image, rootless
  Podman, Xvfb, NVIDIA CDI, and one RTX host; Docker, DRM/Intel/AMD, Wayland,
  multiple devices, and broader driver versions remain manual coverage.
- WebGPU SwiftShader can create a Dawn device in the current image but loses it
  during swap-chain SharedImage creation, so it is not a supported visual gate.
- Optional quality-sweep tools may not be installed on every machine.
- CDP implementation complexity could grow beyond the intended small command vocabulary if future tasks skip the CLI contract and ExecPlan gates.
- Stable JSON contracts require compatibility discipline as command output evolves.
- Minimal dependency choices could increase protocol maintenance burden.

## Maintenance Rules

Update this file when reliability expectations, validation budgets, recovery paths, landing confidence, or known flaky surfaces change. Do not use it as a run log.

## Computed DOM semantics

Computed role/name and exact test-ID targets share the existing interaction
budget and no-replay engine. Candidate replacement and new ambiguity are checked
around actionability, and missing browser methods fail explicitly. The compact
accessibility view reports truncation and omits child-document/shadow content;
it is not a complete accessibility audit. Real-Chromium contracts cover these
boundaries as well as labelled typing, key dispatch and observed semantic clicks.
