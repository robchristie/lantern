# Browser sessions

Read this reference only when inspection needs Lantern to own the browser
lifecycle, reach a host service from a container, or reuse a dedicated
authenticated profile.

## Resolve and verify Lantern

Use plain `lantern` first. In a Smoogle child run, a run-local shim is injected
when Lantern is discoverable. If needed, check `$LANTERN_BIN`,
`/usr/local/bin/lantern`, `/nvme/development/lantern/target/release/lantern`, then
`/nvme/development/lantern/target/debug/lantern`. Record the selected path and
`lantern capabilities --json`; local builds can share a package version while
having different capabilities.

## Disposable managed browser

Build `ops/browser-cdp/Containerfile` first when the browser image is absent.
Keep the instance id, install a cleanup trap, and pass the returned endpoint to
every inspection command:

```bash
LANTERN="${LANTERN_BIN:-lantern}"
ID="$("$LANTERN" browser start --json | jq -r .instance.id)"
cleanup_lantern_browser() {
  if [ -n "${ID:-}" ]; then
    "$LANTERN" browser stop "$ID" --json >/dev/null 2>&1 || true
    "$LANTERN" browser prune --json >/dev/null 2>&1 || true
  fi
}
trap cleanup_lantern_browser EXIT
ENDPOINT="$("$LANTERN" browser endpoint "$ID" --json | jq -r .instance.endpoint)"
"$LANTERN" doctor --endpoint "$ENDPOINT" --json
```

When the browser is containerised, use a container-reachable application URL,
such as `http://host.docker.internal:<port>/`. `--host-gateway <HOST>` maps one
validated hostname to the runtime host gateway; it is neither a URL nor a
general DNS override.

Explicitly stop and prune before finishing, even when the trap should run:

```bash
"$LANTERN" browser stop "$ID" --json
"$LANTERN" browser prune --json
```

`browser prune` removes stopped, missing or errored instances recorded in this
repository's `.smoogle/` state. It does not make unrelated runtime containers
part of Lantern's ownership.

## Dedicated authenticated profile

Follow `docs/authenticated-browser-testing.md` in the Lantern repository before
connecting to authenticated state. Reuse an operator-approved, dedicated named
profile; never use a daily personal browser profile. The first start may require
visible login through the returned noVNC URL.

```bash
LANTERN="${LANTERN_BIN:-lantern}"
PROFILE=geometis-review
"$LANTERN" browser profile status "$PROFILE" --json >/dev/null 2>&1 || \
  "$LANTERN" browser profile create "$PROFILE" --json

ID="$("$LANTERN" browser start \
  --profile "$PROFILE" \
  --host-gateway lv426.yutani.tech \
  --json | jq -r .instance.id)"
ENDPOINT="$("$LANTERN" browser endpoint "$ID" --json | jq -r .instance.endpoint)"
"$LANTERN" doctor --endpoint "$ENDPOINT" --json
```

The hostname above is the local Geometis route; replace it with the inspected
application's validated hostname when needed, on every start.

Stop and prune the instance after inspection, but preserve the named profile.
Profile deletion requires explicit operator approval and
`browser profile delete "$PROFILE" --yes`. Log out visibly first when
server-side revocation matters.

Treat profile state and authenticated pixels as sensitive. Keep CDP on loopback,
avoid `--no-redact`, do not inspect or export cookie/storage databases, and do
not include profile material in Git or support bundles.
