# CLI foundation and capability discovery

Status: completed

Programme package C; implementation and local qualification complete.
Independent review and landing remain with the programme coordinator.
Baseline: `0f4b87815e968b0bafea3d35b86c0273009d066d`.
The programme coordinator owns independent review, publication, CI and landing.

## Outcome and boundary

Split CLI ownership into parsing/validation, error/output, target selection and
browser, inspection, interaction and screenshot command families. Preserve
existing command behaviour, human/JSON output and executable fixture contracts.
Add endpoint-independent machine-readable package/build identity, command names,
aliases and supported output schema versions. Preserve `--version` output.
No interaction semantics, flow actions, layout or application adapter expansion.

## Decisions and acceptance

- Keep bounded modules inside `lantern-cli` and pass a typed endpoint context;
  avoid a new crate or backend abstraction. Move tests with semantic owners.
- One registry generates command variants and parsing names and exposes discovery;
  schema versions come from their existing output owners.
- Only a verified owning checkout supplies a build commit; modified state is
  explicit. Archives, unavailable Git and unrelated enclosing repositories report
  unknown identity. Refresh provenance on every Cargo build, including ref-only
  changes, linked worktrees and detached HEADs.
- Acceptance: unchanged executable CDP fixtures; discovery without an endpoint;
  package/version, registry and schema consistency; real Cargo rebuild probes
  across commits, modified trees, archives and unrelated Git environments;
  canonical `scripts/validate.sh` including locked Rust 1.85 and docs hygiene.

## Acceptance evidence

`scripts/validate.sh` passed formatting, workspace/all-target checks, locked
Rust 1.85 checks, 201 nextest tests (201 passed, none skipped) and docs hygiene.
All 73 existing executable CDP fixtures are unchanged and pass. All 33 baseline
CLI unit tests moved with their semantic owners and remain present. The only
changed existing command paths are orchestration/dispatch and registry-backed
name parsing; command handlers, output writers and stable errors are preserved.

Six added tests cover registry parsing, endpoint-independent executable discovery,
package/version/schema and alias consistency, execution-only flag rejection, and
actual offline Cargo rebuilds. The rebuild fixtures prove new commit identity
without source changes, packed refs, detached HEADs, tracked/untracked modified
state and return to clean, linked worktrees, exported sources in an unrelated
repository, and caller-supplied Git routing. Staging exported sources in an
unrelated new repository cannot claim its existing empty commit.

## Next action and limits

The programme coordinator independently reviews the exact committed candidate,
repairs findings on this branch, and owns publication, CI, landing and cleanup.
Detailed exact-head acceptance and review/landing evidence belong to the package
pull request. After landing, select I with initial real-Chromium fixture
infrastructure. Package C does not establish new browser interaction behaviour.

Build identity covers Git-visible source state and is not an artefact signature
or reproducible-build claim. Exported archives deliberately report unknown
identity; they have no separately trusted provenance channel. The build script
refreshes provenance on every Cargo invocation to avoid stale ref/state identity.
