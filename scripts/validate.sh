#!/usr/bin/env bash
set -euo pipefail

profile="${1:-standard}"
if [[ $# -gt 0 ]]; then
  shift
fi
repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

# Keep validation portable on machines without optional compiler wrappers.
# Developers can opt into compiler wrappers locally via environment or untracked Cargo config.
export RUSTC_WRAPPER=

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

require_nextest() {
  if ! cargo nextest show-config version; then
    printf 'Nextest is required and must satisfy .config/nextest.toml; see docs/workflows.md#validation-loop for installation.\n' >&2
    exit 1
  fi
}

prepare_tests() {
  # Failed setup or compilation must not leave an old report looking current.
  rm -f -- .lantern/nextest/ci/junit.xml
  require_nextest
}

run_tests() {
  run cargo nextest run --locked --workspace --all-targets --profile ci
  run cargo test --locked --workspace --doc
}

case "$profile" in
  docs)
    run git diff --check HEAD
    ;;
  fast)
    focused_test_args=("$@")
    if [[ -n "${FAST_TEST_ARGS:-}" ]]; then
      if [[ "$FAST_TEST_ARGS" == *$'\n'* ]]; then
        printf 'FAST_TEST_ARGS must be a single line; pass positional arguments after fast instead.\n' >&2
        exit 2
      fi
      if [[ ${#focused_test_args[@]} -gt 0 ]]; then
        printf 'Pass focused arguments after fast or through FAST_TEST_ARGS, not both.\n' >&2
        exit 2
      fi
      # Legacy input is a single line of whitespace-separated tokens. Use
      # positional arguments for filters containing spaces; never evaluate it.
      read -r -a focused_test_args <<< "$FAST_TEST_ARGS"
    fi
    if [[ ${#focused_test_args[@]} -gt 0 ]]; then
      require_nextest
    fi
    run cargo fmt --all -- --check
    run cargo check --locked --workspace --all-targets
    if [[ ${#focused_test_args[@]} -gt 0 ]]; then
      run cargo nextest run --locked "${focused_test_args[@]}"
    else
      printf '\n==> skipping focused tests; pass Nextest arguments after fast to select tests\n'
    fi
    run git diff --check HEAD
    ;;
  standard)
    prepare_tests
    run python3 scripts/test-validation.py
    run cargo fmt --all -- --check
    run cargo check --locked --workspace --all-targets
    run rustup run 1.85.0 cargo check --locked --workspace --all-targets
    run_tests
    run python3 scripts/test-comparison-adjudicator.py
    run git diff --check HEAD
    ;;
  tests)
    prepare_tests
    run python3 scripts/test-validation.py
    run_tests
    ;;
  *)
    printf 'usage: %s [docs|fast [NEXTEST_ARGS...]|standard|tests]\n' "$0" >&2
    exit 2
    ;;
esac
