# Nextest verification pilot

## Outcome and scope

Use Nextest consistently for Lantern's ordinary Rust tests locally and in
GitHub-hosted CI, with per-test timing, slow-test warnings and retained JUnit
reports. Keep doctests on Cargo and preserve the Rust 1.85 compatibility gate.
Lantern owns this first-stage pilot; other portfolio projects are outside scope.

## Acceptance

- Standard validation requires Nextest and cannot silently select Cargo instead.
- Focused validation retains package/target/filter selection.
- Locked workspace unit, integration and example/benchmark test targets remain
  covered on Rust 1.99.0 and the existing Rust 1.85 CI surface.
- Cargo doctests run explicitly, separate from Nextest.
- CI retains separate reports for each toolchain, including failed test runs.
- Command-level regressions cover selection, missing tools and failure propagation.
- Existing formatting, workspace checks, comparison adjudication and real-browser
  CI qualification continue to pass.

## Probe and decision

Question: does Nextest preserve the existing Rust test inventory and improve
diagnostic visibility without a material unexplained execution regression?
Compare one warm Cargo run and one warm Nextest run on the same committed
candidate, toolchain, locked dependencies and host. Capture build preparation
separately, compare named test inventories and retain per-test results. This
small local comparison is descriptive, not a hosted-CI speedup guarantee.

The operational owner retains logs and timings outside the source checkout;
the reviewed pull request owns delivery evidence. Select the candidate when
coverage, reporting, canonical validation and CI are proved. Investigate any
missing tests, failures or material resource/scheduling regression before
landing. Retain the reporting improvement if execution is comparable; do not
claim that this runner reduces compilation or real-browser qualification time.

## State

- Baseline: `c894dc83ad1408108a92befa8dc9507502bf67b2`; successful main CI run
  [37871393389](https://github.com/robchristie/lantern/actions/runs/37871393389).
- Approach: shared local/CI test entry point implemented, with pinned pre-built
  Nextest, separate doctests, diagnostic slow warnings and failure reports.
- Early checkpoint: the three representative Rust 1.85 cases passed; a synthetic
  failed test produced JUnit failure details and durations. Eleven command-level
  regressions, ShellCheck and actionlint passed. These focused observations do
  not establish complete candidate qualification.
- Next: compare full inventories and warm execution, run canonical verification
  on the committed candidate, then independently review and land.
