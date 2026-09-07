# GPU and canvas inspection

Read this reference for WebGL, WebGPU or another GPU-backed canvas. Graphics
mode determines what the evidence can establish. Use the selected executable
and lifecycle cleanup in [browser-sessions.md](browser-sessions.md).

Confirm the selected binary's current surface before startup:

```bash
lantern capabilities --json
lantern browser start --help
```

The default managed graphics mode is disabled. A failure from that mode is not
evidence that the application or browser image lacks WebGL support.

For a hermetic software WebGL smoke check, use SwiftShader:

```bash
ID="$(lantern browser start --graphics swiftshader --json | jq -r .instance.id)"
```

Do not treat SwiftShader as WebGPU or production-GPU coverage. Use
`--graphics gpu` only when the operator has configured explicit device
passthrough.

For hardware WebGPU, use an operator-selected device in a disposable browser
and only against a trusted application:

```bash
ID="$(lantern browser start \
  --graphics webgpu \
  --gpu-device nvidia.com/gpu=0 \
  --json | jq -r .instance.id)"
```

Confirm startup reports `graphics=webgpu`, the intended `gpu_device`, and
`unsafe_webgpu=true`. This mode exposes the selected host GPU to the container
and opts into Chrome's unsafe WebGPU boundary. It cannot be combined with a
named persistent profile.

For any canvas result, collect structured readiness, console and network
evidence, capture at every task-relevant viewport, and open every image. Require
visibly useful, nonblank pixels plus the expected application ready/rendered
state. Judge expected scene content, overlays, controls, alignment, clipping
and loading/empty/error state explicitly. A clean console or CDP-ready browser
does not prove adapter creation, presentation or correct pixels.

State the scope of the result. Hardware WebGPU here is evidence for the selected
Linux/Vulkan device, not for production Chrome, Metal, D3D12 or other devices.
Stop and prune the disposable instance after inspection.
