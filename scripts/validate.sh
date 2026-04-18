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

run_docs_check() {
  if [[ -n "${SMOOGLE_BIN:-}" ]]; then
    run "$SMOOGLE_BIN" docs check
  elif [[ -x "$repo_root/.smoogle/bin/smoogle-cli" ]]; then
    run "$repo_root/.smoogle/bin/smoogle-cli" docs check
  elif command -v smoogle >/dev/null 2>&1; then
    run smoogle docs check
  elif command -v smoogle-cli >/dev/null 2>&1; then
    run smoogle-cli docs check
  elif [[ -f crates/smoogle-cli/Cargo.toml ]]; then
    run cargo run -q -p smoogle-cli -- docs check
  else
    printf '\n==> skipping smoogle docs check; set SMOOGLE_BIN, keep .smoogle/bin/smoogle-cli, or install smoogle\n'
  fi
}

run_tests() {
  if cargo nextest --version >/dev/null 2>&1; then
    run cargo nextest run --workspace --all-targets
  else
    run cargo test --workspace --all-targets
  fi
}

case "$profile" in
  fast)
    run cargo fmt --all -- --check
    run cargo check --workspace --all-targets
    if [[ -n "${FAST_TEST_ARGS:-}" ]]; then
      # shellcheck disable=SC2086
      run cargo test $FAST_TEST_ARGS
    else
      printf '\n==> skipping focused tests; set FAST_TEST_ARGS to run a narrow test filter\n'
    fi
    run_docs_check
    ;;
  standard)
    run cargo fmt --all -- --check
    run cargo check --workspace --all-targets
    run_tests
    run_docs_check
    ;;
  *)
    printf 'usage: %s [fast|standard]\n' "$0" >&2
    exit 2
    ;;
esac
