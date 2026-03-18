# Memory SQLite Prepared Statement Cache Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reuse prepared SQLite statements on the long-lived memory connection so repeated memory reads and writes avoid redundant SQL compilation.

**Architecture:** Keep the current per-path `SqliteRuntime` and its single long-lived `rusqlite::Connection`, but move stable SQL operations onto `prepare_cached` and configure a small explicit statement cache on connection setup. Add test-only connection-handle inspection so the optimization is observable in regression tests instead of being a hidden implementation detail.

**Tech Stack:** Rust, `rusqlite`, SQLite prepared statement cache, existing `memory/sqlite.rs` test module.

---

### Task 1: Add a failing observability test for cached window statements

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add a test-only helper that counts prepared statements resident on the runtime connection via the SQLite handle, then add a test proving a simple `window_direct_with_options(...)` call leaves one cached statement on the reused connection.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app window_reads_leave_cached_statement_resident_on_runtime -- --nocapture`

Expected: FAIL because the current implementation finalizes the prepared statement instead of returning it to a cache.

**Step 3: Do not change production code yet**

Keep this task red until the failure clearly points at missing prepared statement reuse.

**Step 4: Commit later with implementation**

Do not commit during the red phase.

### Task 2: Add a failing observability test for summary/materialization paths

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add a second regression test for `WindowPlusSummary` mode that appends enough turns to trigger summary materialization, then asserts the runtime connection retains multiple cached statements after the write path completes.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app summary_append_path_warms_multiple_cached_statements -- --nocapture`

Expected: FAIL because the current append/materialization path still recompiles each stable SQL statement and leaves nothing cached.

**Step 3: Keep scope narrow**

Do not expand into benchmarks or unrelated refactors in this slice.

### Task 3: Implement prepared statement caching with minimal surface area

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Configure connection cache capacity**

Set a small explicit statement-cache capacity when configuring a new SQLite connection so the runtime has predictable steady-state behavior.

**Step 2: Convert hot read queries**

Replace the repeated `prepare(...)` calls in recent-window and summary query helpers with `prepare_cached(...)`.

**Step 3: Convert stable write queries**

Route append, clear, summary checkpoint upsert, and checkpoint delete through cached prepared statements while preserving the current transaction boundaries and error mapping.

**Step 4: Keep public API unchanged**

Do not alter exported memory function signatures or result shapes.

### Task 4: Verify green on targeted tests

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Run the targeted regression tests**

Run:

```bash
cargo test -p loongclaw-app window_reads_leave_cached_statement_resident_on_runtime -- --nocapture
cargo test -p loongclaw-app summary_append_path_warms_multiple_cached_statements -- --nocapture
```

Expected: PASS.

**Step 2: Run the memory test slice**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS with no regressions in runtime reuse, summary materialization, or direct-path bypass behavior.

### Task 5: Run full verification and commit

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Format**

Run: `cargo fmt --all`

**Step 2: Run repository verification**

Run:

```bash
cargo test -p loongclaw-app provider:: -- --nocapture
cargo test --workspace --all-features
./scripts/check_architecture_boundaries.sh
```

**Step 3: Inspect commit isolation**

Run:

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

Expected: only the prepared-statement-cache slice is staged.

**Step 4: Commit**

```bash
git add docs/plans/2026-03-12-memory-sqlite-prepared-statement-cache-design.md docs/plans/2026-03-12-memory-sqlite-prepared-statement-cache-implementation-plan.md crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): cache sqlite prepared statements"
```
