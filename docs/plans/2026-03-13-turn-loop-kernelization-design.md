# Turn Loop Kernelization Design

Date: 2026-03-13
Branch: `feat/turn-loop-kernelization-20260313`
Scope: behavior-preserving kernelization of the fast-lane conversation turn loop
Status: Approved for implementation

## Problem

`crates/app/src/conversation/turn_loop.rs` currently concentrates too many responsibilities in
`ConversationTurnLoop::handle_turn_with_runtime(...)`:

1. round-state progression
2. provider error handling
3. tool execution result handling
4. tool-loop detection consumption
5. follow-up message assembly
6. second-pass completion fallback
7. turn persistence and final reply return

The code already works, but its control flow is difficult to reason about because state, decisions,
and side effects are interleaved inside one large loop with repeated match branches.

## Goals

1. Keep external behavior stable for the current fast-lane turn loop.
2. Introduce explicit typed round evaluation and next-step decision boundaries.
3. Separate pure decision logic from side effects such as persistence and completion requests.
4. Keep loop guard semantics, raw-tool-output mode, and follow-up prompt behavior unchanged.
5. Make future evolution toward a stronger hybrid agent loop safer by reducing hidden state
   transitions.

## Non-Goals

1. No user-visible turn-loop policy changes in this slice.
2. No safe-lane `turn_coordinator` redesign in this slice.
3. No context-engine API redesign in this slice.
4. No ACP behavior changes in this slice.

## Approaches Considered

### A. Helper extraction only

Move repeated branches into helper functions but keep the implicit control flow shape.

Pros:
- smallest patch
- low short-term risk

Cons:
- hidden state transitions remain
- hard to prove behavior in future changes
- does not create a real kernel boundary

### B. Behavior-preserving turn kernel

Introduce explicit session state, round evaluation, and next-step decision structures while keeping
current externally visible behavior intact.

Pros:
- gives the turn loop a real state-machine boundary
- keeps refactor risk manageable because behavior does not intentionally change
- creates durable seams for later hybrid/runtime evolution

Cons:
- larger patch than pure helper extraction
- requires new kernel-level tests

### C. Kernelization plus policy cleanup

Kernelize the loop and also tighten round-budget / loop-guard / raw-fallback semantics in the same
change.

Pros:
- potentially cleaner end state immediately

Cons:
- mixes refactor and behavior change
- increases regression risk sharply
- harder to review and attribute failures

## Decision

Implement Approach B.

The correct first step is to make the fast-lane turn loop explicit and testable without changing its
observable behavior. Once the kernel exists, later policy adjustments can happen on top of a stable
state machine instead of inside a monolithic function.

## Target Design

### 1. Explicit session state

Introduce a typed session-state container for the active turn loop that owns:

1. assembled messages
2. raw-output request mode
3. last raw reply
4. loop supervisor state
5. follow-up payload budget
6. current round index

This keeps mutable per-turn-loop state in one place instead of scattering it across locals.

### 2. Explicit round evaluation

Represent one round of work as a typed evaluation result that captures:

1. provider turn
2. whether tool intents existed
3. the `TurnResult`
4. loop-guard verdict for this round
5. the raw reply candidate

This separates “what happened in the round” from “what should happen next.”

### 3. Explicit next-step decision kernel

Add a typed decision enum that describes the next action:

1. continue with follow-up messages
2. finalize directly with a known reply
3. finalize through a second-pass completion request
4. finalize with raw fallback
5. finalize provider error inline

The outer loop then applies side effects from that decision instead of deriving behavior ad hoc from
nested branches.

### 4. Finalization boundary

Unify persistence and return handling behind a narrow finalization path so:

1. success persistence stays centralized
2. inline provider-error persistence stays centralized
3. round-budget fallback stays centralized

This reduces the chance of future changes forgetting one persistence path.

The boundary should carry one explicit reply persistence class instead of duplicating separate
"persist success" and "persist inline provider error" terminal variants. The actual lifecycle after
that persist step still stays local to the caller. In `turn_loop` this remains a simple
persist-or-return boundary. In the coordinator path the same persistence class feeds checkpoint
finalization, after-turn hooks, and context-compaction policy without pretending those later stages
are shared semantics.

### 5. Provider request boundary

Provider-turn request results should also be normalized before the loop body consumes them:

1. successful provider turns continue into round evaluation
2. inline provider errors normalize into one terminal action
3. propagated provider errors normalize into one terminal action

This keeps provider error handling aligned with the same typed terminal boundary used by success
finalization and round-limit fallback, instead of letting `handle_turn_with_runtime(...)` branch
directly into persistence code.

## Testing Strategy

1. Preserve existing behavior tests around `handle_turn_with_runtime(...)`.
2. Add kernel-level tests for round-decision mapping:
   - tool result with continue
   - tool result with hard stop
   - tool failure with continue
   - tool failure with hard stop
   - raw-output mode bypassing second-pass completion
3. Keep follow-up helper tests in `turn_loop.rs` and extend them where needed.
4. Re-run targeted conversation tests first, then broader `loongclaw-app` coverage.

## Acceptance Criteria

1. `handle_turn_with_runtime(...)` becomes thinner and primarily coordinates kernel steps.
2. Round evaluation and next-step decisions become explicit types.
3. No intentional external behavior changes are introduced in this slice.
4. Existing turn-loop behavior tests stay green.
5. New kernel-focused tests cover the decision matrix that was previously implicit.
