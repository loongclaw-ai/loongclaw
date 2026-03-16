# Governed Runtime Path Hardening Design

Date: 2026-03-16
Branch: `fix/alpha-test-governed-runtime-path-hardening-20260316`
Scope: close the highest-value conversation-runtime governed/direct drift in `alpha-test`

## Problem

`alpha-test` now carries an explicit `ConversationRuntimeBinding`, but two hot
paths still contradict the kernel-first story in production behavior:

1. async delegate child execution inherits a parent session lineage yet drops
   into `ConversationRuntimeBinding::Direct`
2. kernel-bound session-history reads silently downgrade to direct sqlite when
   the kernel memory-window request fails or returns a non-`ok` outcome

Those paths are not harmless implementation details. They weaken the meaning of
"kernel-bound" from an execution contract into caller discipline.

## Goals

1. Preserve parent conversation binding when launching async delegate child
   turns.
2. Make kernel-bound session-history reads fail closed instead of silently
   downgrading to direct sqlite.
3. Update architecture/security docs so they describe the real runtime contract
   after this slice, including the remaining intentional direct paths.
4. Keep the patch reviewable and local to conversation/runtime/documentation
   seams.

## Non-goals

1. Do not kernelize every direct path in `app`, `channel`, or `acp`.
2. Do not redesign tool approval, channel delivery, or provider failover.
3. Do not introduce the persistent audit sink in this slice.

## Alternatives Considered

### A. Full repository-wide kernelization

Rejected. It would mix channel, provider, session, and conversation concerns
into one high-risk patch and make failures hard to attribute.

### B. Add more audit around the drift but keep behavior

Rejected. That would improve observability but still leave the architecture
contract weaker than the documentation.

### C. Close the highest-value governed/direct gaps first

Recommended. It delivers a concrete architecture-truth improvement with bounded
blast radius and regression tests.

## Decision

Implement option C in one reviewable slice:

1. carry conversation runtime binding through async delegate spawn
2. fail closed when a kernel-bound history request cannot be satisfied by the
   kernel
3. document the current state precisely, including remaining intentional direct
   seams

## Proposed Design

### 1. Async delegate children inherit runtime binding

`AsyncDelegateSpawnRequest` should carry owned inherited kernel authority for
detached child execution. In practice that means threading an owned
`Option<KernelContext>` through the spawn request, then reconstructing
`ConversationRuntimeBinding` inside the async delegate spawner before calling
`run_started_delegate_child_turn_with_runtime(...)`.

This keeps the semantics simple:

1. direct parent -> direct child remains allowed
2. kernel-bound parent -> kernel-bound child remains governed

The child runtime no longer invents a weaker execution mode than the parent.

Implementation notes for this slice:

1. `KernelContext` derives `Clone` so detached spawn requests can own inherited
   authority without borrowing parent stack state.
2. `AsyncDelegateSpawnRequest` carries `kernel_context: Option<KernelContext>`
   instead of inferring runtime mode from an absent borrow.
3. The async delegate spawner reconstructs `ConversationRuntimeBinding` via
   `ConversationRuntimeBinding::from_optional_kernel_context(...)` before
   entering child-turn execution.
4. If inherited kernel execution later fails, the child returns an explicit
   error through the existing spawn/future path rather than silently
   downgrading to direct mode.

### 2. Kernel-bound history reads fail closed

`load_assistant_contents_from_session_window(...)` currently treats
"kernel returned an error" and "turn intentionally runs direct" as the same
outcome. Those are different states.

The helper should instead behave as follows:

1. `ConversationRuntimeBinding::Direct` -> read from sqlite directly
2. `ConversationRuntimeBinding::Kernel(_)` and kernel returns `ok` -> use kernel
   payload
3. `ConversationRuntimeBinding::Kernel(_)` and kernel errors or returns
   non-`ok` -> return an explicit error to the caller

That preserves direct compatibility paths without allowing governed reads to
degrade silently.

For higher-level consumers that intentionally fail open, the runtime should at
least surface that distinction explicitly. In this slice the safe-lane session
governor keeps fail-open planning behavior, but it records history load status
and a normalized error code in runtime diagnostics instead of silently looking
identical to "no historical signal".

#### Backward Compatibility And Migration

This is an intentional contract change for kernel-bound history readers only.
Direct-mode callers still read sqlite directly, but governed callers now get an
explicit error instead of a shadow sqlite fallback.

Affected consumers in this slice:

1. safe-lane governor history loading in
   `load_safe_lane_history_signals_for_governor(...)`
2. checkpoint readers reached through
   `load_turn_checkpoint_history_snapshot(...)`,
   `load_turn_checkpoint_event_summary(...)`, and
   `load_latest_turn_checkpoint_entry(...)`
3. higher-level history summaries that opt into kernel-bound execution, such as
   safe-lane and discovery-first summary helpers

Migration approach for `alpha-test`:

1. leave `ConversationRuntimeBinding::Direct` behavior unchanged so direct-mode
   compatibility remains stable
2. let kernel-bound callers see explicit failures and update fail-open
   orchestration one consumer at a time instead of preserving an implicit shadow
   path
3. keep safe-lane governor fail-open at the planning layer for now, but persist
   `history_load_status` plus a normalized `history_load_error` code so
   operators can distinguish "no history" from "governed history unavailable"

Normalized error contract for persisted diagnostics:

1. `kernel_request_failed` when the kernel memory-window request itself errors
2. `kernel_non_ok_status` when the kernel responds with a non-`ok` status
3. `kernel_malformed_payload` when the kernel responds with `ok` but omits a
   usable `payload.turns` array
4. `direct_read_failed` when an intentional direct-mode sqlite read fails

Observability requirements in this slice:

1. persisted safe-lane governor events keep `history_load_status`
2. persisted safe-lane governor events record only normalized
   `history_load_error` codes
3. full underlying error strings stay in process-local error returns rather than
   durable session events

### 3. Truthful docs for the remaining architecture state

`ARCHITECTURE.md` and `docs/SECURITY.md` should stop claiming that all execution
paths already route through the kernel with no shadow paths. After this slice,
the more accurate statement is:

1. kernel-governed core execution is the architectural direction
2. conversation runtime now distinguishes explicit `Kernel` versus `Direct`
   modes
3. some outer integration and app-only paths still remain intentionally direct
   and are follow-up work

## Expected Behavioral Outcome

1. async delegate children launched from governed parents keep kernel authority
2. kernel-bound history-summary and checkpoint readers report governed failure
   instead of silently reading sqlite
3. direct-mode history helpers still work as before
4. repository docs no longer overclaim full kernel closure

## Test Strategy

Add focused regression coverage for:

1. async delegate spawn request carries the original binding
2. local child-runtime spawn preserves kernel binding in tests
3. kernel-bound history readers fail when the memory window kernel request
   fails
4. direct-mode history readers still use sqlite successfully
5. safe-lane governor diagnostics persist normalized history load error codes

## Why This Slice Matters

The strongest immediate architecture risk in `alpha-test` is not missing
abstractions. It is contract drift between what the code claims and what it
actually guarantees. Closing these two paths moves the runtime toward a more
defensible kernel-first model without pretending the entire repository is
already there.
