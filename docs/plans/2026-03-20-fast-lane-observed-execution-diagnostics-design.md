# Fast-Lane Observed Execution Diagnostics Design

## Problem

The current fast-lane tool batch event tells operators what LoongClaw planned to
do:

- whether fast-lane parallel execution was enabled
- the configured max in-flight limit
- how intents were segmented into `parallel` or `sequential` batches

That is useful, but still incomplete.

The trace is built before execution starts, so it cannot answer the operational
questions that matter when debugging parallel tool execution on `dev`:

- did the batch actually execute with concurrency greater than one?
- what peak concurrency was observed during this run?
- how long did the batch and each segment take?
- did a nominally parallel segment effectively degrade to single-flight
  execution?

There is also one semantic mismatch in the current planner: a segment is labeled
`parallel` whenever fast-lane parallel execution is enabled and the segment has
multiple `parallel_safe` intents, even if the configured in-flight cap is `1`.
That classification overstates what the runtime can actually do.

## Goal

Extend fast-lane batch diagnostics so one persisted event exposes both:

1. configured execution intent
2. observed execution behavior

This slice should make it possible to compare planned vs actual parallelism from
the existing event summary and CLI output without redesigning scheduling policy.

## Non-Goals

- adding a new fast-lane health daemon or startup doctor workflow
- changing safe-lane plan execution
- changing lane arbitration heuristics
- adding adaptive concurrency or policy feedback loops
- introducing new persistence tables or a second event type

## Constraints

- keep the change additive and backward-compatible with existing event history
- avoid hardcoded thresholds in analytics
- preserve existing fast-lane event persistence flow
- keep the implementation rooted in the execution path instead of post-hoc
  analytics inference
- minimize change surface outside the fast-lane tool batch path

## Approaches Considered

### Option A: Derive observed behavior only in analytics

Infer actual concurrency from existing segment counts and configured
`parallel_execution_max_in_flight`.

Pros:

- small analytics-only patch

Cons:

- cannot distinguish configured capability from actual runtime behavior
- cannot measure elapsed time
- cannot detect runtime degradation when the configured cap is greater than one

### Option B: Add observed metrics directly to the execution trace

Create the fast-lane trace before execution, then fill observed metrics while
each segment runs and persist the enriched event through the existing path.

Pros:

- rooted in the real execution boundary
- keeps one event as the source of truth
- supports both batch-level and segment-level observed metrics
- keeps analytics simple and backward-compatible

Cons:

- touches the execution path and tests in multiple layers

### Option C: Emit a second execution-result event after the batch finishes

Keep the existing plan event unchanged and add a new event with observed
metrics.

Pros:

- preserves a clean separation between plan and execution

Cons:

- duplicates event plumbing
- complicates summary reconstruction
- creates ordering and partial-failure edge cases for little practical benefit

## Decision

Choose Option B.

The execution trace should remain the single persisted record for one fast-lane
tool batch, but it needs to carry observed runtime metrics in addition to
configured metadata.

## Proposed Model

### Batch-level metrics

Keep the existing configured fields and add:

- `observed_peak_in_flight`
- `observed_wall_time_ms`

These metrics describe the full batch as executed, not just the planned
segmentation.

### Segment-level metrics

Keep the existing segment shape and add optional observed fields:

- `observed_peak_in_flight`
- `observed_wall_time_ms`

Observed fields are optional at the segment level so a partially executed batch
can still persist a truthful trace when a later segment never starts.

### Execution-mode correction

Update segment classification so a segment is only labeled `parallel` when all
of the following are true:

- fast-lane parallel execution is enabled
- the segment contains more than one `parallel_safe` intent
- the configured `parallel_execution_max_in_flight` is greater than `1`

This keeps planned `execution_mode` aligned with the runtime's effective
concurrency budget.

## Data Flow

1. Build the batch trace before execution so configured metadata remains stable.
2. Execute segments in order.
3. Measure batch elapsed time around the whole execution loop.
4. For each segment:
   - measure elapsed time
   - capture observed peak in-flight concurrency
   - write those values back into the matching trace segment
5. Persist the enriched trace through the existing `fast_lane_tool_batch` event.
6. Extend analytics summary folding to read the new optional fields while
   remaining compatible with older payloads.
7. Extend `/fast_lane_summary` to render configured vs observed values clearly.

## Analytics Surface

Add lightweight summary rollups that stay threshold-free:

- latest observed batch peak in-flight
- latest observed batch wall time
- aggregate observed batch peak average/max
- aggregate observed batch wall-time average/max
- count of degraded parallel segments where a segment planned as `parallel`
  observed a peak in-flight of `1`

This provides immediate operator value while keeping future health policies free
to derive their own thresholds later.

## Testing Strategy

1. Add a `turn_engine` test that proves observed concurrency is captured from
   the real execution path, including mixed parallel and sequential segments.
2. Add analytics tests for latest and aggregate observed metrics, including
   degraded parallel segment counting.
3. Update fast-lane CLI summary tests so the rendered output shows configured vs
   observed values.
4. Update conversation integration coverage to ensure persisted batch events
   include the new fields.

## Risks and Mitigations

### Risk: flakey concurrency assertions

Mitigation:

- use a dedicated test dispatcher with explicit async delays so concurrent
  overlap is deterministic

### Risk: partial execution traces become misleading on failures

Mitigation:

- keep segment-level observed fields optional
- record observations immediately when a segment exits, even on failure paths

### Risk: event history compatibility

Mitigation:

- treat observed fields as optional during summary parsing
- preserve all existing payload fields and event names
