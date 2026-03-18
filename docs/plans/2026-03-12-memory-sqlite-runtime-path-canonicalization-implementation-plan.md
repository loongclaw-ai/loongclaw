# Memory SQLite Runtime Path Canonicalization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Canonicalize SQLite runtime paths so one database file maps to one process-local runtime even when callers use different path spellings.

**Architecture:** Add a best-effort runtime path normalizer inside the SQLite memory backend. Use the normalized path as the registry key, runtime path, and test bootstrap-counter key so startup and hot-path lookups converge on one runtime identity.

**Tech Stack:** Rust, std::fs, std::path, rusqlite

---

### Task 1: Lock same-file alias reuse with failing tests

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing relative-vs-absolute path test**

Add a test that points two configs at the same DB using relative and absolute
paths and asserts only one runtime bootstrap occurs.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::equivalent_relative_and_absolute_paths_share_one_runtime -- --exact --nocapture
```

Expected: FAIL because raw `PathBuf` keys still create multiple runtimes.

**Step 3: Write the failing dot-dot normalization test**

Add a test that points two configs at the same DB using a path containing `..`
and the normalized path and asserts only one runtime bootstrap occurs.

**Step 4: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::dot_dot_aliases_share_one_runtime_after_normalization -- --exact --nocapture
```

Expected: FAIL because the current registry treats the aliases as distinct keys.

### Task 2: Implement normalized runtime path identity

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Add a runtime-path normalization helper**

Normalize file paths using canonicalization when available and stable fallback
absolutization when the DB file does not yet exist.

**Step 2: Route runtime acquisition through the normalized path**

Make the registry key, runtime path, and startup readiness path all use the
same normalized value.

**Step 3: Update test helpers**

Make bootstrap counters and runtime-reset hooks use normalized paths too, so
tests assert the real cache identity.

### Task 3: Verify regressions

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
