# Alpha Session Runtime Reintegration Design

## Goal

Rebuild the hidden session runtime foundation directly on top of the current `alpha-test`
architecture so the stacked session tool PRs can be rebased onto a clean, mergeable base.

## Problem

The old hidden base branch `feat/session-archive-base-20260313` is no longer a practical merge
target for `alpha-test`. The conflict surface spans conversation runtime, provider prompt
construction, tool routing, channel delivery, daemon startup, config, and docs. Rebasing or
merging that branch wholesale would re-introduce architecture drift and make future reviews
opaque.

At the same time, the stacked PRs above it depend on capabilities that do not exist on current
`alpha-test`:

- session-aware tool catalog and tool visibility
- session repository and recovery primitives
- runtime environment export for child execution paths
- messaging/delegate/session app-tool surfaces
- conversation/runtime wiring that can execute app tools without breaking current turn orchestration

## Constraints

- `alpha-test` is the source of truth.
- The rebuild must land in small, reviewable slices.
- Every slice must preserve current green behavior before expanding surface area.
- New session tools must not be advertised before the runtime can safely execute them.
- Validation must cover `fmt`, `clippy -D warnings`, targeted tests, and broader workspace tests
  before GitHub delivery is updated.

## Recommended Approach

### Approach A: Rebase the old hidden base branch

Rejected. The drift is architectural, not cosmetic. The conflict set is too wide and mixes
obsolete assumptions about conversation/runtime internals with new `alpha-test` abstractions.

### Approach B: Cherry-pick old commits selectively

Only partially viable. Some files are useful as reference, but the original commit boundaries are
still coupled to the old architecture. Cherry-picks will be used sparingly as source material, not
as the primary integration strategy.

### Approach C: Reconstruct the base natively on current `alpha-test`

Recommended. Preserve the intent and tests from the hidden base, but rebuild the implementation in
small slices that fit the current architecture. This keeps each change locally understandable and
lets CI validate the new design incrementally.

## Phase Plan

### Phase 1: Tool Catalog Foundation

Introduce a catalog-backed tool surface on current `alpha-test` without changing external behavior.
This establishes the abstractions later slices need:

- `ToolCatalog`
- `ToolDescriptor`
- `ToolView`
- runtime/planned availability metadata
- view-aware capability snapshots and provider tool definitions

The first phase must preserve the currently exposed core tools exactly.

### Phase 2: Config Surface For Session Runtime

Port the session/message/delegate config blocks needed to describe future app-tool visibility and
delegate-child limits, but keep new app tools hidden until their runtime exists.

### Phase 3: Session Runtime Primitives

Introduce `runtime_env`, `session/`, and the repository/recovery foundation. This phase should be
tested primarily at the repository/runtime layer before any tool exposure changes.

### Phase 4: App Tool Execution Path

Add app-tool execution routing and integrate it into the current conversation runtime/turn
coordinator path without regressing existing kernel-mediated tool execution.

### Phase 5: Session Tool Behaviors

Port `sessions_list`, `session_status`, `session_events`, `sessions_history`, `session_wait`,
`session_recover`, `session_cancel`, `sessions_send`, and archive/unarchive surfaces in reviewable
sub-slices.

## Architecture Notes

### Tool Surface

Current `alpha-test` still hardcodes its provider tool schema and capability snapshot directly in
`crates/app/src/tools/mod.rs`. The rebuild should move to a catalog-driven model first, because the
later session runtime requires filtered views for root sessions, child sessions, and future planned
tools.

### Conversation Runtime

Current `alpha-test` already has a stronger `ConversationRuntime` and `ConversationTurnCoordinator`
than the old hidden base. The rebuild should adapt session runtime concepts into these extension
points instead of attempting to restore the older `ConversationTurnLoop`-centric shape.

### Safety

New session tools must remain hidden from provider schemas until:

- the repository layer is durable
- execution routing is explicit
- visibility rules are enforced per session type

## Validation Strategy

Each slice should follow:

1. add or extend failing tests first
2. implement the minimal production change
3. run targeted tests for the touched slice
4. run `cargo fmt --all -- --check`
5. run `cargo clippy --workspace --all-targets --all-features -- -D warnings`
6. run broader workspace tests before PR updates

## Delivery Strategy

Keep issue `#132` as the tracking issue. Replace PR `#133` with a new PR from the fresh
`alpha-session-runtime-integration-20260314` branch once the rebuilt base becomes self-contained
and green against `alpha-test`. Leave explicit traceability from the new PR back to `#133`.
