# Chat Diagnostics Binding Normalization Design

Date: 2026-03-16
Scope: follow-up for issue #191 on `alpha-test`

## Problem

`ConversationRuntimeBinding` already exists to make conversation execution mode
explicit, but the CLI chat diagnostics layer still leaks back to raw
`Option<&KernelContext>`.

The remaining drift is concentrated in:

1. `crates/app/src/chat.rs`
   - `print_history(...)`
   - `print_safe_lane_summary(...)`
   - `print_turn_checkpoint_summary(...)`
   - `print_turn_checkpoint_repair(...)`
2. `crates/app/src/conversation/session_history.rs`
   - `load_discovery_first_event_summary(...)`

These helpers either:

1. accept `Option<&KernelContext>` even when the caller already knows the
   runtime mode, or
2. immediately convert that optional kernel context back into
   `ConversationRuntimeBinding`.

That does not create a new exploit on its own, but it weakens the type-level
contract around governed versus direct execution and makes future drift easier.

## Goals

1. Keep chat diagnostics on explicit `ConversationRuntimeBinding<'_>`.
2. Keep the session-history discovery-first summary implementation on the same
   explicit binding contract as the neighboring history helpers without
   breaking the existing public `Option<&KernelContext>` wrapper.
3. Preserve current behavior:
   - kernel-bound callers still use kernel memory windows
   - direct callers still use direct/sqlite history paths
   - user-facing command output remains unchanged
4. Keep the slice small enough for a standalone issue and PR.

## Non-goals

1. Do not normalize the remaining provider-layer `Option<&KernelContext>` seams.
2. Do not widen this slice into turn-loop or ACP work.
3. Do not redesign history summary rendering or CLI output formats.

## Alternatives Considered

### A. Leave chat diagnostics alone because they are "just CLI helpers"

Rejected. These helpers are user-facing orchestration boundaries. Leaving them
optional-context-based keeps the conversation runtime story weaker exactly where
operator tooling is supposed to be truthful.

### B. Resume the broad conversation binding normalization refactor

Rejected for this branch. The broader 2026-03-15 plan spans multiple files and
surfaces. This follow-up should land a tighter, lower-risk slice that directly
matches issue #191.

### C. Normalize only the chat diagnostics and discovery-first helper

Recommended. It removes the most obvious optional-context leftovers at the CLI
diagnostic boundary without dragging in unrelated conversation internals.

## Proposed Design

Use `ConversationRuntimeBinding<'_>` directly in the remaining chat diagnostic
helpers and in `load_discovery_first_event_summary(...)`.

Concrete changes:

1. `chat.rs`
   - command dispatch sites pass `ConversationRuntimeBinding::kernel(&runtime.kernel_ctx)`
   - helper signatures accept `ConversationRuntimeBinding<'_>`
   - `print_*` remains a thin CLI shell around binding-aware output builders so
     routing and rendered output can be asserted directly in tests
2. `session_history.rs`
   - keep `load_discovery_first_event_summary(...)` as the compatibility wrapper
     that still accepts `Option<&KernelContext>`
   - move the normalized binding-first implementation into an internal helper so
     adjacent history helpers share the same explicit runtime contract without a
     public API break

## Expected Outcome

After this slice:

1. chat diagnostics no longer encode execution mode as `Some/None`
2. the conversation history implementation becomes more internally consistent
   while preserving the current public discovery-first helper signature
3. the governed/runtime narrative becomes slightly more truthful without
   changing runtime behavior

## Test Strategy

Add focused regression coverage that proves:

1. chat diagnostic output builders produce the expected `/history`,
   `/safe_lane_summary`, and `/turn_checkpoint_summary` output for both direct
   and kernel bindings
2. the normalized internal discovery-first helper runs with explicit direct and
   kernel bindings
3. the public discovery-first wrapper still accepts `None` and `Some(&ctx)`
   without compile-time or runtime regressions

The tests do not need to prove new behavior. They need to prove that the
binding contract has become explicit at these remaining seams while the public
compatibility boundary remains stable.
