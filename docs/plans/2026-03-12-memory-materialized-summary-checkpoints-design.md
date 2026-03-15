# Memory Materialized Summary Checkpoints Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Remove the per-read `O(history)` summary rebuild cost from
`memory.profile = "window_plus_summary"` by materializing a deterministic
summary checkpoint inside the SQLite backend.

## Problem

The previous memory optimization slice removed duplicate SQLite reads, but the
remaining hot-path cost is still substantial for long sessions:

- `read_context` loads the active window
- `window_plus_summary` still scans all older turns outside the window
- summary text is rebuilt from scratch on every read

This means low-code or workflow-heavy sessions pay repeated CPU, allocation, and
string concatenation costs even when only one new turn was appended since the
last read.

## Constraints

- Preserve the public memory API and provider/conversation call sites.
- Keep `turns` as the source-of-truth append-only fact table.
- Preserve current deterministic summary formatting semantics.
- Support existing databases without a manual migration step.
- Keep `window_only` and `profile_plus_window` behavior unchanged.

## Options Considered

### Option A: More read-path trimming without schema changes

Keep rebuilding summaries on read, but reduce SQL and string-copy overhead.

Pros:

- smallest surface change
- zero schema evolution

Cons:

- does not remove the core repeated-work problem
- long sessions still pay linear read-time cost

### Option B: Materialized summary checkpoint table with lazy catch-up

Add a lightweight checkpoint table keyed by session and maintain summary state
incrementally as turns slide out of the active window.

Pros:

- shifts summary work from read-time to append-time
- turns repeated reads into a cheap checkpoint lookup plus active window query
- supports old sessions with lazy rebuild

Cons:

- requires SQLite schema evolution and write-path maintenance logic

### Option C: Long-lived SQLite engine plus materialized summaries

Add connection reuse, prepared statement reuse, and checkpoint materialization in
one slice.

Pros:

- highest theoretical performance ceiling

Cons:

- too much surface area for one reviewable patch
- verification cost is materially higher

## Decision

Implement Option B now.

Add a new SQLite table:

- `memory_summary_checkpoints`

Proposed fields:

- `session_id TEXT PRIMARY KEY`
- `summarized_through_turn_id INTEGER NOT NULL`
- `summary_body TEXT NOT NULL`
- `summary_budget_chars INTEGER NOT NULL`
- `summary_window_size INTEGER NOT NULL`
- `summary_format_version INTEGER NOT NULL`
- `updated_at_ts INTEGER NOT NULL`

The table stores the deterministic summary body for turns older than the active
window. `summarized_through_turn_id` uses the monotonic SQLite row id frontier so
incremental catch-up can efficiently fold only newly-overflowed turns into the
checkpoint.

## Architecture

### Source of truth

`turns` remains the canonical conversation history. The checkpoint table is a
derived cache, not an authority.

### Write path

On `append_turn` in `window_plus_summary` mode:

1. insert the new turn
2. compute the oldest id that still belongs in the active window
3. fold any newly-overflowed turns into the session checkpoint
4. update `summarized_through_turn_id`

This work happens in the same SQLite transaction as the turn insert so the
checkpoint never advances beyond committed source turns.

### Read path

On `read_context` in `window_plus_summary` mode:

1. load the active window
2. derive the current summary frontier from the oldest active-window turn id
3. load the session checkpoint
4. rebuild lazily only if the checkpoint is missing, stale, or config-incompatible
5. emit:
   - optional profile block
   - optional summary block from `summary_body`
   - active window turns

### Clear path

`clear_session` deletes both source turns and the derived checkpoint row in one
transaction.

## Data Flow Details

### Incremental catch-up

If the current checkpoint frontier is behind the latest summary frontier, query
only:

- turns with `id > summarized_through_turn_id`
- turns with `id < oldest_active_window_turn_id`

Normalize them into deterministic summary lines and append them into the stored
`summary_body`, reapplying the existing body budget cap.

### Lazy rebuild

Rebuild the checkpoint from source turns when:

- the checkpoint row does not exist
- `summary_budget_chars` differs from the current config
- `summary_window_size` differs from the current config
- `summary_format_version` differs from the implementation constant
- the checkpoint frontier is ahead of the current target frontier

This is essential because changing `sliding_window` or `summary_max_chars`
changes which turns belong in the summary and which belong in the active window.

## Error Handling

- Checkpoint maintenance failures must fail the enclosing write transaction.
  Partial success is not acceptable because it would leave persisted turns and
  derived summary state diverged.
- Missing or incompatible checkpoint rows on read are not fatal; they trigger a
  rebuild from source turns.
- `clear_session` removes the checkpoint row opportunistically inside the same
  transaction as turn deletion, preserving cleanup consistency.

## Why This Design

This is the smallest slice that materially improves the memory hot path under
real workload patterns:

- many appends
- many more reads than writes
- repeated prompt hydration against long-running sessions

For low-code and operator-heavy flows this is the right maturity move: the
runtime stops recomputing old state on every read and instead pays a bounded
incremental maintenance cost as history grows.

## Verification

- Add a failing test that proves `append_turn` materializes a summary checkpoint
  once turns overflow the active window.
- Add a failing test that proves changing `sliding_window` rebuilds the
  checkpoint so previously summarized turns can move back into the active window.
- Add a failing test that proves changing `summary_max_chars` rebuilds the
  checkpoint instead of reusing a stale truncated body.
- Add a failing test that proves `clear_session` removes checkpoint state.
- Run targeted memory tests, full workspace tests, and architecture boundary
  checks.
