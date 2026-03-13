# Delegate Depth Lineage Design

## Context

LoongClaw already exposes a synchronous `delegate` app tool, session inspection tools, and config for `tools.delegate.max_depth`. The current implementation only enforces a shallow special case:

- root sessions can delegate once
- child sessions are hard-blocked when `max_depth <= 1`
- child tool views always exclude `delegate`

That means the current config surface is only partially truthful. `max_depth > 1` does not materially change runtime behavior, and nested delegate chains cannot be observed cleanly from the root session.

## Problem

We need to make delegate depth a real runtime policy rather than a placeholder config field, without expanding into background execution or schema migration work.

The current gaps are:

1. `tools.delegate.max_depth` does not permit true nested delegation.
2. Child tool visibility is hard-coded rather than derived from remaining depth.
3. Session visibility only covers the current session and direct children, which becomes too shallow once grandchild sessions exist.
4. The child tool allowlist default advertises planned tools that are not actually runtime-visible today.

## Goals

- Make `max_depth` semantically real for synchronous nested delegation.
- Keep the thick orchestration in the app-layer turn loop.
- Avoid SQLite schema changes or historical backfill.
- Let root sessions inspect the full descendant delegate chain through session tools.
- Keep session tools hidden from delegated children for this phase.

## Non-Goals

- No background delegate workers.
- No async wait handles or subscription APIs.
- No `sessions_*` exposure inside delegated child tool views.
- No migration that rewrites historical session rows.

## Chosen Approach

Use runtime lineage traversal over the existing `parent_session_id` chain.

### Depth Model

- Root session lineage depth is `0`.
- A direct child created by `delegate` has depth `1`.
- A grandchild has depth `2`, and so on.
- A new delegate call is allowed only when `current_depth + 1 <= max_depth`.

This preserves current default behavior:

- `max_depth = 1`: root can create one child, that child cannot delegate again.
- `max_depth = 2`: root can create a child, and that child can create one grandchild.
- `max_depth = 3`: root -> child -> grandchild -> great-grandchild is allowed.

### Session Visibility Model

`tools.sessions.visibility = "children"` will mean "current session plus descendant delegate chain", not just one-hop children.

This is necessary once nested delegation exists; otherwise the root session cannot inspect deeper delegated work it initiated indirectly.

### Child Tool View Model

Child tool visibility will be computed from:

1. runtime-supported core tools
2. `tools.delegate.child_tool_allowlist`
3. `tools.delegate.allow_shell_in_child`
4. whether another nested `delegate` step is still allowed at the child depth

Session inspection tools remain excluded from delegated child views in this phase.

## Implementation Shape

### Repository Layer

Add lineage helpers to `SessionRepository`:

- compute session lineage depth from `parent_session_id`
- determine whether a target session is a visible descendant of the current session
- list visible descendant sessions recursively

Legacy fallback stays best-effort and current-session-scoped. It remains useful for old roots and resumed sessions, but descendant visibility still depends on concrete `sessions` metadata because historical `turns` do not encode parentage.

### Tool Catalog Layer

Replace the hard-coded delegate child view assembly with a config-aware function that can:

- include runtime-supported child core tools from allowlist
- optionally add `shell.exec`
- optionally add `delegate`

The default child allowlist should be tightened to currently supported runtime child tools so the config no longer promises tools that do not exist.

### Turn Loop Layer

Before creating a child session, the delegate executor will:

1. compute current lineage depth
2. reject if the next child depth would exceed `max_depth`
3. compute whether the child should itself see `delegate`
4. create the child `SessionContext` with that dynamic tool view

This keeps delegate orchestration in `conversation/turn_loop.rs`, where it already owns timeout, event logging, and nested turn execution.

## Error Semantics

Two layers remain intentional:

- `tool_not_visible: delegate`
  Returned when the provider tried to call a tool that is not in the current session's tool view.

- `delegate_depth_exceeded`
  Returned defensively when runtime lineage checks show the requested nested delegate would exceed configured depth.

The first is the normal policy surface. The second protects against mismatched or stale tool-view assumptions.

## Testing Strategy

Use TDD and add regression coverage for:

1. child tool view excludes `delegate` when no further depth remains
2. child tool view includes `delegate` when further depth remains
3. nested grandchild creation succeeds when `max_depth` allows it
4. nested delegate returns `delegate_depth_exceeded` when the next level would exceed config
5. root `sessions_list` can see descendant sessions, not only direct children
6. root `session_status` can inspect descendant sessions
7. legacy fallback behavior remains intact for sessions that only exist in `turns`

## Documentation Impact

Update product and roadmap docs to describe:

- synchronous nested delegate semantics
- descendant session visibility
- the meaning of `max_depth`
- the explicit non-goals that are still deferred
