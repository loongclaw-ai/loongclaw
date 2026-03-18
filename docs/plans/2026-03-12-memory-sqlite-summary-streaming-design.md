# Memory SQLite Summary Streaming Design

## Context

`crates/app/src/memory/sqlite.rs` already reuses SQLite runtimes per normalized path and now also caches prepared statements on the long-lived connection. The next remaining hot path is summary materialization, especially in `WindowPlusSummary` mode.

The current rebuild/catch-up pipeline still does extra work in two places:

- `query_turns_up_to_id(...)` and `query_turns_between_ids(...)` materialize whole `Vec<IndexedConversationTurn>` collections even when the caller only needs:
  - the rolling summary string
  - the newest processed turn id
- `build_summary_body(...)` and `append_summary_line(...)` create additional short-lived allocations:
  - `String::new()` starts without capacity planning
  - `normalize_summary_content(...)` does `split_whitespace -> Vec<&str> -> join(" ")`
  - rebuild paths allocate turns first, then summarize them in a second pass

On long sessions this means summary rebuild cost scales with both SQLite row count and avoidable heap churn. For low-code local memory behavior, that is undesirable because a local memory read/write primitive should stay flat and predictable even as session history grows.

## Goal

Keep external memory behavior unchanged while making summary rebuild/catch-up operate in a streaming style: read rows once, update the summary body incrementally, and avoid intermediate turn vectors and unnecessary string allocation churn.

## Options Considered

### Option 1: Stream rows directly into summary materialization

Replace the rebuild/catch-up helpers that currently return `Vec<IndexedConversationTurn>` with row iterators/callback-driven accumulation. Track the latest processed turn id during iteration and build the summary body in the same pass.

Pros:

- Removes the largest avoidable intermediate allocation in the summary path.
- Preserves the current schema, runtime model, and public API.
- Keeps semantics identical.
- Small enough to validate with focused regression tests.

Cons:

- Requires a little more helper structure around row iteration.
- Error handling needs to stay crisp because the mapping and accumulation now happen in one pass.

### Option 2: Keep vectors but optimize string handling only

Leave the current query helpers as-is and only preallocate the summary string / rewrite whitespace normalization.

Pros:

- Lowest implementation risk.

Cons:

- Leaves the dominant `Vec<IndexedConversationTurn>` allocation intact.
- Improves memory churn only partially.

### Option 3: Stop materializing summaries during append and only rebuild lazily on read

Shift work out of the write path and materialize on demand from `load_context_snapshot(...)`.

Pros:

- Could reduce write-path latency in some workloads.

Cons:

- Changes behavior/latency distribution.
- Risks stale checkpoint behavior unless additional invalidation metadata is introduced.
- Too invasive for the current optimization slice.

## Recommended Design

Use Option 1 with a small amount of Option 2.

Concretely:

- Introduce streaming row-consumption helpers for the two summary-only query shapes:
  - turns up to frontier
  - turns between checkpoint frontier and new summary boundary
- Accumulate summary text and latest processed turn id in one pass.
- Replace `normalize_summary_content(...)`'s `Vec<&str>` + `join` behavior with direct whitespace-compaction into a reusable `String` buffer or direct append flow.
- Preallocate summary output capacity based on `summary_budget_chars`.
- Keep `query_recent_turns_with_ids(...)` unchanged for now because the active window still needs actual turn objects and that path is already bounded by the window size.

## Data Flow

### Rebuild path

Current:

1. Query all rows into `Vec<IndexedConversationTurn>`
2. Iterate vector to build summary body
3. Persist checkpoint

Proposed:

1. Query rows
2. For each row, compact content and append summary line directly
3. Track the latest turn id processed during the same pass
4. Persist checkpoint

### Catch-up path

Current:

1. Query delta rows into `Vec<IndexedConversationTurn>`
2. Iterate vector to append to checkpoint body
3. Read `turns.last()` for the frontier update
4. Persist checkpoint

Proposed:

1. Query delta rows
2. Stream rows directly into checkpoint body
3. Track the latest processed id while iterating
4. Persist checkpoint

## Verification Strategy

The change must stay behavior-preserving while proving the allocation-heavy shape is gone.

Planned checks:

- focused regression tests showing rebuild and catch-up no longer require vector-backed query helpers
- existing summary correctness tests remain green:
  - checkpoint materialization
  - rebuild on window-size change
  - rebuild on budget change
  - clear-session checkpoint removal

The implementation does not need a benchmark in this slice, but the code shape should make it obvious that the largest summary-path intermediate collections are removed.

## Risk Notes

- Streaming helpers must preserve row ordering exactly.
- The latest processed turn id must remain correct even when summary text hits the character budget and truncates.
- Whitespace normalization must preserve the current semantic output shape: collapsing runs of whitespace into single spaces and skipping empty content.
