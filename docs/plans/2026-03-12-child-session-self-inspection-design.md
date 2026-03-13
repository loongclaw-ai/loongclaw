# Child Session Self-Inspection Design

## Context

LoongClaw now supports synchronous nested delegation with lineage-based `max_depth` enforcement. Delegated child sessions can run useful work, but their current tool surface has a gap: they cannot inspect their own session state or transcript through the app-layer session tools.

That makes delegated subtasks less self-aware than root sessions even though the system already records:

- session state
- session events
- session transcript history

## Problem

We want delegated child sessions to be able to inspect themselves without widening cross-session visibility.

The subtle constraint is that child sessions may now have descendants when `max_depth > 1`. If we simply expose `session_status` and `sessions_history` while reusing the global `tools.sessions.visibility = "children"` policy, then a child session would gain access to its own descendants. That is broader than the intended "self-inspection only" behavior.

## Goals

- Let delegated child sessions call `session_status` for themselves.
- Let delegated child sessions call `sessions_history` for themselves.
- Keep `sessions_list` hidden from delegated child sessions.
- Prevent delegated child sessions from inspecting parent or descendant sessions.
- Avoid introducing a new config surface unless clearly necessary.

## Non-Goals

- No broad session tree browsing inside child sessions.
- No `sessions_list` in delegated child tool views.
- No new persisted policy fields or schema changes.
- No change to root-session visibility semantics.

## Options Considered

### Option 1: Expose child self-inspection tools and reuse global visibility

Pros:

- smallest code diff

Cons:

- incorrect semantics once child sessions can also delegate
- child would inherit descendant visibility via the existing `"children"` policy

Rejected.

### Option 2: Expose child self-inspection tools and force a child-local self-only override

Pros:

- matches intended behavior exactly
- keeps existing root semantics untouched
- does not require new config

Cons:

- needs one more policy-derivation layer at app-tool dispatch time

Chosen.

### Option 3: Add a separate config surface for child session inspection policy

Pros:

- most configurable

Cons:

- overfits the current phase
- adds config complexity before the simpler model is proven useful

Rejected for now.

## Chosen Approach

Delegated child sessions will advertise:

- `session_status`
- `sessions_history`

They will continue to hide:

- `sessions_list`
- all broader session-browsing behavior

At execution time, app-tool dispatch will derive an effective tool policy from `SessionContext`:

- root sessions use configured `tools.sessions.visibility`
- child sessions force session-tool visibility to `self`

This separation keeps the provider-visible tool surface truthful while ensuring execution policy remains narrower for delegated children.

## Implementation Shape

### Tool View Layer

Extend delegate child tool-view construction so child sessions can see:

- configured runtime child core tools
- optional `delegate` when remaining depth allows it
- `session_status`
- `sessions_history`

`sessions_list` remains excluded.

### App Tool Policy Layer

Introduce a small helper that derives an effective `ToolConfig` for the current `SessionContext`.

For child sessions:

- clone the current tool config
- override `tools.sessions.visibility` to `self`

This policy should be applied by the default app dispatcher before calling `execute_app_tool_with_config(...)`.

### Session Tool Layer

No broad semantic change is needed inside `tools/session.rs`; it already understands `SelfOnly` and `Children`. The new work is to ensure child sessions arrive with the correct effective policy.

## Behavior

Expected behavior after this change:

- child session can call `session_status` on itself
- child session can call `sessions_history` on itself
- child session cannot call `sessions_list` because the tool is not visible
- child session cannot inspect a descendant session even if it created one via nested delegation
- child session cannot inspect its parent session

## Testing Strategy

Add TDD coverage for:

1. child tool view includes `session_status` and `sessions_history`
2. child tool view still excludes `sessions_list`
3. child session can successfully read its own status
4. child session can successfully read its own history
5. child session gets `tool_not_visible: sessions_list`
6. child session gets `visibility_denied` when trying to inspect a descendant session
7. resumed child sessions get the same self-inspection tool view from the default runtime

## Documentation Impact

Update the product spec to clarify that delegated child sessions have self-inspection only, not session-tree browsing.
