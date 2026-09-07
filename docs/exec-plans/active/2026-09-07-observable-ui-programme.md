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
| T | Absolute transport/operation deadlines; bounded event counts and bytes; explicit lost evidence | `crates/lantern-core/src/cdp.rs`, `flow.rs`: socket timeouts and unbounded pending queue/drains | 1: bounded transport | Implemented and qualified (PR #8) |
| I | Unique, scrolled, enabled, stable, hit-tested interactions; diagnostic failures; compatible opt-in strict exit semantics | `interaction.rs` and CLI contract: dispatch does not establish outcome; timeouts can succeed | 2: trustworthy interactions | Implemented and qualified (PR #10) |
| F | Observe before one action, await typed explicit postcondition, capture state and failures, return verdict without mutation replay | `flow.rs`: navigation observation only; separate interaction commands lose intervening events | 3: action/assertion/capture | Implemented and qualified (PR #11) |
| V | Task-dependent visual review requiring actual image inspection and explicit visual expectations | Tracked inspection skill privileges text and screenshots as supporting evidence | 4: inspection workflow | Implemented and qualified (PR #12) |
| P | Consume existing Polyorama versioned semantic/text/visual evidence; narrow on-demand adapter where live inspection needs it; explicit revision/coverage correlation | Polyorama `docs/ui-snapshots/README.md`, `docs/ui-guides/ui-review.md`; existing artefacts and browser accessibility limitation | 5: application evidence | Implemented and qualified (PR #13) |
| A | Compact computed accessibility view and semantic role/name or test-ID targeting for ordinary DOM applications | DOM summary is not an accessibility snapshot; avoid invented snapshot-reference lifetimes | 6: semantic DOM inspection | Implemented and qualified (PR #14) |
| L | Escaped, unique generated selectors; explicit container configuration; heuristic layout findings and intentional overflow distinction | `layout.rs`: raw identifiers and application-specific container classes | 4: inspection workflow | Implemented and qualified (PR #12) |
| Q | Real-Chromium contracts for duplicates, disabled/occluded/offscreen/moving controls, delayed state, fast request/runtime failures and known canvas | `.github/workflows/check.yml`: Rust checks only; hardware qualification distinct | 2–3, 7: browser qualification | Partial: interaction and action-flow fixtures qualified; final qualification pending |
| C | Discover package/build identity, commands and schemas; split CLI by command family before broader interface expansion | CLI version is package-only; large `main.rs` | 2: CLI foundation (separate package) | Implemented and qualified (PR #9) |
| S | Short core skill, progressive references for lifecycle/auth/GPU/application recipes; evidence-centred project purpose | Tracked skill contains lengthy setup recipes; README leads with MCP overhead | 4: inspection workflow | Implemented and qualified (PR #12) |
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
contexts own implementation packages; the conductor dispatches independent
review and owns publication, CI, landing and cleanup.
The baseline has landed. Refine package boundaries against current
evidence; enabling owners land before consumers. Detailed probes, review findings,
CI and acceptance evidence belong to package plans, tests and PRs.

Enter calibration for uncertain live application adapters and external comparison:
state the question, minimal representative fixture, evidence owner and exit rule
before broad implementation. Select a coherent candidate before ordinary landing.
No baseline row is complete merely because a plan, harness or draft PR exists.

| Package | Owner revision / consumer revision | Aggregate result | Evidence | Status |
| --- | --- | --- | --- | --- |
| Baseline | Lantern `901e419579fbcc3f6ac65df8f0fcd2c7d9cced94` / Polyorama baseline above | Reviewed baseline landed; existing Polyorama evidence reusable | PR #7 | Landed |
| T: bounded transport | Lantern `0f4b87815e968b0bafea3d35b86c0273009d066d` / no downstream consumers yet | Shared deadlines and bounded evidence; 195 canonical tests and PR/post-merge CI passed | [PR #8 landing evidence](https://github.com/robchristie/lantern/pull/8#issuecomment-5564132394); `../completed/2026-09-07-bounded-transport.md` | Landed; task Git state cleaned |
| C: CLI foundation | Lantern `e6171f136e2418eb1d52eb146fb8eb445862d1fe` / I uses this base | Command-family ownership and discovery; 201 tests and PR/post-merge CI passed | [PR #9 landing evidence](https://github.com/robchristie/lantern/pull/9#issuecomment-5564283462); `../completed/2026-09-07-cli-foundation.md` | Landed; task Git state cleaned |
| I: trustworthy interactions | Lantern `c5c8f82b1f15a56a3ced080e4c809ad87eea3feb` / F uses this base | Unique, scrolled, stable hit-tested input; strict exits; 23 real-Chromium contracts with CDP input audit and a fresh-browser focus regression | [PR #10 landing evidence](https://github.com/robchristie/lantern/pull/10#issuecomment-5564795233); `../completed/2026-09-07-trustworthy-interactions.md` | Landed; 209 tests, 23 Linux/macOS browser cases, PR/post-merge CI passed; task Git state cleaned |
| F: action/assertion/capture | Lantern `b1527dd47e832e3dfe1bb1fee5b4d90119920491` / V/L/S uses this base | Explicit false-to-true condition, single observed click, retained failure/capture result; 217 tests and 31 Linux/macOS browser contracts | [PR #11 landing evidence](https://github.com/robchristie/lantern/pull/11#issuecomment-5565022068); `../completed/2026-09-07-action-assertion-flow.md` | Landed; PR/post-merge CI passed; task Git state cleaned |
| V/L/S: inspection workflow | Lantern `8b5bf0b822c05935ff656ff1a627cf241899b200` / P uses this base | Task-dependent evidence, heuristic layout, progressive skill; 218 tests and 38 Linux/macOS browser contracts | [PR #12 landing evidence](https://github.com/robchristie/lantern/pull/12#issuecomment-5565237257); `../completed/2026-09-07-inspection-workflow.md` | Landed; PR/post-merge CI passed; task Git cleanup and installed skill reconciliation complete |
| P: application evidence | Polyorama `db534693a36157e04323f0acec6a1a34e4c08994` / Lantern `c61fe1169538444feae0f40a7ccd7baed2f3a93a` | Version 1 owner bundle and fixed read-only live hook; 227 tests and 44 browser cases | [PR #13 landing evidence](https://github.com/robchristie/lantern/pull/13#issuecomment-5565670734); `../completed/2026-09-07-application-evidence.md` | Landed; PR/post-merge CI passed; installed skill validated and task Git cleaned |
| A: semantic DOM inspection | Lantern `bb9fdd6cd868571ab99a9d77fe83ff7834f0321d` / Q/B uses this base | Computed AX view and exact semantic targets; 231 tests and 64 browser cases | [PR #14 landing evidence](https://github.com/robchristie/lantern/pull/14#issuecomment-5565904470); `../completed/2026-09-07-semantic-dom.md` | Landed; PR/post-merge CI passed; installed skill validated and task Git cleaned |

Current phase: Q/B comparison candidate selected at
`32a32be21a2e2d8311153925a0c84ea7cd341076`. Calibration established equivalent
real CLI input and owned attachment lifecycles, corrected actual PNG viewport
alignment, and selected private observer reporting. Detailed decisions and
provisional observations are in `../../qualification/comparative-ui.md`.
Fresh five-task agents now use identical frozen fixture/browser/tool inputs;
complete their independent adjudication, aggregate checks and owner journey
before selecting the final report candidate.
P reviewed head `655e4f4acb80adb7667dd5d348e3fbd0793d248f` and landed revision
share tree `781fbca4b58d60fbee2f56ab5e674f8cb9d48431`; PR CI 34087614365 and
post-merge CI 34087942651 passed. Q remains partial until aggregate qualification and comparison are recorded.

## Terminal rule

Every row must be implemented and qualified, proved already satisfied/superseded,
or excluded by the authority established above. Reassess the exact final merged
head against this envelope, run aggregate representative integration, reconcile
support documentation and plans, and audit task branches/worktrees/remote heads.
Difficulty or unavailable evidence is not permission to mark a row complete.
