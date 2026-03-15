# SQLite Summary Budget Saturation Design

## Context

`crates/app/src/memory/sqlite.rs` has already removed the buffered
`Vec<IndexedConversationTurn>` rebuild path for materialized summaries. The
remaining hot path waste is now inside streamed summary materialization:

- summary rebuild and catch-up still decode `role` and `content` for every row
  after the summary budget has already been filled
- `append_summary_line` still performs whitespace normalization before checking
  whether any budget remains
- summary-specific queries still fetch `ts`, even though streamed summary
  materialization only consumes `id`, `role`, and `content`

In long-history sessions this creates avoidable CPU work and string churn after
the summary is already saturated, while the checkpoint frontier still has to
advance to the newest summarized turn.

## Goals

- Preserve the external memory API and current checkpoint schema
- Keep summary text output unchanged for non-saturated and saturated cases
- Advance `summarized_through_turn_id` exactly as before
- Eliminate unnecessary Rust-side decode and normalization work once the budget
  is full
- Narrow the SQLite row payload used by summary materialization

## Non-Goals

- No schema migration
- No read-time-only summary generation
- No change to active window queries
- No public configuration changes

## Options Considered

### Option 1: Saturation short-circuit with summary-only SQL

- Add summary-specific SQL that selects `id, role, content`
- In the streaming loop, always decode `id` first to maintain the frontier
- Once `summary_body.len() >= summary_budget_chars`, stop decoding
  `role/content` and skip summary formatting for remaining rows
- Also move the budget guard to the start of `append_summary_line`

Pros:
- Small diff
- Removes the most expensive useless work in long-history sessions
- Keeps the current materialization contract unchanged

Cons:
- Still leaves per-line normalization as a separate pass before append when the
  budget is not yet full

### Option 2: Fully fused normalize-and-append pipeline

- Stream tokens directly into `summary_body` without using a scratch string

Pros:
- Potentially lower allocation pressure on every summarized turn

Cons:
- Larger behavioral surface area
- Harder to prove output parity around truncation and UTF-8 boundaries
- Higher risk for a continuation slice that already has in-flight refactors

### Option 3: Post-budget frontier-only query

- Stop reading full rows once saturated and perform a second `MAX(id)`-style
  query to advance the frontier

Pros:
- Avoids even the row iteration after saturation

Cons:
- Adds extra query complexity and branchy SQL logic
- Lower leverage than removing Rust-side decoding/normalization first

## Decision

Choose Option 1.

This gives the best cost/benefit ratio for the current refactor sequence:

- it removes the hottest remaining waste immediately
- it keeps the frontier semantics intact
- it composes cleanly with the existing streaming materialization changes
- it leaves a future fused normalize-and-append optimization available if
  profiling still shows summary formatting as the next bottleneck

## Design Details

### 1. Summary-specific SQL

Introduce summary-only SQL constants for:

- turns up to a target id
- turns between checkpoint frontier and the active window boundary

These queries will select only `id, role, content`. The existing window query
continues to select `ts` because the active window still needs full
`ConversationTurn` values.

### 2. Budget saturation short-circuit

`stream_summary_rows(...)` will change its row handling order:

1. read `turn_id`
2. update `latest_turn_id`
3. if `summary_body.len() >= summary_budget_chars`, skip the rest of the row
4. otherwise decode `role` and `content`
5. append the summary line

This preserves checkpoint frontier advancement even when the text body is
already full.

### 3. Early budget guard in summary formatting

`append_summary_line(...)` will immediately return when there is no remaining
budget before any normalization work begins.

The existing scratch buffer and truncation behavior stay intact for this slice.

### 4. Test-only observability

Add current-thread-scoped summary formatting metrics so tests can prove:

- rebuild path stops formatting once the budget is full
- catch-up path still advances the checkpoint frontier without mutating the
  summary text
- parallel tests do not pollute metric assertions

## Validation Strategy

- Red-green tests for rebuild saturation and catch-up saturation
- Focused memory tests for checkpoint rebuild/catch-up regressions
- Full `loongclaw-app` memory and provider test suites
- Full workspace test run
- Architecture boundary script

## Expected Impact

- Lower per-row CPU cost for long-history rebuild/catch-up paths
- Reduced temporary string writes after the summary body is saturated
- Lower row decode overhead by removing unused `ts` from summary materialization
- No external behavior or schema changes
