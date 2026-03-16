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

## Why This Slice Matters

The strongest immediate architecture risk in `alpha-test` is not missing
abstractions. It is contract drift between what the code claims and what it
actually guarantees. Closing these two paths moves the runtime toward a more
defensible kernel-first model without pretending the entire repository is
already there.
