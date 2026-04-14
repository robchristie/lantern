# Security

This file records the repository's current security contract. It is scoped to local-first development and safe automation boundaries.

## Current Grade

Overall grade: **C**

Last reviewed: **2026-04-14**

Rationale: `Lantern` has local-first defaults, `.smoogle/` excluded from Git, conservative harness profiles, a documented secrets policy, redacted and bounded CLI output by default, explicit screenshot artifact opt-in, no hidden browser artifact persistence, and optional dependency advisory checks in `scripts/quality-sweep.sh`. The grade remains `C` until dependency review, automated secret scanning, broader artifact handling, and unsafe-boundary behavior are better proven.

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
- Prefer `approval_policy = "never"` for non-interactive runs.
- Keep review profiles read-only unless a task explicitly requires mutation.
- Require explicit configuration for commit, push, mark-done, and auto-landing behavior.
- Do not make network access a hidden dependency for core local workflows.

## Known Security Gaps

- No automated secret scanning is configured yet.
- Dependency advisory checks are available through `scripts/quality-sweep.sh`, but the required tool is optional and no deny policy file is tuned yet.
- Run artifacts and explicitly captured screenshot files are not redacted; screenshots contain visible page pixels by design.
- Authenticated browser testing still relies on operator-owned dedicated profiles, loopback-only CDP exposure, and manual review of transcripts and screenshots.
- Dependency and supply-chain policy is still first-version guidance.
- Console and network commands are bounded and redacted by default, but they still depend on continued review of new fields to avoid leaking sensitive browser state.

## Synthesized Boundaries

- CLI commands consume domain services rather than raw CDP responses
- JSON output shapes remain stable for future Smoogle and UI integration
- future TUI and web UI adapters reuse the same services and redaction policies
- operator-owned Chromium lifecycle stays outside Lantern in the first milestone
- future daemon mode remains optional and evidence-driven


## Maintenance Rules

Update this file when the threat model, secrets policy, sandbox posture, automation boundaries, or security-relevant known gaps change. Track deferred security work in `docs/exec-plans/tech-debt-tracker.md` or follow-up tasks.
