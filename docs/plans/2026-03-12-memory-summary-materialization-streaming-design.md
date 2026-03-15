# Memory Summary Materialization Streaming Design

## Context

`crates/app/src/memory/sqlite.rs` already reuses a long-lived SQLite connection per normalized database path and now routes hot SQL through `prepare_cached`. That removed connection churn and repeated statement compilation, but the summary materialization path still performs avoidable CPU and heap work:

- rebuild path loads every qualifying turn into `Vec<IndexedConversationTurn>` before building the summary string
- catch-up path does the same for newly summarized turns
- the summary builder then walks those turns again, allocating additional temporary strings per turn

This means summary rebuild and catch-up still pay for:

- full row materialization into heap-owned structs
- duplicate traversal of the same row set
- repeated dynamic growth of the target summary buffer

For the low-code memory layer, this is the remaining hot path most likely to amplify latency when session history grows and summary mode is enabled.

## Goal

Reduce CPU and heap overhead in summary materialization by streaming rows directly from SQLite into the summary builder instead of first collecting them into intermediate vectors, while preserving current summary content, truncation behavior, and checkpoint semantics.

## Hot Spots

- `materialize_summary_checkpoint` in `crates/app/src/memory/sqlite.rs`
  - rebuild and catch-up branches still depend on row collection helpers before string assembly
- `rebuild_summary_checkpoint`
  - calls `query_turns_up_to_id(...)`, then `build_summary_body(...)`
- catch-up section inside `materialize_summary_checkpoint`
  - calls `query_turns_between_ids(...)`, then loops again to append lines
- `build_summary_body` / `append_summary_line`
  - target string starts empty and grows incrementally; line assembly still creates per-turn intermediate strings

## Options Considered

### Option 1: Stream rows directly into summary construction

Introduce streaming helpers that:

- run the summary row query
- iterate rows once
- append directly into the output summary string
- track the last summarized turn id during iteration

Pros:

- eliminates intermediate `Vec<IndexedConversationTurn>` in summary rebuild/catch-up
- avoids second traversal over the same turns
- smallest high-value change with low behavioral risk

Cons:

- requires a new helper shape for row iteration and checkpoint update bookkeeping

### Option 2: Rewrite summary line normalization to be fully append-only

Keep the same query shape, but replace `split_whitespace().collect::<Vec<_>>().join(" ")` and per-line `format!` with a direct append-only normalizer.

Pros:

- reduces per-turn temporary allocations
- keeps data flow local

Cons:

- does not remove the bigger `Vec<IndexedConversationTurn>` materialization cost
- more subtle truncation/formatting edge cases

### Option 3: Push more of the summarization pipeline into SQL or a secondary checkpoint structure

Use SQL-side aggregation or auxiliary summary metadata to reduce rebuild scanning.

Pros:

- potentially largest long-term reduction for very large histories

Cons:

- much larger semantic and maintenance surface
- significantly higher risk than needed for the current hot path

## Recommended Design

Use Option 1 now, with one small companion improvement from Option 2: reserve the summary buffer close to the configured character budget to reduce reallocations.

The slice is:

- add streaming query helpers for summary rebuild and catch-up
- build the summary string in the same pass that rows are read
- return the last summarized turn id from the streaming pass so checkpoint state can be updated without storing the full row set
- pre-reserve summary string capacity using the configured budget cap
- keep external APIs and checkpoint schema unchanged

## Validation Strategy

The regression proof should be explicit, not just inferred from benchmarks.

Add test-only counters around the legacy bulk-materialization path used only by summary rebuild/catch-up. Then add targeted tests proving:

- rebuild path no longer performs bulk turn materialization
- catch-up path no longer performs bulk turn materialization

Existing behavioral tests already cover:

- summary checkpoint creation
- rebuild on window-size change
- rebuild on budget change
- clear-session behavior

Those stay in place to guarantee semantic parity.

## Risk Notes

- The streaming path must preserve current ordering and checkpoint frontier semantics.
- Any truncation differences in summary formatting would be a regression.
- The smallest safe slice is to stream rows first; fully removing all temporary strings inside normalization can wait for a later pass if needed.
