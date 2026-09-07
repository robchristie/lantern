# Observed action, assertion and capture

Status: completed

Programme package F and its Q cases.

## Outcome and contract

Add one bounded `action-flow` click with selector presence, unique-selector text
substring, or exact URL postcondition and optional viewport PNG. Subscribe before
preparation, retain one socket through input, polling, capture and finalisation,
and retain dispatch and all available evidence after later failures. No replay,
generic evaluation command, quietness success rule or browser lifecycle change.

A passing result requires acknowledged input, a condition false before input and
true afterwards, complete evidence and no captured runtime/network failure. This
establishes observed order, not causation. An already matched condition is explicit
and incomplete even if it matches after input. Capture is sequenced after assertion;
it does not establish an atomic frame or visual correctness. Pixels are unredacted,
persistence is opt-in, and ordinary target/redaction policies apply.

## Phases and evidence

- [x] Map existing interaction, bounded CDP and collectors on base
  `c5c8f82b1f15a56a3ced080e4c809ad87eea3feb`.
- [x] Select additive core/CLI boundary and conservative pre-existing contract.
- [x] Implement retention, deadline, uncertainty and redaction regression tests.
- [x] Extend real-browser suite with saved transition, fast HTTP/runtime failure,
  timeout without replay, pre-existing condition, blocked input, failed requested
  capture, and known canvas transition/capture; retain independent input audit.
- [x] Canonical verification and real-browser mechanism qualification; final exact
  candidate identities belong to the package PR and ignored evidence records.
- Conductor independent review, PR/CI/landing and cleanup are the remaining
  delivery gates; this completed plan records the implementation candidate.

Core, CLI and plan integration belong to the package executor. Browser harness,
fixture and browser-contract documentation belong to one bounded helper. The
programme conductor alone owns independent review and GitHub delivery.

## Verification and limits

Canonical validation covers formatting, workspace/all-target checks, locked Rust
1.85 compatibility, all 217 Rust tests and docs hygiene. Six fake-CDP regressions
exercise setup/baseline bounds, failure/loss retention, pre-existing conditions,
blocked input, uncertain acknowledgement without replay and later assertion,
finalisation/capture/persistence failure. A CLI fixture verifies the absolute
budget starts before HTTP target selection. CLI validation tests reject unsupported
expectation/capture combinations before endpoint access.

The real-browser suite contains 31 cases (23 interactions plus eight action-flow
contracts), with independent input auditing. Provisional probe evidence is retained
under `.smoogle/validation/action-flow-probe-2/`; source/build were dirty on the
stated base. The conductor actually inspected its provisional 1000×557 PNG:
Render button, Canvas ready and a 64×64 red-left/green-right canvas. This is a
mechanism observation; exact final source/build browser evidence and any final
image inspection are recorded with delivery. It is not a pixel repeatability,
atomic frame, causal outcome or hardware-renderer claim.

The candidate is identified by its package PR. Run `scripts/validate.sh` and
`scripts/test-browser-contracts.py` from its clean committed source/build; retain
local results under `.smoogle/validation/action-flow-final/` and publish immutable
CI evidence with the PR. The conductor owns final independent review and landing.
Q remains partial until programme final qualification.
