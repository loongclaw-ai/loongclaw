# App Tool Runtime Unification Design

## Context

LoongClaw's app-tool surface has grown in two layers:

- `crates/app/src/tools/mod.rs` exposes catalog, runtime tool view, and a synchronous
  `execute_app_tool_with_config(...)` helper.
- `crates/app/src/conversation/turn_engine.rs` owns the runtime dispatcher used by real
  conversation turns.

This was acceptable while app tools were synchronous session or transcript helpers. It became
misleading once `session_wait`, `sessions_send`, and `delegate_async` landed:

- they are advertised in the runtime tool surface
- they are executable through the real dispatcher
- but they are still split across dispatcher-local special cases instead of one authoritative
  runtime execution boundary

The result is internal truth drift. Runtime behavior is mostly correct, but the routing model is
harder to reason about, test, and extend.

## Root Cause

The main issue is not that these tools are absent. The issue is that app-tool execution is
fragmented:

1. `execute_app_tool_with_config(...)` handles only the synchronous subset.
2. `DefaultAppToolDispatcher` adds ad-hoc special cases for async-capable tools.
3. `delegate` remains a separate turn-loop concern because it needs nested conversation execution.

That means "what counts as an app tool" is defined in catalog/docs/provider schema, but "where app
tools actually execute" is spread across multiple call sites.

## Design Goals

- Keep the public runtime tool surface truthful.
- Centralize non-`delegate` app-tool execution behind one async helper.
- Preserve the existing `delegate` special path, because it genuinely depends on turn-loop runtime
  recursion and should not be faked as a normal direct app helper.
- Reuse existing tool implementations instead of rewriting session, memory, or messaging logic.
- Make unsupported execution modes explicit with precise errors instead of generic
  `app_tool_not_implemented`.

## Chosen Approach

Introduce an authoritative async app-tool execution helper in `crates/app/src/tools/mod.rs` for
all non-`delegate` app tools.

This helper will:

- route synchronous session and memory tools through existing helpers
- route `session_wait` through the existing async wait helper
- route `sessions_send` through the existing messaging helper when app config is present
- route `delegate_async` through a shared delegate-async execution helper
- reject `delegate` with an explicit "requires turn-loop dispatch" error

To support this, move the reusable delegate-async runtime pieces out of
`conversation/turn_engine.rs` into `crates/app/src/tools/delegate.rs`:

- `AsyncDelegateSpawnRequest`
- `AsyncDelegateSpawner`
- async spawn failure persistence helpers
- `execute_delegate_async_with_config(...)`

`DefaultAppToolDispatcher` will then become thinner:

- compute effective tool visibility and effective tool config
- build runtime support inputs
- call the centralized async app-tool executor

`TurnLoopAppToolDispatcher` will still intercept `delegate` first and only fall back to the
default dispatcher for the rest.

## Why This Approach

This is the smallest truthful refactor that improves architecture instead of papering over it.

- It fixes the real layering problem without claiming `delegate` is a plain app helper.
- It reuses existing tested behavior for `session_wait`, `sessions_send`, and session/memory
  tools.
- It makes the next tool slice easier, because new async app tools will have one clear runtime
  entry point.

## Non-Goals

- Implementing browser, cron, or semantic memory features
- Changing tool catalog visibility policy
- Reworking the synchronous delegate flow in `turn_loop.rs`
- Making every app tool executable through the old synchronous helper

## Validation Strategy

- Add direct tests for the new async app-tool runtime helper
- Keep existing dispatcher behavior tests green
- Re-run full `loongclaw-app` tests
- Re-run `loongclaw-daemon --no-run` to verify no compile regressions on the daemon surface
