# Managed Browser Hardware WebGPU

Recovery note (2026-09-05): historical archive evidence; current candidate
qualification is tracked in `../active/2026-09-05-recover-pointer-graphics.md`.

Status: completed

## Summary

Lantern's managed browser can disable graphics, run software WebGL through
ANGLE/SwiftShader, or omit `--disable-gpu` for an operator-managed GPU. The
last option does not inject a device or enable Chrome's Vulkan WebGPU path, so
it cannot currently render the WebGPU-based Lantern Meadow application from a
rootless Podman container.

Add a narrow, explicit hardware-WebGPU mode that preserves existing defaults,
keeps device selection operator-owned, records the unsafe boundary, and can be
dogfooded through the existing CDP inspection commands.

## Contract

- Default managed-browser behavior remains `--graphics disabled`.
- Existing `swiftshader` remains a software WebGL mode and is not presented as
  WebGPU coverage.
- Existing `gpu` remains a generic GPU-enabled Chrome mode.
- New `--graphics webgpu` selects the tested Chrome Linux Vulkan WebGPU launch
  flags and is explicitly documented as an unsafe, trusted-site development
  mode rather than production-browser parity.
- New `--gpu-device <DEVICE>` passes one explicit container-runtime `--device`
  selector. It is accepted only with `--graphics gpu` or `--graphics webgpu`;
  `webgpu` requires it so a launch cannot silently claim hardware coverage
  while receiving no hardware.
- The tested host selector is the NVIDIA Container Toolkit CDI name
  `nvidia.com/gpu=0`; raw DRM paths remain operator choices for other drivers.
- Managed-instance records and JSON/human output include the graphics mode and
  selected GPU device. Older local records deserialize as graphics-disabled
  with no device.
- The browser image maps `CHROME_GRAPHICS=webgpu` to
  `--use-angle=vulkan`, `--enable-features=Vulkan`,
  `--disable-vulkan-surface`, and `--enable-unsafe-webgpu` while retaining the
  existing Xvfb, VNC, loopback-CDP, isolated-profile, and cleanup boundaries.
- No endpoint command auto-starts a browser, no GPU device is auto-detected or
  silently selected, and no unsafe WebGPU flag becomes a default.

## Progress

- [x] Reproduce the managed-browser adapter failure in disabled, generic GPU,
  and WebGL SwiftShader modes.
- [x] Prove the same Chrome-for-Testing image renders Lantern Meadow when Xvfb,
  NVIDIA CDI, Vulkan, and unsafe WebGPU are combined.
- [x] Implement the runtime command and persistent metadata contract.
- [x] Implement CLI parsing, validation, help, and output.
- [x] Add the browser-image WebGPU launch case.
- [x] Update product, workflow, skill, reliability, and security guidance.
- [x] Run fast and full repository validation.
- [x] Rebuild the CLI and managed-browser image.
- [x] Dogfood the new managed mode against Lantern Meadow with a nonblank
  screenshot, clean page console/network evidence, and explicit cleanup.
- [x] Move this plan to `completed/` and record the final validation evidence.

## Decisions

- Use a distinct `webgpu` mode instead of silently broadening `gpu`; the Chrome
  flags bypass Linux/X11 rollout checks and deserve an obvious trust boundary.
- Require explicit `--gpu-device` for `webgpu`; device injection is vendor- and
  runtime-specific, and an explicit failure is more reliable than accidental
  software fallback or an adapter-null page.
- Pass a single opaque device selector directly after runtime `--device` rather
  than add NVIDIA-specific concepts to Lantern's domain model.
- Keep WebGPU SwiftShader out of this milestone. A direct probe created a Dawn
  device but Chrome destroyed it when its software path could not create the
  WebGPU swap-chain SharedImage backing.
- Treat successful CDP readiness as browser-process readiness only. Acceptance
  requires application-level rendered pixels plus clean flow evidence.

## Validation

- Parser tests for `webgpu`, `--gpu-device`, missing values, invalid mode/device
  combinations, and WebGPU-without-device rejection.
- Storage/runtime tests for the `--device` argument, `CHROME_GRAPHICS=webgpu`,
  record compatibility defaults, and output metadata.
- Entrypoint shell syntax validation and a rebuilt container image.
- `scripts/validate.sh fast` during implementation and `scripts/validate.sh`
  before completion.
- `smoogle docs check` through the repository validation entrypoint.
- Rootless Podman dogfood using `nvidia.com/gpu=0`, followed by `lantern flow`,
  DOM/status inspection, screenshot capture, `browser stop`, and
  `browser prune`.

Final evidence on 2026-07-27:

- `scripts/validate.sh` passed formatting, workspace checks, all 130 tests, and
  the documentation contract check.
- `cargo build --release -p lantern-cli` completed, and the installed CLI help
  exposes `webgpu` plus `--gpu-device`.
- `localhost/lantern-browser-cdp:stable` rebuilt with Chrome for Testing
  151.0.7922.47.
- A managed rootless-Podman session started with
  `--graphics webgpu --gpu-device nvidia.com/gpu=0`, rendered Lantern Meadow,
  and produced `/tmp/lantern-managed-webgpu-proof.png` at 1279 x 756 with
  visible game pixels.
- The observation flow reported zero console messages, zero exceptions, zero
  failed requests, and zero HTTP errors. The container reported an NVIDIA
  GeForce RTX 3090 with driver 610.43.03 and explicit NVIDIA/DRM device maps.
- `browser stop`, `browser prune`, `browser list`, and the Podman label query
  confirmed that no managed browser instance remained.
- `scripts/quality-sweep.sh` reached strict Clippy and stopped on the existing
  `clippy::single_match` warning in `crates/lantern-core/src/cdp.rs`; this plan
  does not modify that file, and the standard repository validation remains
  green.


## Recovery qualification — 2026-09-05

The recovered integration was qualified at
`27088bac31d35bd2afd39e1b6a7c40726c9e7600` on top of pointer landing
`75d5173ac14ff5dcffb4e9b64c77a9183373c08c`.

- Browser image: `sha256:812689b74fc4ca018ccba0acfeac949b0fbd2dbfd301c807c9d93b4a9507d286`,
  local tag `localhost/lantern-browser-cdp:recovery-2026-09-05`.
  Built by copying the recovered entrypoint into immutable existing base image
  `sha256:b0e3d80abba6a43e9a9790664d10109c70452dbc98a00859a0e2e36cbe3d7b3f`;
  the repository Containerfile and other image inputs did not change.
  Chrome for Testing 151.0.7922.47; entrypoint SHA-256
  `662688cc8c05d81db84b0af047d32168853f322e7dc823f2fbeae916224684c8`.
- Podman 6.0.0, explicit CDI selector `nvidia.com/gpu=0`, NVIDIA RTX 3090
  UUID `GPU-45bd98b8-90fa-f274-d033-87c48ecbefd4`.
- Trusted synthetic fixture served inside the disposable container on localhost.
  WebGPU reported NVIDIA/Ampere, `isFallbackAdapter=false`, and rendered red
  `[255,0,0]` and green `[0,255,0]` sample pixels in a 256 x 256 PNG.
  The pre-navigation flow observed no console messages/exceptions or network
  failures/HTTP errors and reported no collection gaps.
- The same WebGPU session passed hover, wheel, timed drag and scrolled viewport
  cropping through the explicit `host.docker.internal` host-gateway mapping.
- A newly created synthetic named profile rendered SwiftShader WebGL before and
  after stop/prune/restart. WebGPU with that named profile returned usage and
  left its profile state unchanged. All owned browsers were stopped/pruned,
  and the synthetic profile was explicitly deleted.
- Untracked evidence lives in `.smoogle/recovery-evidence/`: graphics PNG/JSON,
  `profile-evidence.json`, `webgpu-pointer/`, image build and canonical logs.
  The recovery coordinator retains these under the root checkout's ignored
  `.smoogle/recovery-2026-09-05/` when removing worktrees.

Retain the recovered candidate. This proves the selected Linux/NVIDIA hardware
and software-WebGL paths, not other devices, production browser parity, or
hardware WebGPU with named profiles (which is deliberately rejected).
