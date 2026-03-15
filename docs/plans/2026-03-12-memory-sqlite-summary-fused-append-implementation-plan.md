# SQLite Summary Fused Append Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the scratch normalization buffer from SQLite summary materialization and stream normalized summary text directly into the final summary body without changing output.

**Architecture:** Keep the existing streamed summary row pipeline and UTF-8-safe truncation helper, but refactor summary line construction so whitespace normalization and appending happen in one pass directly into `summary_body`. Use thread-scoped test metrics and explicit formatting regression tests to prove output parity.

**Tech Stack:** Rust, rusqlite, SQLite, cargo test

---

### Task 1: Add failing tests for fused summary append

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`
- Test: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing tests**

Add tests that prove:

- rebuild path no longer uses scratch normalization after summary formatting
- catch-up path no longer uses scratch normalization after budget saturation
- summary formatting remains stable for mixed whitespace and Unicode truncation

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_avoids_scratch_normalization_buffer -- --nocapture
cargo test -p loongclaw-app summary_catch_up_avoids_scratch_normalization_buffer_after_saturation -- --nocapture
cargo test -p loongclaw-app append_summary_line_preserves_whitespace_collapse_and_utf8_safe_truncation -- --nocapture
```

Expected:

- the scratch-buffer tests fail before implementation
- the formatting regression test either fails or exposes any behavioral drift

### Task 2: Implement direct token streaming

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the minimal implementation**

- remove the per-row scratch `String` from `stream_summary_rows(...)`
- remove the scratch parameter from `append_summary_line(...)`
- replace scratch normalization with a direct token-streaming helper
- preserve existing line prefixes and UTF-8-safe truncation by reusing
  `append_truncated_summary_fragment(...)`
- keep thread-scoped metrics intact and expose whether scratch normalization is
  still used

**Step 2: Run the targeted tests to verify they pass**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_avoids_scratch_normalization_buffer -- --nocapture
cargo test -p loongclaw-app summary_catch_up_avoids_scratch_normalization_buffer_after_saturation -- --nocapture
cargo test -p loongclaw-app append_summary_line_preserves_whitespace_collapse_and_utf8_safe_truncation -- --nocapture
```

Expected:

- all three tests pass

### Task 3: Re-run existing summary regressions

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs` if a regression appears

**Step 1: Run focused summary/cache tests**

```bash
cargo test -p loongclaw-app summary_rebuild_routes_through_streaming_row_accumulation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_routes_through_streaming_row_accumulation -- --nocapture
cargo test -p loongclaw-app summary_rebuild_skips_summary_formatting_after_budget_saturation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_advances_frontier_after_budget_saturation_without_reformatting -- --nocapture
cargo test -p loongclaw-app summary_append_path_routes_multiple_sqls_through_cached_preparation -- --nocapture
```

Expected:

- all focused regressions pass

### Task 4: Full verification

**Files:**
- Verify: `crates/app/src/memory/sqlite.rs`

**Step 1: Format**

```bash
cargo fmt --all
```

**Step 2: Run repository verification**

```bash
cargo test -p loongclaw-app memory:: -- --nocapture
cargo test -p loongclaw-app provider:: -- --nocapture
cargo test --workspace --all-features
./scripts/check_architecture_boundaries.sh
```

Expected:

- formatting clean
- targeted suites pass
- workspace passes
- architecture boundary script passes

### Task 5: Commit the implementation cleanly

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Inspect isolation**

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

**Step 2: Commit**

```bash
git add crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): fuse sqlite summary append"
```
