# Session Observability Design

## Context

LoongClaw now has a working app-layer session surface:

- `sessions_list`
- `sessions_history`
- `session_status`
- synchronous `delegate`
- sqlite-backed `sessions` and `session_events`

That is enough to create and inspect delegate child sessions, but the observability model is still thin:

- operators can only read a coarse status snapshot
- terminal delegate results are not stored in a dedicated durable read model
- event inspection is only available indirectly through the fixed `recent_events` block inside `session_status`
- there is no bounded wait primitive for another session to finish

## Problem

We need the next thick slice to improve session observability without prematurely committing LoongClaw to a background worker architecture.

If we jump straight to `delegate_async`, we immediately inherit a larger runtime problem:

- which process owns the child execution after the parent returns
- what survives process restart
- how in-process task handles reconcile with sqlite state
- how channels and CLI sessions discover orphaned or detached work

That is solvable, but it is not the smallest causal next step.

## Goals

- Add a durable terminal outcome read model for delegate child sessions.
- Add an explicit `session_events` tool for polling session event history.
- Add an explicit `session_wait` tool for bounded waiting on visible sessions.
- Keep root-session visibility semantics unchanged.
- Keep delegated-child visibility semantics self-only for session tools.
- Reuse existing sqlite session infrastructure rather than introducing a worker runtime.

## Non-Goals

- No background delegate workers.
- No `delegate_async`, `session_cancel`, or push subscription transport.
- No cross-process task registry beyond sqlite session state.
- No historical backfill for legacy sessions that only exist in `turns`.

## Options Considered

### Option 1: Event log only

Expose `session_events` and derive terminal result from the latest event payload.

Pros:

- smallest schema change
- reuses the existing event table

Cons:

- terminal result remains implicit rather than a stable read model
- callers have to infer semantics from event ordering and payload shape
- future `session_wait` would still be coupled to event inspection heuristics

Rejected.

### Option 2: Durable terminal outcome + polling tools

Add a `session_terminal_outcomes` table, persist delegate terminal payloads there, expose:

- `session_events`
- `session_wait`
- `terminal_outcome` inside `session_status`

Pros:

- gives session tooling a real durable result model
- keeps current synchronous `delegate` semantics unchanged
- provides the exact primitives needed before any async delegate work
- composes cleanly with the current sqlite session repository

Cons:

- adds one more read model and one async polling code path

Chosen.

### Option 3: Full async delegate now

Introduce `delegate_async`, wait handles, and in-process task ownership immediately.

Pros:

- closer to the richer OpenClaw orchestration model
- more obviously future-facing

Cons:

- couples this phase to worker lifecycle design
- leaves restart/orphan semantics under-specified
- much larger execution surface than the current runtime actually supports

Rejected for now.

## Chosen Approach

Implement an observability-first layer on top of the existing synchronous delegate model:

1. Persist terminal outcomes for delegate child sessions in a dedicated sqlite table.
2. Expose `session_events` as a root-visible session tool for bounded event polling.
3. Expose `session_wait` as a bounded async app tool that waits until:
   - the target session becomes terminal, or
   - the wait timeout expires.
4. Extend `session_status` to surface `terminal_outcome` when present.

This gives LoongClaw a stable read model for eventual async delegation without forcing async execution ownership into this slice.

## Tool Surface

### Root Sessions

Root sessions will advertise:

- `sessions_list`
- `sessions_history`
- `session_status`
- `session_events`
- `session_wait`
- `delegate`

### Delegated Child Sessions

Delegated child sessions will continue to advertise:

- `session_status`
- `sessions_history`

They will continue to hide:

- `sessions_list`
- `session_events`
- `session_wait`

This keeps child self-inspection narrow and avoids broadening their orchestration authority before async delegation exists.

## Data Model

Add a new sqlite table:

- `session_terminal_outcomes`

Suggested columns:

- `session_id TEXT PRIMARY KEY`
- `status TEXT NOT NULL`
- `payload_json TEXT NOT NULL`
- `recorded_at INTEGER NOT NULL`

This table stores the terminal `ToolCoreOutcome` shape for a completed delegate child session:

- success payload
- timeout payload
- error payload

The session row remains the source of truth for lifecycle state (`ready`, `running`, `completed`, `failed`, `timed_out`).
The terminal outcome table becomes the source of truth for terminal result payloads.

## Execution Model

### Delegate Completion Persistence

When synchronous `delegate` exits:

- success: mark child `completed`, append `delegate_completed`, persist terminal outcome
- failure: mark child `failed`, append `delegate_failed`, persist terminal outcome
- timeout: mark child `timed_out`, append `delegate_timed_out`, persist terminal outcome

### `session_status`

Keep the existing snapshot payload, but add:

- `terminal_outcome`

If no terminal outcome exists, return `null`.

### `session_events`

Add direct event inspection with:

- required `session_id`
- optional `limit`
- optional `after_id`

Semantics:

- if `after_id` is present, return events with `id > after_id` in ascending order up to `limit`
- otherwise return the most recent `limit` events in ascending order

### `session_wait`

`session_wait` will be async at the app-dispatch layer, not inside the synchronous sqlite helper.

Inputs:

- required `session_id`
- optional `timeout_ms`

Behavior:

- validate visibility first
- repeatedly read current session summary + terminal outcome
- stop early when the target session reaches a terminal state
- otherwise return a timeout snapshot after the deadline

Return shape:

- `wait_status`: `completed` or `timeout`
- `session`
- `terminal_outcome`
- `recent_events`

Tool outcome status remains:

- `ok` when terminal state was observed before the deadline
- `timeout` when the deadline expires first

## Visibility Policy

Session visibility rules stay unchanged:

- root sessions use configured `tools.sessions.visibility`
- delegated child sessions force effective visibility to `self`

That means:

- roots can inspect visible descendants with `session_events`, `session_wait`, and `session_status`
- children cannot use the new tools because they are not in the child tool view

## Testing Strategy

Add TDD coverage for:

1. repository upsert/load of terminal outcomes
2. `session_status` includes `terminal_outcome` when present
3. `session_events` returns ordered event payloads and respects `after_id`
4. `session_wait` returns `ok` when a target session is already terminal
5. `session_wait` times out cleanly for a non-terminal session
6. synchronous `delegate` persists terminal outcome rows for success, failure, and timeout
7. runtime root tool view includes `session_events` and `session_wait`
8. delegated child tool view still excludes the new observability tools

## Documentation Impact

Update the product spec and roadmap to reflect:

- durable terminal delegate outcomes
- `session_events`
- `session_wait`
- continued absence of background async delegation
