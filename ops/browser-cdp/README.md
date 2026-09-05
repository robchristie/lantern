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

The managed browser defaults to `CHROME_GRAPHICS=disabled`, preserving the
historical `--disable-gpu` launch behavior. For WebGL smoke checks without host
GPU passthrough, start through Lantern with `--graphics swiftshader`; the
entrypoint enables Chrome's SwiftShader/ANGLE software renderer. For
operator-managed GPU passthrough experiments, use `--graphics gpu`.

For a hardware-backed WebGPU session, select the reviewed WebGPU launch mode
and pass one explicit container-runtime device selector:

```sh
lantern browser start \
  --graphics webgpu \
  --gpu-device nvidia.com/gpu=0 \
  --json
```

`webgpu` enables Chrome's Vulkan path with `--enable-unsafe-webgpu`. It is an
opt-in development mode for disposable profiles and trusted sites, not a claim
that ordinary production Chrome would expose the same adapter. Lantern does
not auto-detect devices. NVIDIA CDI selectors require the NVIDIA Container
Toolkit; other runtimes and drivers may use a path such as
`/dev/dri/renderD128`. Always require application-level rendered pixels in
addition to CDP readiness.


Named persistent profiles can use `disabled`, `swiftshader`, or `gpu` graphics.
Hardware `webgpu` is restricted to disposable trusted-site sessions: combining
`--graphics webgpu` with `--profile` is rejected before state or runtime access.
