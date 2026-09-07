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

The suite covers unique and duplicate selectors, native, disabled-fieldset and
ARIA disabled controls, disabled pointer hover, read-only and non-editable text
targets, focus-listener state changes, a non-focusable key target, an occluded
target, an offscreen target that requires scrolling, a padded control with no
text content, a continuously moving target, delayed rendering, input focus and
key delivery, and representative hover, wheel and drag behaviour. It checks the
opt-in strict exit status alongside the additive interaction diagnostics and
also keeps one legacy non-strict failure case. It deliberately does not make a
top-level interaction `ok` assertion.

The fixture also includes two dormant hooks for the later action-and-capture
package: `case=post-action-failure-hook` produces a fast failed request and a
runtime exception after a click, while `case=known-canvas-hook` draws known red
and green canvas regions. This suite does not claim those future flow or visual
contracts.

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
