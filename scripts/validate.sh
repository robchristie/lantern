#!/usr/bin/env bash
set -euo pipefail

profile="${1:-standard}"
repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

# Keep validation portable on machines without optional compiler wrappers.
# Developers can opt into compiler wrappers locally via environment or untracked Cargo config.
export RUSTC_WRAPPER=

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_tests() {
  if cargo nextest --version >/dev/null 2>&1; then
    run cargo nextest run --workspace --all-targets
  else
    run cargo test --workspace --all-targets
  fi
}

case "$profile" in
  docs)
    run git diff --check HEAD
    ;;
  fast)
    run cargo fmt --all -- --check
    run cargo check --workspace --all-targets
    if [[ -n "${FAST_TEST_ARGS:-}" ]]; then
      # shellcheck disable=SC2086
      run cargo test $FAST_TEST_ARGS
    else
      printf '\n==> skipping focused tests; set FAST_TEST_ARGS to run a narrow test filter\n'
    fi
    run git diff --check HEAD
    ;;
  standard)
    run cargo fmt --all -- --check
    run cargo check --workspace --all-targets
    run rustup run 1.85.0 cargo check --locked --workspace --all-targets
    run_tests
    run python3 scripts/test-comparison-adjudicator.py
    run git diff --check HEAD
    ;;
  *)
    printf 'usage: %s [docs|fast|standard]\n' "$0" >&2
    exit 2
    ;;
esac
