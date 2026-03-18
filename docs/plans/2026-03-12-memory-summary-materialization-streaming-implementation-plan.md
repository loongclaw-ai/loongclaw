# Memory Summary Materialization Streaming Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove intermediate bulk turn materialization from summary rebuild and catch-up so summary mode uses less CPU and heap when session history grows.

**Architecture:** Keep the existing checkpoint model and summary semantics, but replace summary-specific row collection with streaming helpers that append directly into the target summary buffer while rows are read from SQLite. The plan keeps public APIs unchanged and uses test-only counters to prove summary paths no longer rely on intermediate vectors.

**Tech Stack:** Rust, `rusqlite`, SQLite cached statements, existing `memory/sqlite.rs` tests.

---

### Task 1: Add a failing rebuild-path regression test

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add test-only counters for summary bulk materialization, then add a test that triggers summary rebuild through `load_context_snapshot(...)` after a window-size change and asserts the rebuild path performs zero bulk summary row materializations.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app summary_rebuild_path_avoids_bulk_turn_materialization -- --nocapture`

Expected: FAIL because the current rebuild path still calls the bulk collection helper.

### Task 2: Add a failing catch-up-path regression test

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add a second test that establishes an existing summary checkpoint, resets the counter, appends one more turn, and asserts the catch-up path performs zero bulk summary row materializations.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app summary_catch_up_path_avoids_bulk_turn_materialization -- --nocapture`

Expected: FAIL because the current catch-up path still collects a `Vec<IndexedConversationTurn>`.

### Task 3: Implement streaming summary materialization

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Add a streaming summary helper**

Create a helper that:

- prepares the summary query
- iterates rows once
- appends normalized summary lines directly into a mutable summary string
- tracks the last seen turn id

**Step 2: Convert rebuild path**

Update `rebuild_summary_checkpoint(...)` to use the streaming helper instead of `query_turns_up_to_id(...) -> Vec -> build_summary_body(...)`.

**Step 3: Convert catch-up path**

Update the incremental summary catch-up branch in `materialize_summary_checkpoint(...)` to stream rows directly into the existing summary body and update the frontier from the streaming result.

**Step 4: Reserve summary buffer capacity**

Reserve the summary buffer near the configured summary budget so large histories do not repeatedly grow the target string.

### Task 4: Verify targeted green

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Run the new regression tests**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_path_avoids_bulk_turn_materialization -- --nocapture
cargo test -p loongclaw-app summary_catch_up_path_avoids_bulk_turn_materialization -- --nocapture
```

Expected: PASS.

**Step 2: Run the memory slice**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS with no behavior regressions in summary formatting, checkpoint rebuild, or runtime reuse.

### Task 5: Run full verification and commit

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Format**

Run: `cargo fmt --all`

**Step 2: Full verification**

Run:

```bash
cargo test -p loongclaw-app provider:: -- --nocapture
cargo test --workspace --all-features
./scripts/check_architecture_boundaries.sh
```

**Step 3: Verify commit isolation**

Run:

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

Expected: only this summary-streaming slice is staged.

**Step 4: Commit**

```bash
git add docs/plans/2026-03-12-memory-summary-materialization-streaming-design.md docs/plans/2026-03-12-memory-summary-materialization-streaming-implementation-plan.md crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): stream summary materialization"
```
