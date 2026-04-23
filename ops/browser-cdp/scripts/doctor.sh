#!/usr/bin/env bash
set -euo pipefail

host="127.0.0.1"
port="9222"
quiet=0

usage() {
  cat <<'USAGE'
usage: doctor.sh [--host HOST] [--port PORT] [--quiet]

Verifies that a Chrome DevTools Protocol endpoint answers /json/version.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      host="${2:?missing value for --host}"
      shift 2
      ;;
    --port)
      port="${2:?missing value for --port}"
      shift 2
      ;;
    --quiet)
      quiet=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

url="http://${host}:${port}/json/version"
body="$(curl -fsSL "$url")"

if [[ "$body" != *'"Browser"'* || "$body" != *'"webSocketDebuggerUrl"'* ]]; then
  printf 'CDP endpoint answered but did not look like /json/version: %s\n' "$url" >&2
  exit 1
fi

if [[ "$quiet" -eq 0 ]]; then
  printf 'CDP endpoint healthy: %s\n' "$url"
  printf '%s\n' "$body"
fi
