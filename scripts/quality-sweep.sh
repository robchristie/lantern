#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

# Keep validation portable on machines without optional compiler wrappers.
# Developers can opt into compiler wrappers locally via environment or untracked Cargo config.
export RUSTC_WRAPPER=

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

skip() {
  printf '\n==> skipping %s; %s\n' "$1" "$2"
}

run ./scripts/validate.sh standard
run cargo clippy --workspace --all-targets --all-features -- -D warnings

if cargo deny --version >/dev/null 2>&1; then
  run cargo deny check
elif cargo audit --version >/dev/null 2>&1; then
  run cargo audit
else
  skip "dependency advisory check" "install cargo-deny or cargo-audit"
fi

if command -v typos >/dev/null 2>&1; then
  run typos
else
  skip "typos" "install typos-cli"
fi

if command -v taplo >/dev/null 2>&1; then
  mapfile -d '' toml_files < <(
    find . \
      -path './.git' -prune -o \
      -path './.smoogle' -prune -o \
      -path './target' -prune -o \
      -name '*.toml' -print0
  )
  if ((${#toml_files[@]} > 0)); then
    run taplo fmt --check "${toml_files[@]}"
  else
    skip "taplo" "no TOML files found"
  fi
else
  skip "taplo" "install taplo-cli"
fi

if cargo machete --version >/dev/null 2>&1; then
  run cargo machete
else
  skip "cargo machete" "install cargo-machete"
fi

if [[ "${SMOOGLE_COVERAGE:-0}" == "1" ]]; then
  if cargo llvm-cov --version >/dev/null 2>&1; then
    run cargo llvm-cov --workspace --all-features --summary-only
  else
    skip "cargo llvm-cov" "install cargo-llvm-cov"
  fi
else
  skip "cargo llvm-cov" "set SMOOGLE_COVERAGE=1 for periodic coverage evidence"
fi
