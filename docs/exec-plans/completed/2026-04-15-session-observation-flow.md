# ExecPlan: Session Observation Flow

Status: completed

## Goal

Move Lantern beyond purely snapshot-oriented browser inspection by adding a bounded session command that keeps one CDP attachment open across navigation, waiting, and inspection.

## Context

Separate `open`, `wait`, `console`, and `network` invocations are useful ad hoc tools, but they can miss events that occur between attachments. This was especially visible for frontend-development review loops, where the agent needs to know whether a fresh page-load flow emitted console or network failures.

## Completed Slice

- Added `lantern flow --timeout-ms <MS> [--quiet-ms <MS>] [--open <URL>]`.
- Reused existing console, network, wait, target-selection, URL-redaction, and error-output contracts.
- Kept the implementation bounded: one command, one selected page target, one temporary CDP attachment, no daemon, no click/type/eval/session persistence.
- Made collection-gap semantics explicit: `--open` starts observation before the navigation performed by `flow`; attaching to an already-running page still reports a collection gap.
- Added fixture-backed CLI coverage for navigation, quiet wait, console event capture, network failure capture, redaction, and collection-gap semantics on one WebSocket attachment.

## Follow-Up

A later ExecPlan may add multi-step interaction sessions, but only after defining how `click`, `type`, and observation checkpoints compose without turning Lantern into a broad browser automation framework.
