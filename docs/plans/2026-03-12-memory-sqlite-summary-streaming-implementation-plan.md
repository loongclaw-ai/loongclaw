# Memory SQLite Summary Streaming Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove avoidable `Vec` and string churn from SQLite summary materialization while keeping memory behavior and checkpoint output unchanged.

**Architecture:** Keep the current `WindowPlusSummary` semantics, schema, and public API, but refactor summary rebuild/catch-up into streaming row consumers that update summary text and frontier state in one pass. Pair that with small string-capacity and whitespace-compaction improvements so the summary path allocates less under long histories.

**Tech Stack:** Rust, `rusqlite`, SQLite prepared statements, existing `crates/app/src/memory/sqlite.rs` tests.

---

### Task 1: Add a failing test proving summary rebuild no longer needs buffered turn vectors

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add a test-only metric around the summary-only row helpers so rebuild/catch-up paths can assert they were consumed through the new streaming helpers instead of the old vector-returning helpers.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app summary_rebuild_routes_through_streaming_row_accumulation -- --nocapture`

Expected: FAIL because rebuild currently uses `query_turns_up_to_id(...) -> Vec<IndexedConversationTurn>`.

**Step 3: Keep production logic unchanged**

Do not touch rebuild code until the test is red for the right reason.

### Task 2: Add a failing test for catch-up streaming

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add a regression test that exercises summary checkpoint catch-up after more turns are appended, then asserts the path routed through streaming row accumulation instead of the old buffered delta helper.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app summary_catch_up_routes_through_streaming_row_accumulation -- --nocapture`

Expected: FAIL because catch-up currently buffers rows into a vector.

### Task 3: Implement streaming summary row consumers

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Add streaming row helpers**

Create summary-specific row-consumption helpers that:

- execute the prepared query
- iterate rows once
- update summary output incrementally
- track the last processed turn id

**Step 2: Move rebuild path to streaming**

Refactor `rebuild_summary_checkpoint(...)` to use the streaming helper instead of `query_turns_up_to_id(...) -> build_summary_body(...)`.

**Step 3: Move catch-up path to streaming**

Refactor the `materialize_summary_checkpoint(...)` catch-up branch to stream new rows directly into the checkpoint body and frontier instead of materializing a delta vector.

### Task 4: Remove avoidable summary string churn

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Preallocate summary body**

Initialize summary buffers with a capacity derived from `summary_budget_chars`.

**Step 2: Rewrite whitespace compaction**

Replace `split_whitespace().collect::<Vec<_>>().join(" ")` with a direct compaction helper that avoids the intermediate vector.

**Step 3: Preserve output semantics**

Keep the same visible summary format and truncation behavior.

### Task 5: Verify targeted and slice-level tests

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Run targeted regression tests**

Run:

```bash
cargo test -p loongclaw-app summary_rebuild_routes_through_streaming_row_accumulation -- --nocapture
cargo test -p loongclaw-app summary_catch_up_routes_through_streaming_row_accumulation -- --nocapture
```

Expected: PASS.

**Step 2: Run memory slice**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS with no changes in summary correctness tests.

### Task 6: Run full verification and commit

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Format**

Run: `cargo fmt --all`

**Step 2: Run full verification**

Run:

```bash
cargo test -p loongclaw-app provider:: -- --nocapture
cargo test --workspace --all-features
./scripts/check_architecture_boundaries.sh
```

**Step 3: Inspect isolation**

Run:

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

Expected: only the summary-streaming slice is staged.

**Step 4: Commit**

```bash
git add docs/plans/2026-03-12-memory-sqlite-summary-streaming-design.md docs/plans/2026-03-12-memory-sqlite-summary-streaming-implementation-plan.md crates/app/src/memory/sqlite.rs
git commit -m "refactor(memory): stream sqlite summary materialization"
```
