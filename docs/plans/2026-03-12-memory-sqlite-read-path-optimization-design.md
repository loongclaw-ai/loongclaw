# Memory SQLite Read Path Optimization Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Reduce repeated SQLite opens and repeated context reads in the `read_context`
 path while preserving existing prompt-context semantics.

## Problem

After kernel and non-kernel prompt-context semantics were unified, the hot
 memory read path became clearer:

- `context::load_prompt_context(...)` loads the active window
- `window_plus_summary` then loads the full session again to derive the summary
- each SQLite read path currently performs schema preparation and opens a fresh
  connection

That means a single `read_context` call in `window_plus_summary` mode can
trigger multiple `Connection::open(...)` calls and duplicate reads over the same
 session.

## Constraints

- Preserve existing output semantics for:
  - `profile_plus_window`
  - `window_plus_summary`
  - normal sliding-window turn ordering
- Keep the public memory API additive-only.
- Do not introduce schema changes in this slice.
- Do not yet build materialized summaries or long-lived engine state.

## Options Considered

### Option A: Shared SQLite read-path helper and context snapshot

Add a SQLite-only helper that:

- opens one connection
- prepares schema once
- loads the active window once
- optionally loads only the older turns needed for summary construction

Then route `context::load_prompt_context(...)` through that helper.

Pros:

- smallest safe performance slice
- preserves behavior
- no schema migration
- easy to verify with focused tests

Cons:

- does not eliminate `O(total_turns)` summary construction yet
- still uses per-call SQLite connections

### Option B: Long-lived SQLite engine

Introduce a reusable SQLite engine or connection manager and route all memory
 operations through it.

Pros:

- larger performance upside
- sets up WAL, prepared statements, and future pooling

Cons:

- broader surface change
- harder to land safely in one slice

### Option C: Materialized summary table

Persist summary checkpoints and avoid rebuilding summary text from older turns on
 every read.

Pros:

- highest long-session payoff

Cons:

- requires schema and write-path changes
- larger correctness risk

## Decision

Implement Option A now.

This slice will:

- add a shared SQLite connection helper for memory operations
- add a SQLite context-snapshot helper that splits:
  - active window turns
  - older turns used for summary construction
- route `context::load_prompt_context(...)` through the snapshot helper
- keep public memory and conversation call sites unchanged

This slice will not yet:

- introduce a long-lived SQLite engine
- add WAL/pooling state
- materialize summaries in storage
- split event lanes into separate tables

## Rationale

This is the safest minimal optimization with real hot-path impact. It removes
 duplicate reads and repeated connection setup from the most exercised memory
 hydration path without changing the observable prompt-context contract.

It also creates a cleaner internal seam for the next slice, where summary
 materialization or engine reuse can be added without reworking higher layers.

## Verification

- Add failing tests for a SQLite context snapshot helper that separates window
  turns from older summary-source turns.
- Add a failing regression test proving `load_prompt_context(...)` still
  preserves summary and turn ordering through the new path.
- Run targeted memory tests.
- Run `cargo test --workspace --all-features`.
- Run `./scripts/check_architecture_boundaries.sh`.
