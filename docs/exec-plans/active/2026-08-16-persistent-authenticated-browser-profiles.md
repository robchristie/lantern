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
   A second active owner fails; a stopped owner can be replaced, while a
   missing owner remains reserved for a bounded starting grace period before
   stale recovery. This prevents another process from claiming the profile in
   the interval between reservation and container creation.
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
