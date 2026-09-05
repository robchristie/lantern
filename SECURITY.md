# Security

This file records the repository's current security contract. It is scoped to local-first development and safe automation boundaries.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-08-20**

Rationale: `Lantern` has local-first defaults, `.smoogle/` excluded from Git,
conservative harness profiles, a documented secrets policy, redacted and
bounded CLI output by default, bounded click/type/key and pointer interaction metadata,
owner-private no-follow file-backed secret entry for `type`,
explicit screenshot artefact opt-in including region/crop coordinates, explicit opt-in managed browser
containers with loopback CDP publishing, owner-private named persistent
profiles, session-oriented flow evidence, and optional dependency advisory
checks in `scripts/quality-sweep.sh`. Persistent profiles retain sensitive
Chromium-owned session state by design, so the grade remains `C` until
dependency review, automated secret scanning, broader artefact handling, and
unsafe-boundary behaviour are better proven.

## Grade Scale

- `A`: clear threat model, strong secret handling, conservative automation defaults, repeatable security checks, and tested unsafe-boundary behavior.
- `B`: practical local security posture with documented boundaries and known gaps tracked.
- `C`: sensible defaults but mostly manual review for secrets, permissions, and automation safety.
- `D`: unclear threat model, risky automation defaults, or common secret-leak paths.
- `F`: known unsafe behavior that can expose secrets, overwrite untrusted state, or bypass operator intent.

Use `+` or `-` only when the repo is clearly between two grades.

## Local Threat Model

This repo primarily protects against:

- accidental credential disclosure through checked-in files, prompts, logs, or generated artifacts
- accidental disclosure of authenticated browser state through Lantern output, screenshots, transcripts, or run artifacts
- unintended file modification outside the selected repo/worktree
- unsafe unattended landing or pushing without explicit repo policy
- stale docs that mislead future agents about validation, sandboxing, or automation boundaries

This repo does not currently provide multi-user access control, hosted service hardening, compliance attestations, or guaranteed redaction of run artifacts.

## Secrets Policy

Do not commit secrets, tokens, private keys, credentials, customer data, or machine-specific sensitive configuration.

Use untracked local configuration, environment variables, or the operator's credential manager for secrets. Treat `.smoogle/config.toml`, `.smoogle/runs/`, prompts, logs, diagnostics, and shell history as sensitive.

## Safe Automation Boundaries

- Default to local-first operation.
- Keep `.smoogle/` out of Git.
- Managed browser lifecycle commands must be explicit; endpoint-based commands must not auto-start containers.
- GPU devices must be selected explicitly. Do not auto-discover a host GPU or
  silently enable `--enable-unsafe-webgpu`.
- Use hardware WebGPU only for disposable profiles loading trusted sites; it
  bypasses browser adapter safety policy and exposes a host device to the
  container.
- Publish managed CDP ports to host loopback only by default.
- Treat `browser start --host-gateway` as an explicit per-host routing choice:
  accept one bounded DNS hostname, map it only to the runtime's fixed
  `host-gateway` target, and never accept a caller-selected address or raw
  runtime argument.
- Create a persistent profile only for a dedicated automation identity. Never
  point Lantern at a daily browser profile or import a browser profile into its
  state home.
- Treat the persistent profile tree as a local credential store. Keep it
  owner-private, out of source worktrees, backups, support bundles and shared
  artefacts unless the operator intentionally secures that copy.
- Stop and prune preserve persistent profiles. Deletion requires the explicit
  `browser profile delete <NAME> --yes` command and fails while the profile is
  attached.
- Lantern must not expose cookie, local-storage, session-storage or password
  contents through profile metadata or lifecycle output.
- File-backed `type` input must come from an owner-private regular UTF-8 file,
  must not follow a final symlink on Unix, and must never expose its path,
  contents, hash or source kind in output or errors. `--no-redact` does not
  relax this boundary.
- Prefer `approval_policy = "never"` for non-interactive runs.
- Keep review profiles read-only unless a task explicitly requires mutation.
- Require explicit configuration for commit, push, mark-done, and auto-landing behavior.
- Do not make network access a hidden dependency for core local workflows.

## Known Security Gaps

- No automated secret scanning is configured yet.
- Dependency advisory checks are available through `scripts/quality-sweep.sh`, but the required tool is optional and no deny policy file is tuned yet.
- Run artifacts and explicitly captured screenshot files are not redacted; screenshots contain visible page pixels by design.
- Authenticated browser testing still relies on operator-owned dedicated
  profiles, loopback-only CDP exposure, service-controlled session expiry and
  manual review of transcripts and screenshots.
- Dependency and supply-chain policy is still first-version guidance.
- Console and network commands are bounded and redacted by default, but they still depend on continued review of new fields to avoid leaking sensitive browser state.
- Managed browser container images and runtime behavior have two-instance rootless Podman and Docker smoke evidence, but still require broader manual smoke review on representative hosts.
- The hardware-WebGPU trust boundary is proven on one rootless Podman/NVIDIA
  host; other runtimes, device selectors, drivers, and untrusted-page behavior
  have not been broadly exercised.

## Synthesized Boundaries

- CLI commands consume domain services rather than raw CDP responses
- JSON output shapes remain stable for future Smoogle and UI integration
- future TUI and web UI adapters reuse the same services and redaction policies
- operator-owned Chromium lifecycle remains supported alongside Lantern's
  explicit managed container and named-profile lifecycles
- future daemon mode remains optional and evidence-driven


## Maintenance Rules

Update this file when the threat model, secrets policy, sandbox posture, automation boundaries, or security-relevant known gaps change. Track deferred security work in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.
