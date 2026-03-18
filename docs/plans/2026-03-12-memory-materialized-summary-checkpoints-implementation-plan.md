# Memory Materialized Summary Checkpoints Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Materialize deterministic summary checkpoints for `window_plus_summary` so repeated memory reads no longer rebuild older-session summaries from scratch.

**Architecture:** Keep `turns` as the source-of-truth table and add a derived SQLite checkpoint table keyed by session. `append_turn` incrementally advances the checkpoint inside the same transaction, `read_context` lazily rebuilds only when the checkpoint is missing or config-incompatible, and `clear_session` removes both source and derived state together.

**Tech Stack:** Rust, rusqlite, serde_json

---

### Task 1: Lock the checkpoint contract with failing tests

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`
- Modify: `crates/app/src/memory/context.rs`

**Step 1: Write the failing checkpoint materialization test**

Add a SQLite test that appends enough turns to overflow the active window and
then asserts a summary checkpoint row exists for the session.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::append_turn_materializes_summary_checkpoint_once_window_overflows -- --exact --nocapture
```

Expected: FAIL because no checkpoint table or row exists yet.

**Step 3: Write the failing rebuild tests**

Add focused tests proving:

- changing `sliding_window` rebuilds checkpoint boundaries
- changing `summary_max_chars` rebuilds checkpoint text
- `clear_session` removes checkpoint state

**Step 4: Run the focused failing tests**

Run the exact test commands and confirm each fails for the expected missing
checkpoint behavior.

### Task 2: Add SQLite checkpoint schema and state helpers

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Add schema support**

Extend SQLite initialization with `memory_summary_checkpoints`.

**Step 2: Add internal checkpoint structs and helpers**

Implement helpers for:

- loading a checkpoint row
- upserting a checkpoint row
- deleting a checkpoint row
- computing the current summary frontier from the active window
- rebuilding the checkpoint from source turns

**Step 3: Add deterministic summary-body helpers**

Centralize:

- turn-content normalization
- summary line formatting
- body append/retrim behavior

### Task 3: Route write/read/clear paths through the checkpoint

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`
- Modify: `crates/app/src/memory/context.rs`

**Step 1: Update append path**

Wrap `append_turn` in a transaction and incrementally advance the summary
checkpoint when the active window overflows in `window_plus_summary` mode.

**Step 2: Update read snapshot path**

Change `ContextSnapshot` so it carries materialized summary content instead of
raw `older_turns`, and make `load_context_snapshot(...)` use the checkpoint with
lazy rebuilds.

**Step 3: Update prompt-context hydration**

Build the summary block directly from the materialized checkpoint body while
preserving current outward message semantics.

**Step 4: Update clear path**

Delete checkpoint rows alongside source turns.

### Task 4: Verify targeted and full behavior

**Files:**
- No new files unless verification reveals a regression

**Step 1: Run targeted memory tests**

```bash
cargo test -p loongclaw-app memory:: -- --nocapture
```

Expected: PASS

**Step 2: Run provider regression guard**

```bash
cargo test -p loongclaw-app provider:: -- --nocapture
```

Expected: PASS

**Step 3: Run full workspace verification**

```bash
cargo test --workspace --all-features
```

Expected: PASS

**Step 4: Run architecture boundary checks**

```bash
./scripts/check_architecture_boundaries.sh
```

Expected: `memory_mod` and `provider_mod` remain within budget
