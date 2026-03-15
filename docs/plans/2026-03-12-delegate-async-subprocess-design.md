# Delegate Async Subprocess Design

## Context

LoongClaw now has the following session/delegation primitives:

- synchronous `delegate`
- lineage-bounded nested delegation
- delegated-child self-inspection
- durable terminal outcomes
- `session_events`
- `session_wait`

That gives the system a stable polling and observability layer, but it still lacks non-blocking delegation. Parent sessions must wait for child execution inline inside the same turn loop.

## Problem

We want a real `delegate_async` capability without pretending LoongClaw already has a durable worker queue or long-lived task registry.

The critical architectural question is how child execution continues after the parent tool call returns.

## Goals

- Add a non-blocking `delegate_async` tool.
- Reuse `child_session_id` as the wait handle.
- Reuse existing `session_wait`, `session_status`, and `session_events` instead of inventing a second observability plane.
- Keep nested delegation bounded by existing lineage-based `max_depth`.
- Preserve child tool-view restrictions.

## Non-Goals

- No durable task queue.
- No cancellation API in this phase.
- No websocket/push event stream.
- No cross-host or post-reboot recovery guarantees.

## Options Considered

### Option 1: In-process Tokio task spawn

Pros:

- smallest apparent runtime diff
- avoids subprocess creation overhead

Cons:

- parent-side code only has borrowed runtime references in many paths
- hard to detach safely across turn-loop boundaries
- dies with the parent process
- difficult to test without threading borrowed `ConversationRuntime` through `'static` task ownership

Rejected.

### Option 2: Subprocess one-shot worker

Spawn the current daemon binary in a dedicated one-shot command that processes exactly one session turn and exits.

Pros:

- parent can return immediately after spawning
- child work no longer depends on borrowed runtime references from the parent turn loop
- child may outlive the parent process once the subprocess is launched
- reuses the existing sqlite-backed session store and session id handle

Cons:

- needs a stable single-turn CLI entrypoint
- requires config-path handoff into the child process
- adds process startup overhead

Chosen.

### Option 3: Durable queue/worker subsystem first

Pros:

- strongest eventual architecture
- best path to retries/cancel/resume

Cons:

- much larger than the current phase
- drags in queue ownership, leasing, and recovery semantics

Rejected for now.

## Chosen Approach

Add `delegate_async` as a sibling to synchronous `delegate`, backed by a subprocess worker.

### Parent-side behavior

When a session calls `delegate_async`:

1. validate payload and lineage depth
2. create child session row
3. append `delegate_queued`
4. spawn a subprocess worker
5. return immediately with:
   - `child_session_id`
   - `label`
   - `mode = "async"`
   - `state = "queued"`

### Worker-side behavior

The subprocess worker executes exactly one turn for the child session:

1. load config
2. mark child session `running`
3. append `delegate_started`
4. run the child turn with timeout
5. persist terminal outcome and final state
6. append `delegate_completed` / `delegate_failed` / `delegate_timed_out`
7. exit

## CLI Foundation

Add a new daemon command:

- `loongclaw run-turn`

Inputs:

- `--config` optional
- `--session` required
- `--input` required
- `--timeout-seconds` optional
- `--delegate-child` optional flag

When `--delegate-child` is set, the command wraps execution with delegate-child lifecycle state/event persistence rather than behaving like a generic one-shot turn command.

## Config Handoff

The subprocess worker needs the active config path.

Preferred source order:

1. explicit `--config`
2. inherited `LOONGCLAW_CONFIG_PATH`

Interactive and channel entrypoints should export `LOONGCLAW_CONFIG_PATH` once the resolved config file is known.

## Tool Surface

### Root sessions

Root sessions advertise:

- `delegate`
- `delegate_async`
- `sessions_list`
- `sessions_history`
- `session_status`
- `session_events`
- `session_wait`

### Delegated child sessions

When remaining depth allows further delegation, child sessions advertise:

- `delegate`
- `delegate_async`
- `session_status`
- `sessions_history`

They still do not advertise:

- `sessions_list`
- `session_events`
- `session_wait`

## State Model

### New async lifecycle events

Add:

- `delegate_queued`
- `delegate_spawn_failed`

Existing events remain:

- `delegate_started`
- `delegate_completed`
- `delegate_failed`
- `delegate_timed_out`

### Session states

Use existing session states only:

- `ready` while queued
- `running` once worker starts
- `completed` / `failed` / `timed_out` on terminal exit

No new state enum is needed in this phase.

## App Dispatcher Design

`delegate_async` should execute through `DefaultAppToolDispatcher`, not the synchronous direct tool helper.

Introduce a small async delegate spawner abstraction:

- production: subprocess spawner
- tests: fake spawner

This keeps app-layer tests deterministic without requiring the real daemon binary to be launched during unit tests.

## Testing Strategy

Add TDD coverage for:

1. tool catalog/provider schema includes `delegate_async`
2. root runtime tool view includes `delegate_async`
3. delegated child tool view includes `delegate_async` only when remaining depth allows
4. `delegate_async` returns immediate queued handle payload
5. spawn failure marks child session failed and records `delegate_spawn_failed`
6. fake async completion becomes observable via `session_wait`
7. `run-turn --delegate-child` lifecycle helper persists terminal outcomes for success/failure/timeout

## Documentation Impact

Update product spec and roadmap to reflect:

- async delegation is now available
- session id is the async handle
- waiting remains polling-based
- no cancellation and no durable queue yet
