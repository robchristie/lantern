# Paired UI qualification

Status: bounded calibration; no comparative result selected yet.

## Question and scope

Can Lantern and pinned Playwright CLI 0.1.19 and agent-browser 0.36.0 correctly
verify the same five representative UI tasks from fresh disposable states?
Correct outcome or failure diagnosis, false passes and external interventions
come before invocation counts and elapsed time. One run per tool is a bounded
case study, not statistical superiority.

Use fresh agents with the same model and reasoning and equivalent outcome
instructions. Agents may read their tool's supplied documentation and inspect the
UI, but cannot read fixture source, independent truth or competing results.
All evaluated browser actions use the assigned real CLI and physical browser
input. Browser-level CDP text insertion is permitted; DOM assignments, injected
application actions and alternate automation backends are not. Setup and
independent read-only adjudication may use Playwright.

## Reproduction and evidence ownership

- `scripts/fixtures/comparison/index.html` owns the five synthetic tasks: submitted
  contact form, fast HTTP/runtime failure, clipped destination label, pointer-
  driven canvas and browser restart with explicit disposable draft reset.
- `scripts/comparison-owner.mjs` launches only a fresh owned Chromium process,
  records executable/fixture/build identities, argv, PID, viewport and initial
  URL, and exposes a private loopback lifecycle endpoint. Restart closes only
  this process and creates a fresh profile. Fixture event logs and a separate
  Playwright console/network observer establish adjudication truth.
- `scripts/record-comparison.py --run-dir DIR -- REAL_CLI ARGS...` records each
  actual CLI invocation, UTC start/end, monotonic elapsed, exit and hashed raw
  outputs. It does not perform browser automation. CLI invocations are not
  orchestration tool calls; model context/token and orchestration metrics remain
  unavailable unless the agent runtime exposes actual measurements.
- Retain run state under `.smoogle/comparison/`. Start the owner with
  `node scripts/comparison-owner.mjs serve RUN_DIR`; `status`, `restart` and
  `stop` act only on that directory's recorded loopback owner. The owner requires
  `LANTERN_CHROMIUM` and the locally installed `playwright-core` dependency under
  `.smoogle/qualification-tools/`. Pinned local package lock and executable
  identities belong in the selected evidence manifest.

For this environment use Chromium 151.0.7922.34, 1280 × 900 CSS pixels, DPR 1,
headless SwiftShader and sandbox retained. Set `LD_LIBRARY_PATH` to the approved
Polyorama sysroot library directory and `FONTCONFIG_FILE` to the qualification
font configuration. No personal browser profiles or global session-close commands
are used. This is software-renderer evidence, not hardware support qualification.

## Calibration exit

The smallest probe is the submitted form plus fast failure for all three actual
agents, including CLI attach/detach and owner PID survival. Retain independently
observed successes and tool limitations. Revise mechanics only for equivalent
readiness or invalid measurement. Exit after identical starting conditions,
physical input and independent verdicts are established, then run the full five
cases with fresh agents and browser state. External tool failure is a valid
baseline result, not a reason to change the success criteria.
