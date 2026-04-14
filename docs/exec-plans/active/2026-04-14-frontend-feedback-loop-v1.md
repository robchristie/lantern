# ExecPlan: Frontend Feedback Loop v1

Status: active

## Progress Notes

- 2026-04-14: seeded this plan as the next product sequence after the bootstrap, page summary, and DOM summary work. No product code has been changed for this plan yet.
- 2026-04-14: implemented Slice 1 explicit target selection with `--target-id` for `page` and `dom`, exact CDP page target id matching, stable not-found errors for non-page or missing ids, updated CLI contract coverage, and fixture tests for explicit selection.
- 2026-04-14: implemented Slice 2 navigation/open with `lantern open <URL>`, exact selected page target reuse, supported `http`, `https`, and `about:blank` URLs, bounded CDP `Page.navigate` plus page-state reads, redacted JSON output, and fixture coverage for success, `--no-redact`, invalid URLs, and navigation failure.
- 2026-04-14: implemented Slice 3 console feedback with `lantern console`, exact selected page target reuse, bounded Runtime/Log event collection, redacted and truncated console/error messages, explicit truncation metadata, and fixture coverage for JSON, human output, and missing WebSocket errors.
- 2026-04-14: implemented Slice 4 wait conditions with `lantern wait <ready|url|selector|text|quiet>`, explicit bounded `--timeout-ms`, selected target identity, elapsed time, matched/timed-out condition results, concise observed state, and fixture coverage for readiness, URL shape, selector presence, selector text, quiet-period waits, and invalid timeout bounds.
- 2026-04-14: implemented Slice 5 screenshot capture with `lantern screenshot --output <PATH>`, explicit overwrite opt-in, current visible viewport capture through CDP, best-effort viewport dimensions, metadata-only transcript output, redaction caveats, and fixture coverage for JSON, human output, overwrite behavior, and missing WebSocket errors.
- 2026-04-14: implemented Slice 6 network failures with `lantern network`, bounded CDP Network event collection for failed requests and HTTP error responses, redacted URL shapes, no body/header capture, explicit observed-clean and collection-gap metadata, and fixture coverage for JSON, human output, clean windows, and missing WebSocket errors.

## Goal

Build a small agent-native frontend feedback loop on top of the existing local Chromium CDP CLI.

The loop should let an operator or coding agent choose a target, open or navigate a page, wait for useful browser state, collect bounded feedback, and perform limited interactions without turning Lantern into a broad DevTools replacement.

## Why This Matters

Lantern already proves local CDP connectivity and read-only inspection through `doctor`, `targets`, `page`, and `dom`. The next useful product step is a narrow feedback loop for frontend development: open the work-in-progress page, observe failures, capture page state, make small UI interactions, and repeat with transcript-friendly output.

Keeping this plan slice-based prevents later tasks from mixing target selection, navigation, console, network, screenshots, waits, and interactions into one large ambiguous implementation.

## Constraints

- keep Chromium lifecycle operator-owned
- keep endpoint resolution unchanged unless a later slice explicitly updates the contract
- keep all browser access local to the configured Chromium CDP endpoint
- preserve redacted, bounded output by default
- prefer stable JSON objects and concise human output for every new command
- avoid persistent browser artifacts unless a later plan defines storage, cleanup, and redaction rules
- update `docs/product-specs/cli-contract.md` before implementing each new command surface
- do not imply unimplemented commands are available until their slice lands

## Non-Goals

- launching or managing Chromium
- replacing DevTools
- open-ended JavaScript evaluation
- full console log streaming
- full network capture or HAR generation
- screenshot retention, diffing, or gallery management
- broad form automation, arbitrary script execution, crawling, or test-runner behavior
- daemon mode, MCP server, TUI, web UI, or cross-browser support

## Product Direction

The v1 loop should grow from explicit selection and page opening into bounded feedback commands. Each command should answer one practical agent question:

- Which page target am I operating on?
- Can Lantern open the URL I need?
- Did the page reach the expected state?
- Are there console errors or page errors?
- Did important network requests fail?
- What does the page look like?
- Can Lantern perform one small user-like interaction and report the result?

The current implemented `page` and `dom` commands remain part of the read side of the loop. This plan should only add the missing control and feedback pieces needed for a minimal frontend development cycle.

## Implementation Slices

### Slice 1: Explicit Target Selection

Status: completed

- design a stable target selector shared by `page`, `dom`, and later feedback-loop commands
- support selecting a page target by exact CDP target id
- preserve current automatic selection behavior when no selector is supplied
- fail clearly when a selector does not match a page target
- update the CLI contract and fixture tests before wiring behavior through commands

### Slice 2: Navigation And Open

Status: completed

- add a bounded command surface for opening or navigating the selected page target to an operator-supplied URL
- keep Chromium launch and browser lifecycle out of scope
- require explicit local endpoint configuration as existing commands do
- return selected target identity, requested URL shape, final URL shape when available, and loading state
- document allowed URL schemes, error cases, and redaction behavior before implementation

### Slice 3: Console And Page Errors

Status: completed

- collect recent console errors and page runtime exceptions from the selected page target
- keep output bounded by count and text length
- default to redacting URLs and sensitive-looking values in messages
- avoid long-running streaming mode in v1
- report whether truncation occurred

### Slice 4: Wait Conditions

Status: completed

- add explicit wait conditions useful after navigation or interaction
- cover document readiness, URL shape match, selector presence, selector text, and quiet-period waits where practical
- make timeouts explicit and bounded
- return the condition result, elapsed time, selected target identity, and any concise observed state

### Slice 5: Screenshot Capture

Status: completed

- capture a screenshot of the selected page target using CDP
- define the artifact path, overwrite behavior, viewport assumptions, and cleanup expectations before implementation
- keep transcript output short: target identity, dimensions when known, file path, and redaction caveats
- avoid visual diffing or screenshot history in v1

### Slice 6: Network Failures

Status: completed

- report failed network requests and HTTP error responses from the selected page target
- bound output by count, URL shape, status, method, resource type, and failure reason
- avoid full request or response bodies
- distinguish collection gaps from an observed clean result

### Slice 7: Limited Interaction Commands

Status: pending

- add a small set of user-like interactions for selected elements
- start with click and text entry if selector handling is stable
- require explicit selectors and bounded timeouts
- return whether the action was dispatched, concise post-action state, and any immediate errors
- avoid arbitrary JavaScript evaluation, complex gestures, crawling, or test-runner abstractions

## Validation

- run `scripts/validate.sh fast` during docs and implementation slices
- run `scripts/validate.sh` before landing Rust behavior changes
- run `smoogle docs check` after ExecPlan, CLI contract, prompt, or core docs changes
- add deterministic local CDP fixture coverage for each command before marking its slice complete

## Acceptance Criteria

- each implemented command is documented before or with the code that exposes it
- target selection is explicit enough that agents can avoid ambiguous multi-page sessions
- navigation/open, waits, console errors, screenshots, network failures, and limited interactions remain bounded and local
- default output remains redacted and concise enough for agent transcripts
- JSON output uses stable field ordering and clear nullable fields
- errors preserve the existing CLI posture: actionable messages, stable codes, and appropriate exit codes
- no slice claims implemented functionality before it lands

## Open Questions

- Should the selector flag be named `--target`, `--target-id`, or another term that avoids confusion with CSS selectors?
- Should navigation create a new page target when none is selected, or should v1 only navigate existing page targets?
- What screenshot path convention best fits local task worktrees without introducing durable artifact management too early?
- Which wait conditions are essential for the first navigation slice, and which should stay separate until after screenshots and network failures land?
