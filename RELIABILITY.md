# Reliability

This file records the repository's current reliability contract. Keep it focused on operational expectations, known weak points, and recovery guidance.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-09-07**

Rationale: `Lantern` has local-first docs, a Rust workspace, prompt templates,
validation profile scripts, conservative harness defaults, fixture coverage for
the implemented frontend feedback loop, bounded click/type/key and pointer interaction
commands, screenshot region capture, session-oriented flow observation, and explicit managed browser
lifecycles for disposable containers and dedicated persistent profiles. The
grade remains `C` until broader real-browser/container smoke coverage, recovery
behaviour, and landing behaviour are proven across representative local setups.

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
already matched baseline remains incomplete. Capture follows the assertion and
does not claim visual correctness or an atomic frame. Independent browser input
audits and fake-CDP deadline/loss regressions cover the no-replay contract.

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
