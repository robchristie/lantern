# Functional actions

Read this reference for functional command recipes or the detailed
`action-flow` contract.

For initial load evidence:

```bash
lantern flow --endpoint "$ENDPOINT" --open "$URL" \
  --timeout-ms 5000 --quiet-ms 500 --json
lantern page --endpoint "$ENDPOINT" --json
lantern dom --endpoint "$ENDPOINT" --json
```

For one action and condition:

```bash
lantern action-flow --endpoint "$ENDPOINT" \
  --selector '#save' \
  --expect-selector '#status' --expect-text 'Saved' \
  --timeout-ms 3000 --strict --json
```

Pass exactly one postcondition form: `--expect-selector <CSS>`, that selector
plus `--expect-text <TEXT>`, or `--expect-url <URL>`. Add `--output <PNG>` and,
when intentionally replacing a file, `--overwrite` to retain a sequenced
visible-viewport capture. Region flags do not apply to `action-flow`.

Judge `interaction`, baseline and final `postcondition`, console, network,
capture and `verdict` separately. `ok: true` means the structured command
completed; it does not mean the application outcome passed. `--strict` makes a
standalone interaction require acknowledged dispatch and makes `action-flow`
require `verdict=passed`.

Use `click`, `type`, `key`, `hover`, `wheel` and `drag` only for explicit,
bounded checks. Pass the command's documented selector, input or pointer values
and `--timeout-ms`, add `--strict`, then inspect an explicit application
postcondition. Separate snapshot commands can miss fast events between
attachments; use `action-flow` when its single-click contract fits.

Never automatically replay input whose dispatch is uncertain or may have
partly executed. Inspect current state first. Preserve the distinction between
pre-input setup failure, not-dispatched input, uncertain dispatch, failed or
timed-out postcondition, observed runtime/network failure, incomplete evidence
and capture failure when reporting the result.

## Computed DOM semantics

For ordinary DOM controls, inspect `lantern accessibility --endpoint "$ENDPOINT"
--json` and use the browser-computed exact role/name or a literal test ID:

```bash
lantern click --role button --name 'Save' --timeout-ms 2000 --strict --endpoint "$ENDPOINT" --json
lantern type --test-id account --text-file /private/input --timeout-ms 2000 --strict --endpoint "$ENDPOINT" --json
```

Choose exactly one of CSS, role/name or test ID. These target strategies also
apply to key, pointer commands and the click in action-flow; wait and explicit
postconditions remain CSS-based. The accessibility list is bounded and filters
ignored/non-element, child-document and shadow content. Inspect `truncated` and
evidence loss; unsupported browser methods fail explicitly. No input values or
reusable node references appear. Duplicate semantic matches send no input;
missing matches can poll within the supplied timeout. The normal actionability,
strict outcome and no-replay rules still apply.
