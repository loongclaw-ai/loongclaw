# SQLite Summary Budget Saturation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove wasted decode and normalization work from SQLite summary materialization once the summary budget is saturated, while preserving frontier advancement and output stability.

**Architecture:** Keep the existing streamed summary materialization shape, but split summary queries onto narrow `id, role, content` SQL, short-circuit per-row formatting after the budget is full, and add thread-scoped test metrics proving the skip path works for both rebuild and catch-up.

**Tech Stack:** Rust, rusqlite, SQLite, cargo test

---

### Task 1: Add red tests for saturation behavior

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`
- Test: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing tests**

Add test-only metrics and two tests:

- rebuild saturation should stop summary formatting after the budget is filled
- catch-up saturation should preserve the existing summary body while advancing
  `summarized_through_turn_id`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_skips_summary_formatting_after_budget_saturation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_advances_frontier_after_budget_saturation_without_reformatting -- --nocapture
```

Expected:

- both tests fail before production code changes

**Step 3: Commit**

```bash
git add crates/app/src/memory/sqlite.rs
git commit -m "test(memory): cover summary budget saturation"
```

### Task 2: Implement summary saturation short-circuit

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the minimal implementation**

- add summary-specific SQL constants selecting `id, role, content`
- update streamed summary queries to use the new SQL
- in `stream_summary_rows`, decode `turn_id` first, update frontier, and skip
  `role/content` decode once the summary budget is full
- move the budget exhaustion guard to the start of `append_summary_line`
- add test-only counters for summary formatting attempts/skips using the current
  thread capture

**Step 2: Run targeted tests to verify they pass**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_skips_summary_formatting_after_budget_saturation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_advances_frontier_after_budget_saturation_without_reformatting -- --nocapture
```

Expected:

- both tests pass

**Step 3: Commit**

```bash
git add crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): short-circuit saturated sqlite summaries"
```

### Task 3: Re-verify existing summary streaming behavior

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs` if needed
- Test: `crates/app/src/memory/sqlite.rs`

**Step 1: Run focused regression tests**

```bash
cargo test -p loongclaw-app window_reads_route_through_cached_statement_preparation -- --nocapture
cargo test -p loongclaw-app summary_append_path_routes_multiple_sqls_through_cached_preparation -- --nocapture
cargo test -p loongclaw-app summary_rebuild_routes_through_streaming_row_accumulation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_routes_through_streaming_row_accumulation -- --nocapture
```

Expected:

- all focused statement-cache and summary-streaming tests pass

**Step 2: Fix only if a regression appears**

- keep changes confined to `crates/app/src/memory/sqlite.rs`

### Task 4: Full verification

**Files:**
- Verify: `crates/app/src/memory/sqlite.rs`

**Step 1: Format**

```bash
cargo fmt --all
```

**Step 2: Run repository-appropriate validation**

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

### Task 5: Final clean commit

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Inspect isolation**

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

**Step 2: Commit implementation**

```bash
git add crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): short-circuit saturated sqlite summaries"
```
