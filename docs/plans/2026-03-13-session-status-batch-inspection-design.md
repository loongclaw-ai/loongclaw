# Session Status Batch Inspection Design

## Context

LoongClaw's operator surface now supports:

- filtered `sessions_list` discovery
- single-session `session_status`
- batch `session_cancel` / `session_recover` with `session_ids`

That still leaves one practical inspection gap. After discovery returns several relevant sessions,
operators still need to call `session_status` one target at a time to get the richer inspection
payload that includes terminal outcome state, delegate lifecycle, recovery metadata, and recent
events.

## Problem

We need a thicker inspection slice without adding a new top-level tool and without widening session
authority.

The design question is how to let operators inspect multiple visible sessions through the existing
`session_status` surface while preserving:

- backward compatibility for current single-session callers
- visibility boundaries per target
- clear per-target results when some requested sessions are not visible

## Goals

- Extend `session_status` to support batch targeting via `session_ids`.
- Preserve the legacy single-session response shape for `session_id`.
- Return machine-readable per-target inspection results in batch mode.
- Keep visibility checks per target and avoid leaking hidden-session details.
- Reuse the existing `session_inspection_payload` rather than inventing another inspection schema.

## Non-Goals

- No new top-level tool such as `session_status_many`.
- No mutation behavior, `dry_run`, or operator remediation in this slice.
- No event streaming or wait semantics for multiple sessions.
- No pagination, saved queries, or push updates.
- No broadening of delegated child authority.

## Options Considered

### Option 1: Keep `session_status` single-target and rely on caller loops

Pros:

- zero schema change
- no implementation risk

Cons:

- pushes repetitive inspection orchestration onto every caller
- makes the control surface uneven after batch remediation already exists
- discourages richer inspection after `sessions_list`

Rejected.

### Option 2: Extend `session_status` with `session_ids`

Accept either:

- `session_id`
- `session_ids`

Preserve the old single-target response shape and use an aggregated response only for batch calls.

Pros:

- keeps the tool surface compact
- composes naturally with `sessions_list`
- reuses the existing inspection payload
- mirrors the compatibility pattern already used by batch remediation

Cons:

- introduces a second response shape
- requires per-target visibility normalization and result classification

Chosen.

### Option 3: Add a new batch inspection tool

Examples:

- `session_status_many`
- `sessions_inspect`

Pros:

- explicit name
- no dual response shape on `session_status`

Cons:

- expands the top-level tool surface
- overlaps heavily with an existing tool whose meaning already matches the need

Rejected.

## Chosen Approach

Extend `session_status` rather than adding a new tool.

### Request model

`session_status` accepts exactly one of:

- `session_id: string`
- `session_ids: string[]`

### Backward compatibility

When the request uses `session_id`, behavior stays unchanged:

- status remains `ok`
- payload remains the existing `session_inspection_payload`

When the request uses `session_ids`, the response becomes aggregated.

### Batch response shape

Batch inspection returns:

- `tool`
- `current_session_id`
- `requested_count`
- `result_counts`
- `results`

Each `results[]` item includes:

- `session_id`
- `result`
- `message`
- `inspection`

Result values are intentionally narrow:

- `ok`
- `skipped_not_visible`

Visible targets get the full `session_inspection_payload`. Hidden or non-resolvable targets return
`inspection = null` plus a visibility-denied message.

### Visibility semantics

Visibility remains enforced per target using the same repository-backed checks as the current
single-target path.

Batch inspection must not hard-fail the whole request just because one requested target is hidden.
Instead:

- visible targets are inspected normally
- hidden targets become `skipped_not_visible`

Malformed input still hard-fails the request.

### Ordering semantics

Return `results[]` in the same order as the requested `session_ids`.

This keeps the response easy for callers to correlate with the input set and avoids adding extra
sorting semantics.

## Data Model

No schema change is required.

The slice reuses:

- `sessions`
- `session_events`
- `session_terminal_outcomes`

## Testing

Add focused coverage for:

- provider schema updates exposing `session_ids` on `session_status`
- batch `session_status` with mixed visible and hidden targets
- preservation of the legacy single-target `session_status` response shape
