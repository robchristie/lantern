# Observable and verifiable UI outcomes

Status: active

## Outcome and authority

Implement the user-supplied Lantern review dated 7 September 2026. Keep Lantern
a small browser-inspection and evidence tool with explicit browser ownership.
The user authorised implementation; repository standing authority covers ordinary
reviewed source, test, documentation and CI landing. Releases, deployment, live
credentials, destructive operations and publication remain separate boundaries.

Baseline Lantern revision: `46c77296767aace433521ee734d09f12c7b911b8` (also the
reviewed revision). Baseline Polyorama revision:
`d469d74a14f0bc4494fdfc44504f12462ee50841`. No open Lantern PRs at baseline.
The existing persistent-profile plan is unrelated and must be preserved.

## Capability envelope and baseline

| ID | Required outcome | Baseline evidence and gap | Candidate package | State |
| --- | --- | --- | --- | --- |
| T | Absolute transport/operation deadlines; bounded event counts and bytes; explicit lost evidence | `crates/lantern-core/src/cdp.rs`, `flow.rs`: socket timeouts and unbounded pending queue/drains | 1: bounded transport | Qualified candidate; reviewed landing pending |
| I | Unique, scrolled, enabled, stable, hit-tested interactions; diagnostic failures; compatible opt-in strict exit semantics | `interaction.rs` and CLI contract: dispatch does not establish outcome; timeouts can succeed | 2: trustworthy interactions | Pending |
| F | Observe before one action, await typed explicit postcondition, capture state and failures, return verdict without mutation replay | `flow.rs`: navigation observation only; separate interaction commands lose intervening events | 3: action/assertion/capture | Pending |
| V | Task-dependent visual review requiring actual image inspection and explicit visual expectations | Tracked inspection skill privileges text and screenshots as supporting evidence | 4: inspection workflow | Pending |
| P | Consume existing Polyorama versioned semantic/text/visual evidence; narrow on-demand adapter where live inspection needs it; explicit revision/coverage correlation | Polyorama `docs/ui-snapshots/README.md`, `docs/ui-guides/ui-review.md`; existing artefacts and browser accessibility limitation | 5: application evidence | Pending |
| A | Compact computed accessibility view and semantic role/name or test-ID targeting for ordinary DOM applications | DOM summary is not an accessibility snapshot; avoid invented snapshot-reference lifetimes | 6: semantic DOM inspection | Pending |
| L | Escaped, unique generated selectors; explicit container configuration; heuristic layout findings and intentional overflow distinction | `layout.rs`: raw identifiers and application-specific container classes | 4: inspection workflow | Pending |
| Q | Real-Chromium contracts for duplicates, disabled/occluded/offscreen/moving controls, delayed state, fast request/runtime failures and known canvas | `.github/workflows/check.yml`: Rust checks only; hardware qualification distinct | 2–3, 7: browser qualification | Pending |
| C | Discover package/build identity, commands and schemas; split CLI by command family before broader interface expansion | CLI version is package-only; large `main.rs` | 2: CLI foundation (separate package) | Pending |
| S | Short core skill, progressive references for lifecycle/auth/GPU/application recipes; evidence-centred project purpose | Tracked skill contains lengthy setup recipes; README leads with MCP overhead | 4: inspection workflow | Pending |
| B | Paired external CLI baseline on form, async failure, layout defect, canvas and restart/recovery | No comparative benchmark; source claims are not measured results | 7: comparative qualification | Pending |

## Boundaries and acceptance

- Preserve existing consumers with additive contracts and opt-in strict execution;
  separate command completion, dispatch and verified application outcome.
- Quiet intervals never substitute for an explicit application postcondition.
  Dispatched but uncertain mutations are inspected, never automatically replayed.
- Retain explicit browser lifecycle, endpoint choice, redaction defaults,
  screenshot persistence opt-in and the fact that screenshot pixels are unredacted.
- Reuse Polyorama's evidence machinery; do not build a parallel renderer, DOM
  mirror, high-frequency state synchronisation or internal-action input shortcut.
- Exclude general workflow languages, daemons, general multi-backend architecture,
  broad Playwright parity and new hardware/platform support claims, as directed
  by the review. Snapshot references are optional and require lifetime semantics
  if introduced. No generic JavaScript execution command is required.
- Browser contracts establish behaviour; pinned visual evidence establishes only
  its renderer's repeatability; hardware evidence names the actual graphics path.
- The comparison records exact tool/configuration/input identities and prioritises
  correctly verified completion, false passes and interventions before calls,
  elapsed time and available context measurements. Unavailable metrics remain
  explicit rather than inferred. Use official current tool documentation.

## Execution and evidence

The programme conductor owns selection and reconciliation. Fresh bounded executor
contexts own implementation packages and their independent reviewed landing loops.
Land this baseline before dispatch. Refine package boundaries against current
evidence; enabling owners land before consumers. Detailed probes, review findings,
CI and acceptance evidence belong to package plans, tests and PRs.

Enter calibration for uncertain live application adapters and external comparison:
state the question, minimal representative fixture, evidence owner and exit rule
before broad implementation. Select a coherent candidate before ordinary landing.
No baseline row is complete merely because a plan, harness or draft PR exists.

| Package | Owner revision / consumer revision | Aggregate result | Evidence | Status |
| --- | --- | --- | --- | --- |
| Baseline | Lantern `901e419579fbcc3f6ac65df8f0fcd2c7d9cced94` / Polyorama baseline above | Reviewed baseline landed; existing Polyorama evidence reusable | PR #7 | Landed |
| T: bounded transport | Candidate identified by package PR / no downstream consumers yet | Shared deadlines and bounded evidence; 195 tests and canonical checks passed | `../completed/2026-09-07-bounded-transport.md` | Independent review and landing pending |

Next: select and independently review the bounded transport candidate, then land
it before the separate CLI foundation package. Other capability rows remain pending.

## Terminal rule

Every row must be implemented and qualified, proved already satisfied/superseded,
or excluded by the authority established above. Reassess the exact final merged
head against this envelope, run aggregate representative integration, reconcile
support documentation and plans, and audit task branches/worktrees/remote heads.
Difficulty or unavailable evidence is not permission to mark a row complete.
