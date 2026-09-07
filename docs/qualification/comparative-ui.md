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

## Calibration observations

The first Lantern agent, using `gpt-5.6-terra` at medium reasoning and executable
built from `9783775a62d7b2ddf53712707523a0ad53f62c52`, correctly verified the
submitted name and diagnosed the single failed submission. Independent fixture
truth found one trusted form submission, one saved value, one failed submission,
HTTP 503 and the matching runtime error. The agent retained final DOM evidence
and opened both images; no false pass or external intervention occurred.

Its wrong initial action target and mismatched expected status text were agent
choices, retained in the record. The resulting action-flow deadline left no
capture budget; it explicitly returned incomplete, and the agent obtained a
separate screenshot without replaying either submission. This is not a passing
action-flow result. Fixture telemetry POST requests also appeared as aborted
background requests; they are distinct from the intentional failing save route.

That run is rejected for viewport qualification: CSS geometry was 1280 × 900 but
actual PNGs were 1280 × 757. Setting the visible surface only before navigation
also failed. The retained correction keeps an owner CDP session and reapplies
`Emulation.setVisibleSize` after navigation. A separate readiness capture decoded
and opened at 1280 × 900. Semantic and lifecycle observations from the first run
remain valid; final fresh agents use the corrected common setup. These probes
are calibration, not the measured paired results.

Local evidence: `.smoogle/comparison/cal-lantern-v1/` and
`.smoogle/comparison/viewport-probe-v2/readiness.png`. The original browser PID
survived all per-command attachments, and the owner then stopped it and removed
its disposable profile. No global lifecycle action was used.

For each measured run, `scripts/adjudicate-comparison.py RUN_DIR` summarises
trusted events, acknowledged values, failure observations, canvas pixels and
restart identities separately from the agent's assertions. The final adjudicator
must compare the report to those observations and open the relevant PNGs. A
fixture truth pass alone cannot become a correctly verified agent completion.
The elapsed CLI window is the earliest recorded start through the latest finish;
the sum of CLI execution durations is reported separately. Reading skills before
the first command and writing the final report are outside that window.

Playwright's fresh calibration independently confirmed one trusted submission per
case, acknowledged saved state, HTTP 503 and the matching runtime error. Both
opened PNGs decoded at 1280 × 900; named detach preserved the owned browser PID.
Its trace also retained the original fixture's private telemetry POST payloads,
although the agent did not open or rely on those payloads. To prevent ordinary
network evidence exposing independent adjudication counters, the final fixture
uses an owner-installed reporting-only CDP binding. It supplies no action API,
creates no network request and is not exposed through normal console output.
All measured agents receive the same corrected fixture; the two earlier
calibration runs are not comparative measurements.

Agent-browser's calibration likewise acknowledged the saved form and correctly
diagnosed failure from page and HTTP evidence, while explicitly reporting an
empty runtime-console observation. The independent owner saw the exception, so
the agent's runtime coverage was incomplete; an empty console was not treated as
proof of no error. Both opened PNGs were 1280 × 900 and named close preserved the
owned browser PID. Its ordinary `fill` helper emitted an untrusted preparatory
input event followed by trusted `insertText` and a trusted submit. This sequence
is retained, not rejected: normal CLI preparation is permitted, whereas
agent-injected DOM values and application actions are not. The final fixture
also records input data and resulting value to identify the text carried by
trusted events. A separate keyboard readiness probe demonstrated fully trusted
input, but does not replace or alter the real agent result or constrain its
normal CLI choices.

Calibration selects the common private reporting binding, retained visible-
surface alignment and owned-process lifecycle. The five-task comparison uses
fresh agents, browsers and profiles; calibration timings are excluded.
