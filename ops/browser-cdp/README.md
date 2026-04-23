# Browser CDP Runtime

This directory contains the Containerfile used by `lantern browser start`.
It runs Chrome for Testing with Xvfb, fluxbox, x11vnc, noVNC, and a loopback
Chrome DevTools Protocol endpoint proxied for container port publishing.

Build the default image from the repository root:

```sh
podman build -f ops/browser-cdp/Containerfile -t localhost/lantern-browser-cdp:stable ops/browser-cdp
```

Docker can build the same image:

```sh
docker build -f ops/browser-cdp/Containerfile -t localhost/lantern-browser-cdp:stable ops/browser-cdp
```

The container keeps Chromium's internal CDP listener on `127.0.0.1:9222` and
exposes a small internal proxy on the container network address. Lantern
publishes CDP to a random host loopback port by default.

When Lantern starts this image through Docker, it runs the container as the
host profile-directory owner, sets `HOME=/tmp`, and sets
`CHROME_NO_SANDBOX=1`. The entrypoint converts that flag to Chromium
`--no-sandbox` for Docker environments where the Chromium sandbox is not
available inside the container.
