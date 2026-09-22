# Remove the retired harness

## Outcome and acceptance

Lantern builds, validates and documents its own CLI without the retired Smoogle
harness. New local state and test artefacts use `.lantern/`. Existing disposable
browser ownership and mounted profile paths remain usable; persistent profiles
and historical qualification evidence remain intact.

## Decisions

- Remove the unused harness configuration, generated bootstrap interview,
  harness prompt templates and dashboard-specific skill reference.
- Replace the optional external docs check with a repository whitespace check;
  changed documentation links and paths also receive manual inspection.
- Retain the old disposable registry only when it exists, with precedence over
  the new root. Never move live profile mounts or silently hide malformed state.
- Keep both local roots ignored and preserve historical evidence paths.

## Progress

- Implementation and documentation cleanup complete.
- Standard validation passed: 241 Rust tests and four comparison-adjudication
  tests, formatting and current/MSRV workspace checks.
- Shell, JavaScript and Python syntax and documentation references checked.
- Exact-candidate runner smoke, independent review, CI and final landing evidence
  are recorded in the associated pull request.

## Validation

No earlier evidence covered the changed registry selection or script paths.
Validation commands: `scripts/validate.sh`, shell/JavaScript syntax checks, documentation reference
inspection, and a disposable comparison-owner start/status/stop smoke using the
new dependency directory. CI supplies the real-Chromium contract suite.
