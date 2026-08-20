# Secret-safe file-backed form entry

Status: active

## Outcome

Let an authorised local operator insert a secret into one reviewed browser form
field without placing the secret in process arguments or Lantern output. The
new input is an additive alternative on `lantern type`; existing literal text
entry remains unchanged.

The motivating pilot is the development-only Geometis browser-test principal.
The immutable coordination plan is
[`codex-browser-test-identity.md`](https://github.com/robchristie/geometis-system/blob/781bde28ec5246d3480cb38d44bfa56f340a7e1b/plans/codex-browser-test-identity.md).

## Scope

- Add `lantern type --text-file <PATH>` as an alternative to `--text`.
- Require exactly one text source for `type`; retain `--text` behaviour for
  `wait text` and reject `--text-file` on every other command.
- Read at most 64 KiB of UTF-8 from an owner-private regular file without
  following a final symlink on supported Unix hosts.
- Preserve every input character, including leading/trailing whitespace and
  newlines, and continue reporting only the inserted character count.
- Update CLI, output, authenticated-testing and security documentation.

No credential store, credential discovery, path inference, password prompt,
browser target inference, form submission or authentication policy belongs in
Lantern.

## Design invariants

1. `type` requires exactly one of `--text` and `--text-file`; their precedence
   is never implicit.
2. The file is opened once before CDP access. On Unix the open uses no-follow,
   then validates the opened descriptor as a regular file with no group or
   other permission bits.
3. Files larger than 64 KiB and invalid UTF-8 fail before browser mutation.
   Empty files remain valid because literal empty text is already valid.
4. File errors use stable bounded messages and never include the path,
   contents, hash or input-source kind.
5. Success output is byte-for-behaviour compatible with literal text entry:
   the existing interaction schema reports only `inserted_text_length`.
   `--no-redact` does not weaken this boundary.
6. The secret remains an operator-owned file. Lantern does not create, copy,
   rotate, delete or persist it.

## Implementation sequence

1. Add CLI grammar, pre-CDP input resolution and owner-private file checks.
2. Add adversarial CLI/CDP fixtures for exclusivity, permission, type, size,
   encoding, symlink and transcript secrecy.
3. Update product/security/authenticated-testing documentation.
4. Run the canonical validation, publish a held pull request and obtain an
   independent exact-head review before the Geometis consumer pilot.

## Acceptance proof

- A mode-0600 UTF-8 file reaches exactly the selected input and reports only
  its character count in human and JSON output.
- The path and contents do not appear in stdout or stderr for success or
  failure, including with `--no-redact`.
- Literal plus file, missing source, non-type use, symlink, directory,
  group/other-readable mode, invalid UTF-8 and over-limit input all fail before
  a CDP request.
- Existing literal `type` and `wait text` fixtures remain unchanged.
- Rust 1.85 and the complete canonical repository gate pass.

## Progress

- 2026-08-20: Published the immutable Geometis coordination checkpoint and
  created an isolated Lantern owner worktree from `origin/main`.
- 2026-08-20: Fixed the additive CLI and filesystem-security contract before
  implementation.
- 2026-08-20: Implemented file-backed type input, stable bounded errors,
  transcript-secrecy and adversarial filesystem fixtures, plus the CLI,
  authenticated-testing, output and security documentation.
- 2026-08-20: Passed the standard canonical gate: current and Rust 1.85 locked
  all-target checks, 151 tests and the 28-path documentation check.
