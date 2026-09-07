# Paired UI qualification

Status: comparison selected; aggregate qualification follows below.

## Selected paired results

The three tool trajectories produced the following bounded results. Finding the
intentional layout defect is successful task completion; the UI itself fails
its readability expectation. Correct failure diagnosis likewise does not mean
the save succeeded. The fresh contexts all used `gpt-5.6-terra`, medium reasoning,
with equivalent outcome instructions. There were no agent false-success claims.

| Tool / attempt | Correctly verified tasks | False-success claims | CLI invocations / nonzero exits | CLI window seconds | Summed CLI execution seconds |
| --- | --- | --- | --- | --- | --- |
| Lantern | 5/5 | 0 | 36 / 1 | 222.438 | 9.123 |
| Playwright CLI 0.1.19 | 5/5 | 0 | 47 / 0 | 300.608 | 16.456 |
| agent-browser 0.36.0, interrupted | 3 verified; canvas incomplete; recovery unexecuted | 0 | 36 / 6 | 100.888 | 13.274 |
| agent-browser 0.36.0, fresh run after lifetime hardening | 3/5; form and recovery unverified | 0 | 45 / 3 | 239.765 | 27.987 |

**The agent-browser trajectory required one conductor-directed owner-lifetime
hardening and fresh run after an unattributed interruption.** Each agent run
needed zero unplanned in-run assistance; those zeros do not erase that overall
intervention. The common prescribed owner restart in task 5 is separate from an
unplanned intervention. Both agent-browser attempts remain in the evidence.

Lantern's form expected text omitted `contact:`, producing a timeout, followed
by an explicit read-only acknowledgement check. Its failure action selected a
condition already true before the click and correctly returned incomplete.
The next flow recovered a source-attributed runtime exception and an HTTP 503
**console message**; it did not retain the direct Network response. The agent
used the actual failure status and attributed observations without replay.
Playwright retained explicit acknowledgement, direct HTTP 503 and runtime error,
opened layout/canvas images and verified the fresh-profile recovery semantics.

In agent-browser's fresh run, the owner independently recorded successful form
and both recovery saves, trusted intended text, single submissions, draft
persistence across reload, and reset after restart. The agent used interactive-
only snapshots that omitted status output and a mismatched acknowledgement
string, so it did not verify form or recovery. Its inference that the draft
failed to persist because the input was empty is a **false negative**: the saved
status and owner truth retained `Morgan Draft`. This describes that agent's
observation choices, not a claim that agent-browser cannot inspect status.
Its failure diagnosis used HTTP 503; its empty console missed the independently
observed runtime exception and was not proof of no error. Its canvas click was
trusted and inside the canvas, changing blue/revision 0/frame 1 to green/revision
1/frame 2. The screenshot-derived point was about 17 CSS pixels above centre;
that capsule precision deviation is retained, while the required physical canvas
interaction and verified rendered outcome passed.

Every required layout and before/after canvas PNG in the completed visual tasks
was opened by its agent and independently opened during adjudication. All saved
comparison images decoded at 1280 × 900; all fixture viewport observations were
1280 × 900 at DPR 1. The canvas owner observed blue RGBA `[32,95,200,255]` and
green `[22,128,60,255]`, two committed frames and trusted physical pointer events.
The owner could distinguish successful application changes from agent uncertainty.

One representative run (plus the disclosed interruption/replacement) cannot
establish statistical superiority. Invocation counts and elapsed windows depend
on chosen observations and command mistakes; they are not model context or total
agent runtime. Actual model context/token and orchestration tool-call metrics
were unavailable and remain null. The fixed software-rendered fixture does not
establish hardware or broad website coverage.

The measured Lantern executable is from
`32a32be21a2e2d8311153925a0c84ea7cd341076`; its SHA-256 is
`ebc280537a9cb0d1d62b0a1fe24260ba9a74cb71b8672ad1caa58a19767cdbe9`.
A read/execute-only retained copy is
`.smoogle/comparison/lantern-measured-32a32be21a2e2d8311153925a0c84ea7cd341076`.
Subsequent qualification builds at `target/debug/lantern` are separate artefacts.
The public [evidence manifest](comparative-ui-evidence.json) retains exact input
hashes, equivalent capsules, original agent reports, independent summaries,
per-run owner identities, screenshot hashes, and raw-record hashes. Detailed
local records remain under `.smoogle/comparison/`. The owner's later diagnostic
and detached-session delta is documented above and in its per-run identity.

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

## Interrupted agent-browser run and lifetime probe

The first full agent-browser run verified form, failed-save diagnosis and layout,
then ended with canvas incomplete and recovery unexecuted. Its unsupported
`mouse click` command dispatched no pointer. The following wait lost CDP; both
the Chromium endpoint and the separate owner endpoint were gone. The owner
process completed with exit 0, without an exception or recorded stop request.
No evaluator close, restart, process signal or shell cleanup was found. This is
an **unattributed owner/browser interruption**, not a demonstrated CLI failure.
Its report and all invocation metrics are retained separately.

A bounded reproduction used the same canvas navigation, observation, unsupported
command, wait and later reads. It returned the expected command error and
25-second wait timeout, with both owner and browser still healthy. The loss was
not reproduced. Owner instrumentation now records received signals and browser
and owner exit status; signal cleanup also removes the owned profiles. The
replacement run launches that owner in a detached process session to separate
its lifetime from the command runner. A disposable readiness probe proved fresh
about:blank state, 1280 × 900 CSS and actual PNG dimensions, new PID/profile on
restart, and clean recorded SIGTERM teardown of both browser and profiles.

One fresh five-task agent-browser run uses the same model, outcome capsule and
application/tool/browser materials without command hints. The owner diagnostic
and lifetime-isolation delta is explicit: owner implementations are not claimed
byte-identical across all runs. Earlier completed Lantern/Playwright runs retain
their successful lifecycle evidence. Local evidence owners are
`.smoogle/comparison/eval-agent-browser/`, `lifetime-probe/`, and
`detached-lifetime-probe/` beneath the same comparison directory.
