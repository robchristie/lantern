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
