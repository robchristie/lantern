# Polyorama evidence

Use Polyorama's current `docs/ui-snapshots/README.md` and
`docs/ui-guides/ui-review.md` as the evidence owners. Generate fixtures with its
`cargo xtask ui` tools into the owner's approved ignored output directory. A
checked-in `expected/` image illustrates its baseline; it is not a new live
capture or proof of current application readiness.

```sh
lantern polyorama --evidence-dir /absolute/path/to/fixture --json
lantern polyorama --endpoint http://127.0.0.1:9222 --target-id TARGET \
  --timeout-ms 10000 --output /absolute/path/live.png --json
```

The narrow adapter supports version 1 `metadata.json`, `semantic.json` and
`text.json`, with optional `visual.png`, and the gallery's fixed
`window.__POLYORAMA_GALLERY_HANDLE.snapshot()` hook. It always emits JSON.
It does not expose arbitrary JavaScript or call story/configuration/repaint/input
hooks. Use the owner harness to arrange a synthetic fixture; label that setup
separately from physical input. Existing Lantern input commands remain the
physical interaction path, with their ordinary outcome checks.

Inspect returned semantic roles, names, state, action identities and geometry,
text allocated/painted/clip rectangles, audit findings and coverage together.
`ok=true` means the bounded adapter contract was read and validated; it does not
mean the UI, text audit, pixels or design passed. Failed text attempts and audit
findings remain evidence. Missing coverage is unavailable, never a zero-count
pass. Counts include submitted clipped controls; ordinary labels and listed native
text categories remain excluded. Semantic nodes have no clipping rectangles:
use text clipping and opened pixels where needed.

Canonical bundles omit frame and source revision. Live observations supply the
application and semantic frame counters, story/configuration, bootstrap marker
and viewport. Neither route supplies build/application revision or outstanding
runtime work. Keep those fields unavailable and attach independently recorded
source/build provenance to your review; do not infer it from a directory name,
URL, ready body class or quiet interval.

Optional capture reads a snapshot, captures viewport pixels, then reads another
snapshot on the same bounded CDP connection. `same_observed_frame` means equal
bracketing counters; `differing` means they changed. The pixel frame and atomicity
remain unproved in both cases. A failed trailing read retains captured pixels
and reports the frame relationship unavailable. Compare PNG device dimensions
with the observed CSS viewport and device scale, and preserve mismatches.
Semantic/text geometry is in egui points with the owner's `pixels_per_point`.

Record artefact SHA-256 hashes, actual browser/render route and independently
known source/input identities. Open every relevant PNG and apply explicit visual
expectations. A successful PNG decode can still describe black pixels. Missing
hooks and oversized live snapshots produce `ok=false` with an unavailable reason
and a successful command exit. Malformed contracts and oversized local files
fail the command. Files are limited to 8 MiB, collections to 5000 items,
and decoded PNGs to 64 MiB. Owner names, descriptions, disabled reasons, layout
errors and domain references receive ordinary snippet redaction by default;
stable identifiers and hashes remain available for correlation. Screenshot pixels
are unredacted and are persisted only when explicitly requested.
