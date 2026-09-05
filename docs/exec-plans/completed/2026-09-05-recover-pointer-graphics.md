# ExecPlan: Recover pointer and graphics work onto current main

Status: completed

## Outcome

Recovered and landed pointer/crop and managed graphics functionality while
preserving persistent profiles, host-gateway routing and secret-safe file-backed
input. Refreshed the installed CLI from merged main and verified the combined
behaviour. The original source remains recoverable on remote branch
`codex/archive-pointer-graphics-2026-09-05` at
`3a418f254f81789bd88db83460900d6af5b33db0`.

## Checkpoints

| Increment | Reviewed revision | Landed revision | Result | Evidence |
| --- | --- | --- | --- | --- |
| Pointer, wheel, crop and cleanup | `df58d48501e0f419f4df7a7cbb29b503e23d48b9` | `75d5173ac14ff5dcffb4e9b64c77a9183373c08c` | PASS: 162 tests, browser probe, independent review and CI | [PR #1](https://github.com/robchristie/lantern/pull/1) |
| Graphics and profile/gateway integration | `7644922afd977c115aa4cdae18a41412a49487c0` | `1157b17110d7814941ae074f3c60a863e82b429e` | PASS: 173 tests, rendered hardware/software pixels, profile restart, independent review and CI | [PR #5](https://github.com/robchristie/lantern/pull/5) |
| Installed binary and final integration | Source `1157b17110d7814941ae074f3c60a863e82b429e` | Same merged source | PASS: canonical verification and installed-binary runtime probes | [Landing and installation evidence](https://github.com/robchristie/lantern/pull/5#issuecomment-5551342498) |

Reviewed and landed trees matched for both code increments. Both pre-merge and
post-merge CI passed; detailed review and repair history lives on each PR.

## Repairs and qualification

The synthetic pointer fixture exposed the archive's scrolled-crop origin bug.
The repair translates viewport-relative crop coordinates using page offsets.
Independent review also found a missing mouse release after a rejected drag
movement; the repair attempts bounded cleanup at the last accepted point while
preserving the original error. Regression fixtures cover both repairs.

The final installed binary passed hover, wheel, timed drag and identical crop
pixels before/after scrolling in a hardware-WebGPU browser reached through the
explicit host-gateway mapping. The WebGPU fixture rendered red/green pixels,
reported a non-fallback NVIDIA/Ampere adapter and had clean pre-navigation
console/network observations. A synthetic named profile rendered SwiftShader
WebGL before and after stop/prune/restart. WebGPU with a named profile was
rejected without changing profile state.

Exact runtime/image/device identities and the reproducible synthetic probe are
recorded in [graphics recovery qualification](2026-07-27-managed-browser-hardware-webgpu.md#recovery-qualification--2026-09-05).
This remains selected Linux/Podman/NVIDIA and synthetic-workload evidence, not
other-device, Docker-runtime or production-browser parity. Release cleanup is
best effort if Chromium or its transport rejects it.

## Installation and retained state

`/home/rob/.cargo/bin/lantern` was installed using locked dependencies from
merged source `1157b17110d7814941ae074f3c60a863e82b429e`. SHA-256:
`b039896457a16bbf3eaada7bdee24b373cf8369fc26c1633bce1b37831b4f842`.
Its help exposes pointer, region, graphics/device and file-backed input options.
The stable Chrome 151.0.7922.47 image was unchanged: its entrypoint already
matches the reviewed entrypoint SHA-256
`662688cc8c05d81db84b0af047d32168853f322e7dc823f2fbeae916224684c8`.

Before replacement, the installed binary came from the August persistent-profile
worktree, lacked pointer/graphics options, and had SHA-256
`4df82c8531368d624d777ce04bfc215569a637f078a927e4e35b2dee24ca580b`.
A rollback copy and candidate verification records are retained under ignored
`.smoogle/recovery-2026-09-05/`; final installed-binary evidence is under ignored
`.smoogle/recovery-evidence/`. These are optional local recall aids; durable
acceptance results are recorded here and on the PRs.

All owned test browsers were stopped/pruned and synthetic profiles deleted.
Pointer/graphics worktrees, local task branches, tracking references and live
task heads were removed. The remote archive is deliberately retained. The
candidate-only image was removed, and the root checkout returned to clean main.
The documentation closeout's own landing and cleanup are recorded on its PR.
No release, deployment or live identity operation was performed.
