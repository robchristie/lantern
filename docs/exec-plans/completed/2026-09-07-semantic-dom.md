# Computed accessibility and semantic DOM targets

Status: completed

## Outcome and boundaries

Package A adds a bounded browser-computed accessibility view and exact role/name
or data-testid targets to the existing guarded interaction engine, including
observed click flows. CSS and wait contracts remain compatible. Ordinary main
 document elements only; no frame/shadow traversal or retained snapshot references.
The programme conductor owns review, publication and landing.

## Calibration

Question: can Chromium's computed names resolve through backend DOM identity to
ordinary elements without bypassing actionability or retaining stale references?
Smallest probe: labelled and aria-labelledby input, duplicate named buttons,
exact test ID, disabled/occluded controls and replacement; query an unavailable
Accessibility method to establish explicit protocol failure. Evidence owner:
this plan and `.smoogle/qualification-tools/semantic-dom-calibration.json`.
Exit: names and backend resolution agree with fixture identity, replacement
changes backend identity, ignored/non-element/shadow/frame nodes can be filtered,
and unsupported commands fail explicitly. Then select one candidate and extend
the existing audited Chromium contracts before canonical validation.

## Acceptance and current state

Candidate selected and locally qualified: canonical verification passes (231
tests, current compiler and MSRV, documentation), the tracked skill validates,
and 64 audited real-Chromium cases pass (44 inherited, 20 added). Exact build and
fixture identities are recorded by the browser harness. Final clean-head evidence
location: `.smoogle/semantic-dom-clean/evidence.json`; terminal landing evidence is recorded below.
Reuse the unique target,
two-sample actionability, absolute deadline, cleanup and no-replay engine.
Computed names are browser outputs; never recompute ARIA naming in JavaScript.
Output excludes AX values, raw properties and ignored reasons; names are bounded
and use existing text redaction. Semantic request metadata must be redacted.

Protocol authority: [CDP Accessibility](https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/)
(checked 7 September 2026): queryAXTree includes ignored nodes and returns
backendDOMNodeId; resolve it using DOM.resolveNode, never a numeric frontend join.

## Calibration decision and evidence

Retain the backend-to-wrapper route. At source
`c61fe1169538444feae0f40a7ccd7baed2f3a93a`, the minimal real-browser probe
returned one labelled textbox, two duplicate named buttons, the intended ordinary
Element for aria-labelledby, a changed backend identity after replacement and a
disconnected old wrapper. An unavailable Accessibility method returned -32601.
The disposable-probe assertions passed; its profile cleanup initially raced
Chromium shutdown, with no effect on observations. The canonical browser harness
owns and completes browser cleanup for qualification.
Detailed local calibration: `.smoogle/qualification-tools/semantic-dom-calibration.json`;
probe: `.smoogle/qualification-tools/calibrate-ax.py`. Retained implementation proof
is executable in `semantic.rs`, `interaction.rs` and the audited browser harness.
Selected-candidate development evidence: `.smoogle/semantic-dom-selected/evidence.json`; the
representative disabled and occluded semantic controls returned their existing
blockers with zero Input, while replacement typing succeeded with one insertText.

## Terminal delivery

Landed as [PR #14](https://github.com/robchristie/lantern/pull/14#issuecomment-5565904470)
at `bb9fdd6cd868571ab99a9d77fe83ff7834f0321d`. Reviewed head
`d99cd4a7524d073e0c0144a5cc743b0756d5e62a` and merge share tree
`3afb194c94c61b2300b817d9d821d3a609beed97`. Canonical 231 tests, 64 audited
browser cases, PR CI 34089430775 and post-merge CI 34089762431 passed.
Installed skill reconciliation and task Git cleanup completed. Programme Q/B
remains separate from this completed outcome and the persistent-profile plan.

Calibration probe SHA-256:
`5dea1435c6e41bebecfb43847c864af45c203818e5fb6036c4299a1681cf88a5`.
Calibration result SHA-256:
`a2c6a928d7797625694f9bcd5a6ea38212c049cbd33c25711634985e02320638`.
