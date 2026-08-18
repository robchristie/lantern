# Persistent authenticated managed browser profiles

Status: active

## Outcome

Add an explicit named profile lifecycle to Lantern so a dedicated authenticated
browser can be stopped and restarted without destroying its Chromium-owned
session state. Preserve the current disposable managed-browser lifecycle as the
default and keep authentication, credentials and cookie contents outside
Lantern's command and metadata surfaces.

The motivating canary is repeated managed-OIDC browser proof for Geometis. The
immutable coordination plan is
[`persistent-authenticated-browser.md`](https://github.com/robchristie/geometis-system/blob/f1ba8555211364853c879d832fe56d33a58f7f52/plans/persistent-authenticated-browser.md).

## Scope

- Add owner-private persistent profile storage under the operator's Lantern
  state home rather than a repository worktree.
- Add `lantern browser profile create|list|status|delete` and
  `lantern browser start --profile <NAME>`.
- Keep persistent instance metadata under the same operator state home so the
  workflow is independent of the inspected repository's `.smoogle` directory.
- Preserve profiles across `stop`, ordinary `prune`, failed starts and task
  cleanup; require an explicit confirmed profile delete.
- Enforce one starting/running browser per profile with recoverable stale
  attachment handling.
- Update the tracked inspection skill, product contract, architecture,
  authenticated-testing workflow and security record.

No cookie import/export, credential entry, session introspection, remote CDP,
authentication bypass, automatic account selection or Keycloak policy change
is included.

## Design invariants

1. `browser start` without `--profile` retains the existing repository-local
   disposable profile and prune behaviour.
2. Persistent profile names use the existing bounded browser-ID grammar and
   are never interpreted as paths.
3. The state root resolves from `LANTERN_STATE_HOME`, then
   `XDG_STATE_HOME/lantern`, then `$HOME/.local/state/lantern`; it must be
   absolute. Profile and persistent-instance paths are derived beneath it.
4. Persistent directories are mode `0700` and metadata files are mode `0600`
   on Unix. Symlink profile directories and non-directory paths fail closed.
5. Profile metadata contains only name, timestamps and a bounded attachment
   reservation. Lantern never reads browser databases to determine login state.
6. Attachment reservation is compare-and-swap under the profile registry lock.
   Start, stop, prune and delete additionally hold a process-scoped advisory
   lock for the complete profile transition. A second lifecycle command fails;
   a stopped owner can be replaced after the prior process exits, while a
   missing owner remains reserved for a bounded starting grace period before
   stale recovery. This fences both suspended processes and record/runtime
   side effects without making a crashed process permanently own the profile.
7. Stop releases the attachment but preserves the profile. Prune removes only
   stopped instance/container records. Profile deletion requires `--yes` and
   no attachment.
8. Existing lifecycle JSON remains additive: instance results gain explicit
   `profile_kind` and nullable `profile_name`; existing fields remain present.
9. Runtime ports stay random and loopback-only. Existing Podman/Docker mount,
   ownership and image behaviour remain unchanged.

## Implementation sequence

1. Add profile kinds, records, private storage, containment, reservation and
   deletion operations in `lantern-storage`, with adversarial filesystem and
   concurrency tests.
2. Add the CLI grammar, validation, output and persistent start/stop/prune
   integration while retaining disposable compatibility tests.
3. Update docs, the tracked inspection skill and security/reliability records.
4. Run the canonical validation and disposable/persistent container smoke.
5. Publish, independently review and land the Lantern owner before the
   Geometis authenticated restart canary.

## Acceptance proof

- Unit tests cover private modes, traversal and symlink rejection, duplicate
  creation, profile metadata, reservation conflict/CAS/release, active delete
  rejection and explicit deletion.
- CLI tests cover new grammar/flags/errors and unchanged disposable parsing.
- A real managed browser writes a non-secret local-storage canary, stops,
  restarts from the named profile and retains it; prune does not remove the
  profile. A separate disposable instance remains pruneable.
- The Geometis canary authenticates once through noVNC, stops and restarts the
  browser, then reaches the managed public route and identity session without a
  second login while the real Keycloak session is valid.
- No source checkout becomes dirty from persistent runtime state, and no test
  or transcript reads cookies, passwords or raw browser storage.

## Progress

- 2026-08-16: Confirmed the earliest failed hand-off is the disposable profile
  lifecycle, not Geometis authentication or noVNC itself.
- 2026-08-16: Created the isolated owner and system worktrees and published the
  immutable coordination checkpoint at `f1ba8555211364853c879d832fe56d33a58f7f52`.
- 2026-08-16: Implemented private named-profile storage, attachment
  compare-and-swap, explicit CLI lifecycle, additive instance provenance and
  adversarial filesystem/CLI tests while preserving disposable defaults.
- 2026-08-16: Passed the real rootless-Podman canary. A non-secret
  `localStorage` marker survived stop, prune and restart on new random loopback
  ports; concurrent start and active delete failed closed; profile directories
  were mode `0700`, metadata was mode `0600`, and confirmed deletion removed
  the stopped profile.
- 2026-08-16: Forced a post-container-start readiness failure with a one
  millisecond timeout. Lantern removed the failed container before releasing
  the attachment, left the profile unattached and permitted explicit deletion.
- 2026-08-16: Hardened missing-container resolution so a failed runtime inspect
  is not itself proof of absence. Persistent ownership is released or recovered
  only after a successful all-container listing proves the managed name is
  absent, or after explicit removal succeeds.
- 2026-08-16: Repaired exact-head review findings by binding every instance
  record and recursive removal to a validated direct registry child, adding a
  unique per-start reservation token, preserving fresh starting reservations
  during prune, retaining ownership on uncertain runtime state, rejecting
  symlinked records and temporary files, and keeping disposable commands usable
  without a configured persistent state home.
- 2026-08-16: Closed the stop/restart hand-off by atomically moving the exact
  reservation to a `stopping` state before requesting browser shutdown. A
  concurrent same-name recovery cannot acquire that reservation until stop has
  positively completed and released it.
- 2026-08-16: Added graceful `Browser.close` handling before persistent
  container stop. The repeated rootless-Podman canary positively matched the
  non-secret marker after stop, prune and restart; Chromium also restored the
  prior page, so the second inspection selected that exact page target.
- 2026-08-16: Serialised each persistent profile's complete lifecycle with a
  process-scoped advisory lock. Exact attachment state is now part of release
  ownership, prune removes stale instance state before releasing ownership, and
  delete holds the same lock through cleanup. This prevents suspended starts,
  stop/prune overlap and restart/delete races while preserving crash recovery.
- 2026-08-16: Brought persistent status refreshes under the same lifecycle
  fence, replaced the newer standard-library file-lock calls with an
  MSRV-compatible advisory-lock dependency, and reserved persistent instance
  identities so profile and disposable lifecycles cannot collide on one
  global record/container identity.
  Persistent prune now retains uncertain runtime states rather than treating
  them as stopped.
- 2026-08-16: Restored a truthful Rust 1.85 lock by selecting compatible ICU4X
  transitive revisions and added the declared MSRV workspace check to the
  canonical standard validation profile.
- 2026-08-16: Qualified each persistent runtime identity with a stable digest
  of its canonical state root, rejected persistent `--id` overrides, and made
  an unowned reserved runtime or legacy record fail closed. Identically named
  profiles in distinct supported state homes can no longer replace one
  another's global container.

## Observations

- Persistent state must be physically separate from disposable instance
  directories. The existing prune contract recursively removes disposable
  instance layouts, so a durability flag inside that tree would be too easy to
  violate during cleanup.
- Chromium restores origin-scoped storage as expected when the same profile data
  directory is rebound into a replacement container. Lantern does not need to
  understand or inspect browser storage to provide the lifecycle guarantee.
- Container restart changes CDP and noVNC host ports. Callers must resolve the
  endpoint from the current instance record instead of retaining an old URL.
