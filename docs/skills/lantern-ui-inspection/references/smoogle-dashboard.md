# Smoogle dashboard inspection

Read this reference only for the Smoogle web dashboard.

When a containerised browser must reach the dashboard, bind it explicitly for a
trusted local setup:

```bash
/nvme/development/smoogle/target/debug/smoogle-cli web serve \
  --host 0.0.0.0 --port 7879 --allow-non-loopback
```

Use the container-reachable URL and begin with one coherent navigation
observation:

```bash
URL=http://host.docker.internal:7879/
lantern flow --endpoint "$ENDPOINT" --open "$URL" \
  --timeout-ms 5000 --quiet-ms 500 --json
lantern page --endpoint "$ENDPOINT" --json
lantern dom --endpoint "$ENDPOINT" --json
lantern layout --endpoint "$ENDPOINT" \
  --container-selector '[data-layout-container]' --json
```

If the default DOM summary omits relevant shell content, retry with
`--depth 8 --max-nodes 220`. Use `console` and `network` after later changes or
interactions. For a visual or responsive task, capture and open the dashboard
at each relevant viewport using the expectations in the skill entrypoint.

Keep non-loopback dashboard binding explicit, local and trusted. Stop the
dashboard process and managed browser you started when inspection is complete.
