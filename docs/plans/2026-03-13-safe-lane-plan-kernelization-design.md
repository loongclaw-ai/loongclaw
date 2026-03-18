# Safe-Lane Plan Loop Kernelization Design

Date: 2026-03-13
Branch: `feat/turn-loop-kernelization-20260313`
Scope: behavior-preserving kernelization of the safe-lane plan execution loop in
`crates/app/src/conversation/turn_coordinator.rs`
Status: Approved for implementation

## Problem

`execute_turn_with_safe_lane_plan(...)` has become the control hotspot of the hybrid runtime.
The function currently interleaves:

1. governor bootstrap and effective budget derivation
2. adaptive verify policy escalation
3. round-start event emission
4. plan build + plan execution
5. tool-output stats and metric aggregation
6. verify-failure routing
7. plan-failure routing
8. terminal result construction
9. replan cursor mutation
10. round advancement and retry ceiling mutation

The runtime behavior is strong, but the control flow is now hard to reason about. This increases
review cost and makes future safe-lane evolution risky because side effects and routing logic are
mixed inside a single loop body.

## Goals

1. Keep safe-lane external behavior unchanged in this slice.
2. Introduce explicit typed loop state for safe-lane plan execution.
3. Introduce explicit typed round outcome and next-step decision boundaries.
4. Separate pure routing / terminalization logic from side effects such as event persistence.
5. Make later hybrid-runtime upgrades safer by reducing hidden state transitions.

## Non-Goals

1. No config schema changes.
2. No new runtime event names or payload semantics.
3. No governor policy tuning changes.
4. No verifier policy behavior changes.
5. No ACP or fast-lane behavior changes.

## Approaches Considered

### A. Helper extraction only

Extract a few helper functions but keep the current implicit loop shape.

Pros:
- smallest patch
- low local risk

Cons:
- leaves mutation order implicit
- does not create a durable state-machine seam
- future changes still need to reason about the whole loop body

### B. Behavior-preserving safe-lane loop kernel

Introduce explicit internal state, explicit round outcome types, and explicit next-step decisions
while preserving current behavior.

Pros:
- creates a durable kernel boundary
- keeps reviewable scope inside one function family
- prepares safe lane for later convergence with fast-lane kernel concepts

Cons:
- larger patch than pure helper extraction
- requires new kernel-level tests

### C. Kernelization plus policy cleanup

Kernelize the loop and also change routing / governor / verifier semantics in the same patch.

Pros:
- potentially cleaner end state immediately

Cons:
- too much semantic risk in one slice
- harder to attribute regressions
- harder to review

## Decision

Implement Approach B.

The next correct step is not new safe-lane features. It is to make the existing safe-lane plan
loop explicit and testable so future policy work sits on a stable execution kernel.

## Target Design

### 1. Explicit safe-lane session state

Add a local state container that owns:

1. governor decision
2. current round index
3. effective round / node retry ceilings
4. replan cursor (`plan_start_tool_index`)
5. seeded tool outputs
6. aggregated runtime metrics
7. adaptive verify policy state

This makes cross-round mutation visible and auditable.

### 2. Explicit round outcome

Represent a safe-lane round as a typed outcome that captures:

1. plan execution report
2. tool outputs for the round
3. summarized tool-output stats
4. derived failure metadata when present
5. verify report when the plan succeeded

This separates “what happened in the round” from “what should happen next.”

### 3. Explicit next-step decision

Introduce a typed decision enum for the loop:

1. finalize success
2. finalize terminal verify failure
3. finalize terminal plan failure
4. replan after verify failure
5. replan after plan failure

The outer loop should apply side effects from that decision instead of deriving behavior from
nested inline branches.

### 4. Event emission stays outside the pure kernel

This slice should keep event names and payload structure unchanged. The kernel only decides what
the next state transition is. Event emission remains an effectful layer applied by the loop.

This preserves compatibility while still reducing control-flow complexity.

## Testing Strategy

1. Add kernel-level tests for pure decision helpers:
   - verify failure replan with remaining budget
   - verify failure terminalized by session governor override
   - plan failure replan preserves failed-subgraph restart cursor
   - plan failure terminalization maps to current failure codes
2. Keep existing high-level safe-lane runtime tests green.
3. Re-run targeted `turn_coordinator` tests before wider package verification.

## Acceptance Criteria

1. `execute_turn_with_safe_lane_plan(...)` becomes thinner and coordinates explicit kernel steps.
2. Cross-round mutation lives in a typed state container.
3. Verify / route / terminal decisions become explicit and unit-testable.
4. No intentional safe-lane behavior changes are introduced.
5. Existing safe-lane runtime tests remain green.
