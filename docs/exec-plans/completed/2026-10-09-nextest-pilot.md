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

## Qualified implementation

The representative probe and full local qualification passed at
`f09b6b797f7a37343252cbcfc8b1e75a2867cf8d`. The resulting implementation
retains the reporting improvement:

- Cargo and Nextest discovered the same 241 named tests across six binaries,
  including matching package, target, test kind and ignored status (none ignored).
- Canonical validation passed, including eleven command-level regressions,
  Rust 1.99 checks/tests, Rust 1.85 checks, doctests and comparison adjudication.
  The complete Rust 1.85 test entry point also passed all 241 tests and doctests.
  Both toolchains currently collect zero authored doctests; the explicit gates
  now cover future documentation examples.
- The representative CLI, socket and nested-build cases passed on Rust 1.85.
  A deliberately failing synthetic test produced parseable JUnit with failure
  details and durations. ShellCheck and actionlint passed.
- On one local warm pair, Cargo took 3.007 seconds and Nextest took 1.845 seconds.
  Both reported approximately 0.22 seconds of incremental build work. Separate
  initial build preparation took 4.331 seconds. These observations use Rust
  1.99.0, Nextest 0.9.146, the same lockfile/target directory and native runner
  concurrency defaults; they do not establish clean-build or hosted speedups.

Detailed logs, inventories and timings are retained by the operational owner
and referenced in the pull request. Independent review, GitHub-hosted Rust and
Chromium checks, merge and post-merge checks remain the normal delivery gates;
the pull request owns their exact revisions and terminal evidence. No change to
other portfolio projects is implied by this completed implementation pilot.
