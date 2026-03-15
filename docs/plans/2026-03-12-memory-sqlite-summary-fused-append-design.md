# SQLite Summary Fused Append Design

## Context

`crates/app/src/memory/sqlite.rs` now streams summary rows directly from SQLite
and short-circuits row payload decoding once the summary budget is saturated.
That removed the largest post-budget waste, but the remaining pre-saturation hot
path still does more work than needed:

- `stream_summary_rows(...)` allocates and reuses a scratch `String` for every
  summarized row
- `append_summary_line(...)` normalizes content into that scratch buffer and
  then re-reads the normalized text to append it into `summary_body`
- this creates a second full pass over normalized content and extra writes to a
  temporary buffer for each summarized turn before the budget is filled

In the current code the output is correct, but the text path still pays a
double-handling cost that is unnecessary once we already have a UTF-8-aware
truncating append primitive.

## Goals

- Preserve external memory API, checkpoint schema, and summary text output
- Remove the per-row scratch normalization buffer from summary materialization
- Keep UTF-8-safe truncation semantics unchanged
- Keep budget saturation behavior and frontier advancement unchanged
- Add regression coverage proving whitespace collapse and truncation behavior

## Non-Goals

- No schema changes
- No read-path contract changes
- No benchmark harness expansion in this slice
- No changes to active window loading logic

## Approaches Considered

### Option 1: Fused normalize-and-append using existing truncation primitive

- Keep `append_truncated_summary_fragment(...)`
- Replace `normalize_summary_content_into(...)` with a streaming routine that
  collapses whitespace and appends tokens directly into `summary_body`
- Remove the scratch `String` from `stream_summary_rows(...)` and
  `append_summary_line(...)`

Pros:
- Small, local diff
- Removes the remaining extra text buffer and second content pass
- Preserves current UTF-8 truncation behavior by reusing the existing append
  helper

Cons:
- Requires careful regression testing around whitespace collapse and line
  prefixes

### Option 2: Manual byte-level writer for the entire summary line

- Build a dedicated byte-oriented writer that owns both normalization and
  truncation in one routine

Pros:
- Maximal control over copies and branch behavior

Cons:
- Larger behavioral surface area
- Harder to reason about and review
- Unnecessary while the current append helper already handles UTF-8-safe
  truncation correctly

### Option 3: Keep scratch normalization and only pre-size it more aggressively

- Retain `normalized_content` but try to reduce reallocations further

Pros:
- Lowest code churn

Cons:
- Keeps the extra buffer and second-pass writes intact
- Lowest leverage of the three options

## Decision

Choose Option 1.

The current hotspot is no longer row materialization itself, but temporary text
materialization before bytes are copied into `summary_body`. Fusing
normalization into the append path removes that temporary state while keeping
the existing truncation primitive, which gives the best optimization-to-risk
ratio for this stage of the refactor.

## Design Details

### 1. Direct token streaming into `summary_body`

`append_summary_line(...)` will:

1. reserve remaining budget
2. append optional line prefix fragments (`\n`, `- `, role, `: `)
3. stream `content.split_whitespace()` tokens directly into `summary_body`
4. insert single spaces between tokens
5. stop naturally through `append_truncated_summary_fragment(...)` once the
   budget is exhausted

This preserves the current output contract:

- all runs of whitespace collapse to single spaces
- no leading/trailing content whitespace survives
- truncation stays UTF-8 safe

### 2. Remove scratch normalization buffer

- delete the `normalized_content` scratch variable from `stream_summary_rows(...)`
- remove the extra parameter from `append_summary_line(...)`
- replace `normalize_summary_content_into(...)` with a streaming helper that
  only returns whether any non-whitespace token existed

### 3. Test-only observability

Keep the thread-scoped SQLite metric capture introduced in the previous slice
and add a narrow regression signal proving the scratch normalization routine is
no longer used after the refactor.

The tests should prove:

- rebuild path no longer uses scratch normalization
- catch-up path no longer uses scratch normalization
- whitespace collapse and Unicode-safe truncation still produce stable text

## Validation Strategy

- Red-green targeted tests for scratch-buffer elimination
- Regression test for exact summary text formatting with mixed whitespace and
  multi-byte Unicode
- Existing summary streaming/cache tests
- `cargo test -p loongclaw-app memory:: -- --nocapture`
- `cargo test -p loongclaw-app provider:: -- --nocapture`
- `cargo test --workspace --all-features`
- `./scripts/check_architecture_boundaries.sh`

## Expected Impact

- Less temporary string traffic while filling the summary budget
- Lower CPU cost from removing the normalize-then-copy double pass
- No external behavior changes
- Cleaner summary materialization pipeline that is closer to the minimal
  low-level work required for each summarized row
