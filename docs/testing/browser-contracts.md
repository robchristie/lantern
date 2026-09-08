# Browser contracts

The browser contract suite qualifies Lantern interactions and heuristic layout against a real,
explicitly selected Chromium binary. It is separate from ordinary Rust tests:
Rust tests do not install or start a browser.

Build Lantern, set `LANTERN_CHROMIUM` to an executable Chromium or Chrome binary,
then run:

```sh
cargo build --locked --workspace
LANTERN_CHROMIUM=/path/to/chrome \
  scripts/test-browser-contracts.py \
  --lantern target/debug/lantern \
  --output-dir .smoogle/browser-contracts
```

The runner owns the whole local test environment. It starts a loopback fixture
server and one headless browser with a fresh temporary profile and an ephemeral,
loopback-only CDP port. Every command and browser startup has a bound. Cleanup
stops Chromium, closes the server and removes the profile even after failure.
The runner never adds `--no-sandbox` and never connects to a daily browser
profile.

Lantern performs every CLI click, text, key and pointer interaction. Four
isolated page-side preparation probes additionally execute the actual
`actionability.js` once against the focus-disabling fixture; their evidence
explicitly distinguishes these samples from CLI dispatch observations. A successful
dispatch is not accepted as the application postcondition: the fixture changes
its title from event handlers, and a separate read-only `lantern page` call
checks the resulting state. Negative fixtures also install document-level event
capture and, where focus matters, a separate editable sentinel. A loopback CDP
proxy audits commands from Lantern to Chromium and requires no `Input.*` command
for every non-dispatched result. Positive controls prove that the same proxy
detects mouse, text and key commands, while deliberately injected audit records
exercise the same assertion and prove that it fails closed. The runner invokes
each mutation once and never automatically retries a possibly dispatched action.
For `action-flow`, an acknowledged click must produce exactly the three expected
mouse dispatch commands; a blocked action must produce none.

The suite covers unique and duplicate selectors, native, disabled-fieldset and
ARIA disabled controls, disabled pointer hover, read-only and non-editable text
targets, focus-listener state changes, a non-focusable key target, an occluded
target, an offscreen target that requires scrolling, a padded control with no
text content, a continuously moving target, delayed rendering, input focus and
key delivery, and representative hover, wheel and drag behaviour. It checks the
opt-in strict exit status alongside the additive interaction diagnostics and
also keeps one legacy non-strict failure case. It deliberately does not make a
top-level interaction `ok` assertion.

The first mutation in a fresh browser is a text attempt against an input whose
focus listener disables it. This runs before any click or other interaction can
activate the initial page and requires `element_disabled`, no dispatch, no
outgoing `Input.*` method and unchanged fixture state. The later matrix retains
the same focus-listener case after other interactions as a separate regression.

The suite extends the 23 interaction cases with action-flow contracts. Each
action flow starts console and network collection before one click, records the
pre-action condition state, awaits one explicit condition and returns the
interaction, postcondition, observation, capture and verdict in one structured
result. The cases cover all supported condition forms:

- selector and text substring matching after an asynchronous save;
- selector presence that becomes true only after an HTTP 500 response is
  observable, alongside the click handler's runtime exception;
- an exact URL transition;
- an acknowledged assertion timeout with collection-deadline evidence and one
  audited click;
- a condition that was already true before the action and therefore cannot
  produce a passing verdict;
- a disabled action with no outgoing input;
- screenshot persistence failure after acknowledged dispatch; and
- a canvas transition followed by a persisted PNG capture.

The HTTP failure marker is created only after the fixture receives the 500
response, preventing a matched condition from racing ahead of the network
evidence. The capture-write failure uses a missing parent directory so path
persistence fails after the action without relying on platform-specific device
files. The canvas fixture draws fixed red and green halves only after the click;
the runner verifies the capture summary and PNG signature. These checks establish
the capture sequence and persisted file contract, not pixel repeatability or a
visual judgement.

Every action-flow command supplies exactly one of `--expect-selector`,
`--expect-selector` with `--expect-text`, or `--expect-url`. Capture remains
opt-in through `--output`, with `--overwrite` used for the repeatable successful
case. `--strict` exits 1 for `failed` and `incomplete`, while `ok=true` continues
to mean that Lantern completed and returned the structured observation. The
runner checks `matched_before_action`, `matched`, `timed_out`, the final observed
value, console and network collection gaps, capture status and error, and the
top-level verdict and error.

An assertion exhausting the shared budget remains `incomplete` with
`postcondition.timed_out=true`, retained observations and collection-deadline
flags, without a fabricated observation error. Deterministic CDP tests cover
expiry before a command and while a probe is awaiting its response, and preserve
protocol, evaluation and transport failures as errors even when their text
mentions a deadline. A received WebSocket close remains an observation failure
when the peer delays TCP teardown beyond the operation budget. A missing input
acknowledgement remains uncertain.

The audit proxy intentionally observes frames without delaying, dropping or
rewriting CDP traffic. Real browser cases therefore cover acknowledged and
blocked dispatch plus a genuine collection deadline. Deterministic protocol
tests own lost acknowledgements and other injected transport uncertainty; adding
those faults to this proxy would make the same run's independent input audit
ambiguous.

Each run first replaces any previous verdict with a fresh failing record, then
writes `evidence.json` and the Chromium log under the selected output directory.
This happens before checking `LANTERN_CHROMIUM` or either executable path, so a
preflight failure cannot leave a previous pass as the apparent current result.
When an interaction returns an unexpected exit status, the runner still retains
its actual JSON output and exit status, audits the outgoing input methods and
performs one independent read-only page-title observation before failing the
same contract. It does not replay the interaction.
Evidence names the run UUID and UTC bounds, source revision and dirty state,
fixture SHA-256, Lantern build capabilities, actual browser product and protocol
version, requested window and actual fixture viewport, per-case command and
verdict, and elapsed time. The browser contracts establish functional behaviour
only. Screenshot repeatability, visual review and selected hardware graphics
qualification require separate, pinned evidence.

CI runs these contracts on GitHub's supported `macos-15-intel` hosted runner and
installs the browser selected by Playwright 1.62.1 into a job-local directory.
The earlier Ubuntu runner could not initialise Chrome's usable sandbox under its
AppArmor user-namespace policy. Moving the functional contract to macOS retains
the browser sandbox without weakening host policy or passing an unsafe browser
flag. Local Linux and CI macOS runs are separate platform observations; neither
establishes visual equivalence or hardware graphics qualification.

The layout fixture adds seven contracts for default and explicit containers,
quoted selector configuration, CSS punctuation and deep duplicate selectors,
intentional scroll/ellipsis versus clipped text, 40-finding truncation,
unprovable bounded paths, the 10,000-element scan limit, and invalid CSS. The
runner resolves each emitted selector independently through read-only CDP and
checks unique node identity, text and geometry; it also checks container identity.
App classes acquire container meaning only when explicitly configured. The
fixture's repeated rows share a punctuation ID below seven identical ancestors.

The runner captures `layout.html` at 1000×800 and 390×844 CSS pixels with device
scale factor 1. The owning CDP harness aligns emulated layout and visible
surface size and stays attached through capture; the runner independently checks
both measured viewport and decoded PNG dimensions. It records fixture and image
SHA-256 identities and viewport configuration. These captures remain marked as awaiting image inspection: a
reviewer must open them. At desktop width, expect two aligned columns, readable
headings and ready state, and consistent card gaps; at narrow width, expect one
column and ordinary vertical scrolling. The yellow scroll row must expose a
scrolling affordance, the abbreviation row an ellipsis, and the two suspected
clipping rows must visibly cut text without ellipses. Red strips intentionally
escape dashed containers. These are known defects to detect, not an example of
a visually passing application. The lower repeated-structure card is outside
the initial narrow viewport and is covered by desktop capture and DOM contracts.
This fixture does not establish application-specific design or hardware quality.

The Polyorama adapter cases use a labelled synthetic owner hook to qualify stable
and differing frame counters, missing/oversized hooks, malformed geometry,
retained screenshot hashes, preservation of captured pixels after a trailing
snapshot exception, and absence of input dispatch. They do not render
Polyorama or establish GPU coverage. Actual owner application-shell qualification
and its source/browser/render provenance belong to the application evidence
package plan and PR evidence.

Computed accessibility contracts cover implicit/explicit roles, associated labels
and aria-labelledby, exact punctuation-bearing test IDs, duplicate rejection
with zero audited input, delayed targets, disabled/occluded controls, focus-time
replacement and new ambiguity, semantic type/key/hover/action-flow, output bounds
and redaction, omitted input values, and excluded frame/shadow content. Protocol
fixtures separately prove backend IDs are resolved rather than joined to frontend
IDs, and an unsupported Accessibility method fails without input or fallback.

## Focus blocker qualification (8 September 2026)

The bounded investigation asks whether a synchronous focus listener can disable
and blur a target before the first actionability sample diagnoses the blocker.
The smallest probe runs the actual `actionability.js` once on the real
`focus-disables` fixture, records document focus, disabled state and active
element, and requires `element_disabled` without input. This document owns the
probe evidence. Exit requires a failing baseline, a passing corrected first
sample, unchanged CLI semantics and timeouts, canonical validation and the full
browser contracts; macOS CI remains a separate required platform observation.

The baseline script at `e564a356e9d5223e68c212e4a634da1fd3464592`
(SHA-256 `f93f22c0e57b1a1e302c7d6fbed89bfa9813ff2f5aa38468f307fa04cc7970f7`)
returned `element_not_focused` while the document was focused and the target was
already disabled and blurred. No input was sent. This reproduces the diagnostic
ordering defect independently of the slower semantic resolution path: a later
poll could repair the diagnostic, but must not be required to discover a state
already observed in the first sample. The macOS failures retained the same
incorrect blocker when the 700 ms operation budget expired:
[PR 16 post-merge run](https://github.com/robchristie/lantern/actions/runs/34121412513)
and [PR 17 post-merge run](https://github.com/robchristie/lantern/actions/runs/34123388184).

Retain the candidate that checks connectivity, enabled and editable state after
focus/scroll listeners and before diagnosing lost focus. It preserves the actual
focus requirement, semantic target checks, timeout and no-input contracts. The
four permanent regressions cover type/key and CSS/resolved-object preparation,
require the disabled-and-blurred observation and reject the baseline after one
sample, regardless of machine speed.

Local calibration evidence uses Chrome 151.0.7922.34 on Linux and fixture SHA-256
`f1b4d58871fe51f260a33ceb5fde4e8afbda8c0b17333b3c7c4135ded97c0727`:

- `.smoogle/focus-baseline/evidence.json`, run
  `cd2133d2-010a-4c48-b873-4a8519d4a227`: rejected first sample above. The harness
  used an existing clean `8f52adc06a5cbdabb758ef8adc79eea8fb6c53d3` binary to
  activate/navigate the page; the sampled script is the exact baseline identified
  above. A subsequent temporary-profile cleanup error does not change that
  retained probe result.
- `.smoogle/focus-candidate/evidence.json`, run
  `4a9ca51a-2e87-409c-bc79-68b275ffe301`: retained candidate script SHA-256
  `31940992efd96feda9ad1e935365b97c2fe3c6a3ff7c322dfbb007af539d745d`, built on
  the same base with these edits, passes all 77 cases, including the unchanged
  semantic focus-disabled CLI assertion. `scripts/validate.sh` passes 234 Rust
  tests, four adjudicator tests, formatting, workspace and Rust 1.85 checks and
  docs hygiene. macOS CI qualification remains required at delivery.
