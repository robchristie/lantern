#!/usr/bin/env bash
set -euo pipefail

display="${DISPLAY:-:99}"
cdp_port="${CDP_PORT:-9222}"
vnc_port="${VNC_PORT:-5900}"
novnc_port="${NOVNC_PORT:-6080}"
profile_dir="${CHROME_USER_DATA_DIR:-/profile}"
geometry="${BROWSER_GEOMETRY:-1280x900x24}"
vnc_password="${VNC_PASSWORD:-}"
cdp_proxy_bind="${CDP_PROXY_BIND_ADDR:-}"

mkdir -p "$profile_dir"

pids=()

terminate() {
  for pid in "${pids[@]}"; do
    if kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done
}
trap terminate EXIT INT TERM

Xvfb "$display" -screen 0 "$geometry" -nolisten tcp &
pids+=("$!")

display_number="${display#:}"
for _ in {1..50}; do
  if [[ -S "/tmp/.X11-unix/X${display_number}" ]]; then
    break
  fi
  sleep 0.1
done

fluxbox >/tmp/fluxbox.log 2>&1 &
pids+=("$!")

vnc_args=(
  -display "$display"
  -forever
  -shared
  -rfbport "$vnc_port"
)

if [[ -n "$vnc_password" ]]; then
  vnc_args+=(-passwd "$vnc_password")
else
  vnc_args+=(-nopw)
fi

x11vnc "${vnc_args[@]}" >/tmp/x11vnc.log 2>&1 &
pids+=("$!")

websockify --web=/usr/share/novnc/ "$novnc_port" "127.0.0.1:${vnc_port}" >/tmp/novnc.log 2>&1 &
pids+=("$!")

if [[ -z "$cdp_proxy_bind" ]]; then
  cdp_proxy_bind="$(ip -o -4 addr show scope global | sed -n 's/.* inet \([0-9.]*\)\/.*/\1/p' | head -n 1)"
fi

if [[ -z "$cdp_proxy_bind" ]]; then
  printf 'could not determine container network address for CDP proxy\n' >&2
  exit 1
fi

socat "TCP-LISTEN:${cdp_port},bind=${cdp_proxy_bind},fork,reuseaddr" "TCP:127.0.0.1:${cdp_port}" >/tmp/cdp-proxy.log 2>&1 &
pids+=("$!")

chrome_args=(
  "--user-data-dir=${profile_dir}"
  "--remote-debugging-address=127.0.0.1"
  "--remote-debugging-port=${cdp_port}"
  "--no-first-run"
  "--no-default-browser-check"
  "--disable-background-networking"
  "--disable-client-side-phishing-detection"
  "--disable-component-update"
  "--disable-default-apps"
  "--disable-domain-reliability"
  "--disable-dev-shm-usage"
  "--disable-gpu"
  "--disable-sync"
  "--metrics-recording-only"
  "--no-service-autorun"
  "--password-store=basic"
  "--window-size=1280,900"
  "about:blank"
)

chrome_log="/tmp/chrome.log"
"${CHROME_BIN:-/opt/chrome/chrome}" "${chrome_args[@]}" >"$chrome_log" 2>&1 &
chrome_pid="$!"
pids+=("$chrome_pid")

cdp_ready=0
for _ in {1..100}; do
  if /usr/local/bin/browser-cdp-doctor --host 127.0.0.1 --port "$cdp_port" --quiet >/dev/null 2>&1; then
    printf 'Browser CDP runtime ready: http://127.0.0.1:%s\n' "$cdp_port"
    printf 'Chrome log: %s\n' "$chrome_log"
    cdp_ready=1
    break
  fi

  if ! kill -0 "$chrome_pid" >/dev/null 2>&1; then
    printf 'Chrome exited before CDP became ready. Last log lines:\n' >&2
    tail -n 80 "$chrome_log" >&2 || true
    wait "$chrome_pid"
  fi

  sleep 0.1
done

if [[ "$cdp_ready" -eq 0 ]]; then
  printf 'Chrome is still running, but CDP did not become ready within 10s. Chrome log: %s\n' "$chrome_log" >&2
fi

wait "$chrome_pid"
