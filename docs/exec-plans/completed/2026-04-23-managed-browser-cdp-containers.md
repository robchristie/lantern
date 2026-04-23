# Managed Browser CDP Containers

Status: completed

## Goal

Add an explicit, opt-in Lantern-managed browser lifecycle MVP for disposable
Chromium/CDP containers. Existing inspection and interaction commands remain
endpoint-based and never auto-start a browser.

## Context

Lantern currently documents operator-owned Chromium/CDP endpoints. That works
for a single manual browser but is fragile for concurrent agents because a
shared fixed `127.0.0.1:9222` endpoint and shared profile can collide. The
reference runtime at `/nvme/development/pythia/ops/browser-cdp` provides a
Chrome-for-Testing container with Xvfb, fluxbox, x11vnc, noVNC, loopback CDP,
an internal CDP proxy, and health/readiness scripts.

## Scope

- Add `lantern browser start|list|status|endpoint|stop|prune`.
- Start disposable managed containers with unique instance ids, unique
  container names, isolated local profile directories, labels, and random
  host-loopback CDP ports.
- Store managed-instance records under
  `.smoogle/lantern/browser-instances/`.
- Keep CDP publication loopback-only by default.
- Provide concise human output and stable JSON output with resolved endpoints.
- Add deterministic tests for parsing, state behavior, and runtime command
  construction without requiring a real container daemon.
- Document build/start/list/stop/concurrent-agent/security workflows and a
  manual smoke path.

## Non-Goals

- No implicit browser startup from `doctor`, `page`, `flow`, or other CDP
  commands.
- No public `0.0.0.0` CDP binding by default.
- No shared default Chromium profile between managed instances.
- No authenticated persistent profile as the default.
- No broad browser automation, arbitrary JavaScript evaluation, or daemon mode.

## Implementation Notes

- Put registry and runtime command construction in `lantern-storage` so the CLI
  stays mostly orchestration and output formatting.
- Prefer a runtime adapter shape that can support podman and docker. The first
  pass may default to podman while keeping command construction explicit and
  testable.
- Use atomic writes for JSON state files and a simple local lock directory for
  state mutations.
- Reconcile record status from container labels/inspection where practical, but
  keep normal validation independent of a running runtime.

## Progress

- 2026-04-23: Created active ExecPlan before implementation.
- 2026-04-23: Added JSON-file registry and runtime command construction in
  `lantern-storage`, including podman/docker command shapes, loopback random
  port publishing, labels, and isolated profile mounts.
- 2026-04-23: Added `lantern browser start|list|status|endpoint|stop|prune`
  CLI path. Existing endpoint commands still bypass lifecycle logic.
- 2026-04-23: Added in-repo Chrome-for-Testing/Xvfb/fluxbox/x11vnc/noVNC CDP
  container scaffold under `ops/browser-cdp`.
- 2026-04-23: Updated CLI, setup, workflow, security, and reliability docs for
  explicit managed lifecycle behavior.
- 2026-04-23: Second pass fixed CLI contract drift so endpoint commands reject
  lifecycle-only flags such as `--runtime` and `--id` with usage exit code `2`
  instead of accepting inert browser lifecycle options.
- 2026-04-23: Rootless Podman smoke found profile mount ownership drift with
  `:U`; replaced it with Podman `--userns=keep-id:uid=1000,gid=1000` plus
  `:Z`, preserving host-writable disposable profiles while allowing Chrome to
  write `/profile`.
- 2026-04-23: Real rootless Podman smoke passed after the ownership fix:
  built `localhost/lantern-browser-cdp:stable`, started `agent1` and `agent2`
  concurrently, verified both resolved endpoints with `lantern doctor`, then
  stopped and pruned both instances.
- 2026-04-23: Docker smoke initially exposed two Docker-specific runtime
  issues: Chromium needed explicit `--no-sandbox`, and the bind-mounted profile
  directory needed the container process to run as the host profile owner with
  `HOME=/tmp`. Added Docker-only runtime args for those cases.
- 2026-04-23: Real Docker smoke passed with Docker 29.4.0 after the runtime
  fixes: built `localhost/lantern-browser-cdp:stable`, started `docker-agent1`
  and `docker-agent2` concurrently, verified both resolved endpoints with
  `lantern doctor`, then stopped and pruned both instances.

## Manual Smoke Evidence

Executed on 2026-04-23 with Podman 5.8.2:

```sh
podman build -f ops/browser-cdp/Containerfile -t localhost/lantern-browser-cdp:stable ops/browser-cdp
target/debug/lantern browser start --id agent1 --json
target/debug/lantern browser start --id agent2 --json
target/debug/lantern browser list --json
target/debug/lantern doctor --endpoint "$(target/debug/lantern browser endpoint agent1 --json | jq -r .instance.endpoint)" --json
target/debug/lantern doctor --endpoint "$(target/debug/lantern browser endpoint agent2 --json | jq -r .instance.endpoint)" --json
podman ps --filter label=dev.lantern.managed=true --format '{{.Names}} {{.Status}} {{.Ports}}'
target/debug/lantern browser stop agent1
target/debug/lantern browser stop agent2
target/debug/lantern browser prune
target/debug/lantern browser list --json
```

Outcome: both instances started and reported separate loopback CDP endpoints
(`agent1` used `http://127.0.0.1:37291`; `agent2` used
`http://127.0.0.1:43287` in this run). `lantern doctor` returned
`ok=true`, `Chrome/147.0.7727.117`, protocol `1.3`, and
`websocket.available=true` for both endpoints. Cleanup left
`browser list` empty and no managed instance records.

Executed on 2026-04-23 with Docker 29.4.0 through `/usr/bin/sg docker`:

```sh
/usr/bin/sg docker -c 'docker --version && docker info --format "ServerVersion={{.ServerVersion}} Rootless={{.SecurityOptions}}"'
/usr/bin/sg docker -c 'docker build -f ops/browser-cdp/Containerfile -t localhost/lantern-browser-cdp:stable ops/browser-cdp'
/usr/bin/sg docker -c 'target/debug/lantern browser start --runtime docker --id docker-agent1 --json'
/usr/bin/sg docker -c 'target/debug/lantern browser start --runtime docker --id docker-agent2 --json'
/usr/bin/sg docker -c 'target/debug/lantern browser list --json'
/usr/bin/sg docker -c 'docker ps --filter label=dev.lantern.managed=true --format "{{.Names}} {{.Status}} {{.Ports}}"'
/usr/bin/sg docker -c 'target/debug/lantern doctor --endpoint "$(target/debug/lantern browser endpoint docker-agent1 --json | jq -r .instance.endpoint)" --json'
/usr/bin/sg docker -c 'target/debug/lantern doctor --endpoint "$(target/debug/lantern browser endpoint docker-agent2 --json | jq -r .instance.endpoint)" --json'
/usr/bin/sg docker -c 'target/debug/lantern browser stop docker-agent1 --json && target/debug/lantern browser stop docker-agent2 --json && target/debug/lantern browser prune --json && target/debug/lantern browser list --json'
/usr/bin/sg docker -c 'docker ps -a --filter label=dev.lantern.managed=true --format "{{.Names}} {{.Status}} {{.Ports}}"'
```

Outcome: Docker was reachable (`Docker version 29.4.0`, server `29.4.0`;
security options `name=seccomp,profile=builtin name=cgroupns`). Both instances
started and reported separate loopback CDP endpoints: `docker-agent1` used
`http://127.0.0.1:32781`, and `docker-agent2` used
`http://127.0.0.1:32784`. Docker showed both containers healthy with loopback
port mappings: `docker-agent1` published `32781->9222/tcp`,
`32779->5900/tcp`, and `32780->6080/tcp`; `docker-agent2` published
`32784->9222/tcp`, `32782->5900/tcp`, and `32783->6080/tcp`.
`lantern doctor` returned `ok=true`, `Chrome/147.0.7727.117`, protocol `1.3`,
and `websocket.available=true` for both endpoints. Cleanup stopped and pruned
both records, `browser list --json` returned `instances: []`, and a final
Docker label query returned no managed containers.

## Validation Plan

- `cargo test -p lantern-storage`
- focused CLI parser/integration tests for lifecycle flag rejection
- `scripts/validate.sh fast` during iteration
- `scripts/validate.sh` before completion if feasible
- `smoogle docs check` after docs and plan edits

## Deferred Work

- Add automated real-runtime smoke coverage once the validation environment can
  reliably provide podman or docker and image build time is acceptable.
