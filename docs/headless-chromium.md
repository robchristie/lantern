# Headless Chromium Setup

Lantern connects to an existing Chromium DevTools Protocol endpoint. It does not launch Chromium, supervise a browser process, create containers, manage VNC, or keep the browser alive. The operator owns the browser lifecycle and passes Lantern a local HTTP CDP endpoint.

Use `lantern doctor` after starting Chromium:

```sh
lantern doctor --endpoint http://127.0.0.1:9222
```

Or set the endpoint once for a shell:

```sh
export LANTERN_CDP_ENDPOINT=http://127.0.0.1:9222
lantern doctor
lantern targets
lantern page
```

Lantern's first milestone accepts only local HTTP endpoints such as `http://127.0.0.1:9222`, `http://localhost:9222`, or `http://[::1]:9222`. Do not pass the `ws://` debugger URL from `/json/version`; Lantern discovers and uses that internally when a command needs it.

## Common Chromium Flags

These flags are useful in most headless Linux and container setups:

```sh
chromium \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/lantern-chrome-profile \
  --no-first-run \
  --no-default-browser-check \
  --disable-background-networking \
  --headless=new \
  about:blank
```

Use the Chromium binary name available on the host: commonly `chromium`, `chromium-browser`, `google-chrome`, or `google-chrome-stable`.

Keep `--remote-debugging-address=127.0.0.1` unless the browser runs in another network namespace. Binding CDP to `0.0.0.0` exposes powerful browser control to the network and should only be used behind a deliberate local tunnel, firewall, or container port mapping.

Use a dedicated `--user-data-dir` for automation. Do not point a headless automation run at a personal browser profile that may contain cookies, logged-in sessions, extensions, or saved credentials.

## Headless Linux

For a local headless Linux host, start Chromium directly:

```sh
mkdir -p /tmp/lantern-chrome-profile

chromium \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/lantern-chrome-profile \
  --no-first-run \
  --no-default-browser-check \
  --disable-background-networking \
  --headless=new \
  about:blank
```

Then verify the endpoint from the same host:

```sh
lantern doctor --endpoint http://127.0.0.1:9222
```

If Chromium reports that a display is required, the installed build may not support the newer headless mode. Try `--headless` instead of `--headless=new`, or use the VNC-compatible pattern below.

## Containers

When Chromium runs in a container and Lantern runs on the host, bind Chromium inside the container to all container interfaces and publish the port only to host loopback:

```sh
docker run --rm -it \
  -p 127.0.0.1:9222:9222 \
  --shm-size=1g \
  chromium-image \
  chromium \
    --remote-debugging-address=0.0.0.0 \
    --remote-debugging-port=9222 \
    --user-data-dir=/tmp/lantern-chrome-profile \
    --no-first-run \
    --no-default-browser-check \
    --disable-background-networking \
    --headless=new \
    --disable-dev-shm-usage \
    --no-sandbox \
    about:blank
```

The important boundary is the Docker publish rule: `-p 127.0.0.1:9222:9222` makes the CDP endpoint available to the host as `http://127.0.0.1:9222` without exposing it on every host interface.

Use `--no-sandbox` only when the container image or runtime cannot support Chromium's sandbox. Prefer a container/runtime configuration that keeps the sandbox enabled when practical.

If Lantern runs in the same container as Chromium, keep Chromium bound to `127.0.0.1` and use `--endpoint http://127.0.0.1:9222` inside that container.

If Lantern runs in a different container, expose Chromium to Lantern as a loopback HTTP endpoint inside Lantern's network namespace, for example with an explicit port-forwarding sidecar or by running both processes in the same container namespace. A plain Docker service name such as `http://chromium:9222` is outside Lantern's first-milestone endpoint contract because the CLI currently accepts only local HTTP hosts.

## VNC-Compatible Browser Sessions

Some frontend debugging loops need a visible browser that an operator can inspect through VNC or noVNC. In that case, run Chromium normally inside an X server session and enable CDP:

```sh
export DISPLAY=:1
mkdir -p /tmp/lantern-chrome-profile

chromium \
  --remote-debugging-address=127.0.0.1 \
  --remote-debugging-port=9222 \
  --user-data-dir=/tmp/lantern-chrome-profile \
  --no-first-run \
  --no-default-browser-check \
  --disable-background-networking \
  about:blank
```

The X server, window manager, VNC server, and noVNC proxy remain operator-managed. Lantern only needs the local CDP HTTP endpoint.

For containerized VNC environments, keep the browser and VNC stack in the container image or supervisor you already use, publish CDP to host loopback only, and verify from the host:

```sh
lantern doctor --endpoint http://127.0.0.1:9222
```

## Verification And Troubleshooting

Check Chromium's CDP metadata directly when `lantern doctor` cannot connect:

```sh
curl -sS http://127.0.0.1:9222/json/version
curl -sS http://127.0.0.1:9222/json/list
```

Common fixes:

- `endpoint_missing`: pass `--endpoint http://127.0.0.1:9222` or set `LANTERN_CDP_ENDPOINT`.
- `endpoint_invalid`: use a local `http://` endpoint without credentials, query string, or fragment.
- `endpoint_unreachable`: confirm Chromium is still running and that the port is published into Lantern's network namespace.
- `cdp_unhealthy`: make sure the endpoint is Chromium's DevTools HTTP endpoint, not an application page URL.
- no page target: open a page in the attached Chromium instance or start Chromium with a URL instead of `about:blank`.

After `doctor` succeeds, use:

```sh
lantern targets --endpoint http://127.0.0.1:9222
lantern page --endpoint http://127.0.0.1:9222
```
