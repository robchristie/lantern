# Browser contracts

The browser contract suite qualifies Lantern interactions against a real,
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

Lantern performs every click, text, key and pointer interaction. A successful
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
