# Memory SQLite Read Path Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce repeated SQLite opens and repeated context reads for `read_context` while preserving existing prompt-context behavior.

**Architecture:** Add a shared SQLite connection helper plus a `ContextSnapshot` helper that reads the active window and older summary-source turns through one prepared connection path. Keep the public `memory::` API stable and only refactor internal SQLite/context plumbing.

**Tech Stack:** Rust, rusqlite, existing `loongclaw-app` memory modules, cargo tests, architecture boundary checks.

---

### Task 1: Add failing tests for SQLite context snapshots

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add tests for a new internal snapshot helper asserting:

- active window turns remain in chronological order
- older turns exclude the active window
- sessions shorter than the window produce no older summary-source turns

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app memory::sqlite::tests::context_snapshot -- --nocapture`

Expected: FAIL because the helper does not exist yet.

**Step 3: Write minimal implementation**

Only after the tests fail.

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app memory::sqlite::tests::context_snapshot -- --nocapture`

Expected: PASS.

### Task 2: Route prompt-context loading through the new snapshot path

**Files:**
- Modify: `crates/app/src/memory/context.rs`
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing regression test**

Extend memory-context regression coverage so `window_plus_summary` still emits:

- one summary entry for older turns
- one turn entry per active window turn
- no duplication of the active window inside the summary source

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app memory::context::tests::window_plus_summary_includes_condensed_older_context -- --exact`

Expected: FAIL once the test is tightened against the new snapshot contract.

**Step 3: Write minimal implementation**

Make `load_prompt_context(...)` consume the SQLite snapshot helper instead of
 stitching together independent window/full-session reads.

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app memory::context::tests::window_plus_summary_includes_condensed_older_context -- --exact`

Expected: PASS.

### Task 3: Collapse repeated SQLite setup into a shared helper

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Use the Task 1 snapshot tests and existing direct read/write tests as the safety
 net.

**Step 2: Run test to verify it fails or stays red until implementation lands**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: FAIL or remain red until the shared helper is wired in correctly.

**Step 3: Write minimal implementation**

Add:

- one shared SQLite connection/schema helper
- connection-scoped query helpers reused by append/window/clear/context reads

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS.

### Task 4: Verify regression and architecture status

**Files:**
- Modify: `crates/app/src/memory/context.rs`
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Run targeted tests**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS.

**Step 2: Run full workspace tests**

Run: `cargo test --workspace --all-features`

Expected: PASS.

**Step 3: Run architecture check**

Run: `./scripts/check_architecture_boundaries.sh`

Expected: PASS with `memory_mod` still inside budget and no new boundary regressions from the SQLite refactor.
