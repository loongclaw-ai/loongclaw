# Memory SQLite Runtime Reuse Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reuse a process-local SQLite runtime per database path so memory reads and writes stop reopening and reinitializing SQLite on every operation.

**Architecture:** Add a per-path runtime registry that lazily creates and caches one long-lived `rusqlite::Connection` for each resolved SQLite path. Route all memory operations through serialized runtime helpers so connection bootstrapping, PRAGMA setup, and schema initialization happen once per path while existing transaction semantics remain unchanged.

**Tech Stack:** Rust, rusqlite, std::sync

---

### Task 1: Lock runtime-reuse behavior with failing tests

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing same-path reuse test**

Add a focused test that performs multiple memory operations against one SQLite
path and asserts the runtime bootstrap counter increments only once.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::memory_operations_reuse_cached_sqlite_runtime_for_same_path -- --exact --nocapture
```

Expected: FAIL because the current code opens a new connection for each
operation.

**Step 3: Write the failing distinct-path isolation test**

Add a test proving two different SQLite paths get distinct runtime bootstrap
counts.

**Step 4: Run the distinct-path test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::distinct_sqlite_paths_get_distinct_runtime_bootstraps -- --exact --nocapture
```

Expected: FAIL because there is no runtime registry yet.

**Step 5: Write the failing runtime-recreation test**

Add a test-only reset path and assert the next access recreates the runtime for
the same SQLite path.

**Step 6: Run the recreation test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::sqlite::tests::resetting_cached_runtime_forces_runtime_recreation_on_next_access -- --exact --nocapture
```

Expected: FAIL because the runtime cache does not exist yet.

### Task 2: Add the SQLite runtime registry and one-time bootstrapping

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Add runtime structs and registry**

Implement:

- a process-local registry keyed by resolved `PathBuf`
- a runtime struct owning one `rusqlite::Connection`
- serialized read/write execution helpers

**Step 2: Move connection bootstrap into runtime creation**

Centralize:

- directory creation
- `Connection::open(...)`
- PRAGMA setup
- schema initialization

**Step 3: Add test-only instrumentation**

Track runtime bootstrap counts so the new tests can prove reuse and recreation
without changing the production API.

### Task 3: Route memory operations through the cached runtime

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Update startup initialization**

Make `ensure_memory_db_ready(...)` acquire the runtime instead of opening a
throwaway connection.

**Step 2: Update read operations**

Route `load_window(...)` and `load_context_snapshot(...)` through runtime read
helpers.

**Step 3: Update write operations**

Route `append_turn(...)` and `clear_session(...)` through runtime write helpers,
preserving existing transaction boundaries and checkpoint maintenance.

### Task 4: Verify behavior and regressions

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
