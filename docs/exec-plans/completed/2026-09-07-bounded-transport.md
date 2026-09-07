# Bounded CDP transport and evidence

Status: completed

Programme package T; implemented, qualified and landed through PR #8.

## Outcome and boundary

Use one absolute budget from HTTP target selection through attachment, commands,
collection and finalisation for bounded CLI commands. Bound retained transport
and request metadata and drain work. Resource loss must prevent an observed-clean
claim. Preserve ordinary JSON fields and include additive loss metadata when
collection is incomplete. Input transport failure remains uncertain execution;
never replay an action. One best-effort pointer release may use a separate
100 ms cleanup budget.

Baseline: PR #7, `901e419579fbcc3f6ac65df8f0fcd2c7d9cced94`.
The package excludes actionability, new flow actions, CLI decomposition,
application adapters, layout and real-browser qualification infrastructure.
The programme coordinator owns independent review, CI, publication and landing.

## Implementation decisions

- Enforce WebSocket deadlines at underlying I/O and message-loop boundaries;
  socket inactivity timeouts alone cannot stop continuous frames or trickles.
- Resolve only local numeric addresses and localhost without DNS; disable HTTP
  redirects. Bound HTTP bodies to 4 MiB and WebSocket messages/frames to 16 MiB.
- Retain at most 1024 queued events and 4 MiB of their encoded payload. Drain
  batches yield after 1024 events or 20 ms. Collector event payloads are capped
  at 256 KiB; request correlation retains at most 1024 requests and 4 MiB.
- Keep historical collection gaps separate from resource loss. Loss metadata
  carries dropped counts/bytes, drain-limit and collection-deadline indicators.
  Quiet matching cannot establish success from an incomplete event collection.

## Acceptance and current evidence

Implemented adversarial fixtures cover continuous events without responses,
unrelated response floods, queued event byte/count limits, partial handshake and
frame trickles, HTTP header/body trickles and oversized bodies/frames, IPv6-only
localhost, short flow setup/finalisation, sustained flow events, HTTP selection
within the CLI budget, navigation uncertainty without replay, correlation limits
and uncertain drag release/duration. Partial event reads, read-ahead and
fragmentation are covered without retaining wire payloads. Existing CLI fixtures preserve ordinary JSON
contracts. Selector timeout fixtures now allow 100 ms for shared setup, replacing
a 1 ms assumption that excluded HTTP and attachment.

Final `scripts/validate.sh`: formatting, workspace/all-target checks, locked
Rust 1.85 checks, 195 nextest tests (195 passed, none skipped), and docs hygiene
passed. Focused CDP, flow and core test runs passed during implementation.
The existing ordinary CLI JSON fixtures still pass; additive evidence metadata
is omitted when complete. The committed candidate revision and subsequent
independent review/CI/landing evidence are owned by the package pull request.

## Independent review repair

The review of `c9ad4dc0c5dd778d45ceaeec07100269c797a18d` identified that
the provisional quiet verdict checked only socket loss, omitting collector-only
payload and request-correlation loss. The repaired verdict checks the final
console and network summaries after finalisation. Two fake-CDP regressions prove
that collector-only oversized payloads and correlation exhaustion clear the
match without transport queue overflow, while preserving completed-command
`ok` and timeout semantics. Focused flow tests passed (5/5), followed by the
canonical verification above. The [BLOCK record](https://github.com/robchristie/lantern/pull/8#issuecomment-5564049707)
owns the review finding; the repaired head passed independent re-review and landed.

## Landing evidence

PR #8 squash-merged as `0f4b87815e968b0bafea3d35b86c0273009d066d`.
Independent review passed for `53396b111a6e305b8efb10a9a7106494f168f935`;
reviewed and landed tree: `94b63d581b5b416956a14b27dfcfd91c6a50b44d`.
PR CI `34075754362` and post-merge CI `34075820982` passed alongside the
195-test canonical acceptance. Task branch, worktree, remote-tracking reference
and live remote head were cleaned. The [landing record](https://github.com/robchristie/lantern/pull/8#issuecomment-5564132394)
owns detailed reconciliation. The separate CLI foundation package is next.

Residual limits: no hard real-time scheduling guarantee; oversized results
(including screenshots over 16 MiB) fail; cleanup acknowledgement is best effort;
real-Chromium qualification infrastructure and trustworthy actionability remain
separate programme outcomes.
