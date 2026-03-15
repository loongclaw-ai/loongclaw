# Memory Local Fast Path Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove local memory protocol overhead by making `append_turn_direct(...)` and `window_direct*` use typed SQLite helpers instead of building requests and re-entering core dispatch.

**Architecture:** Keep `execute_memory_core_with_config(...)` as the stable request boundary, but refactor the SQLite backend so request handlers parse once and delegate to typed internal helpers. Local direct helpers call those typed helpers directly and bypass JSON/request construction.

**Tech Stack:** Rust, rusqlite, serde_json

---

### Task 1: Lock no-dispatch behavior with failing tests

**Files:**
- Modify: `crates/app/src/memory/mod.rs`
- Modify: `crates/app/src/memory/tests.rs`

**Step 1: Add test-only dispatch instrumentation**

Track how many times `execute_memory_core_with_config(...)` is entered during
tests.

**Step 2: Write the failing append direct-path test**

Assert `append_turn_direct(...)` completes without incrementing the core
dispatch counter.

**Step 3: Run the append test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::tests::append_turn_direct_bypasses_core_dispatch -- --exact --nocapture
```

Expected: FAIL because the current implementation re-enters core dispatch.

**Step 4: Write the failing window direct-path test**

Assert `window_direct(...)` completes without incrementing the core dispatch
counter.

**Step 5: Run the window test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app memory::tests::window_direct_bypasses_core_dispatch -- --exact --nocapture
```

Expected: FAIL because the current implementation still re-enters core
dispatch.

### Task 2: Extract typed internal helpers

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Extract typed append helper**

Move the actual append logic into a typed helper that accepts validated
arguments and returns the same data needed by both direct and request paths.

**Step 2: Extract typed window helper**

Move the actual recent-turn loading logic into a typed helper returning
`Vec<ConversationTurn>`.

**Step 3: Keep request handlers as adapters**

Make request-shaped handlers parse payload, validate inputs, call typed helpers,
and preserve the existing outcome payload format.

### Task 3: Route direct helpers through typed fast paths

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Update append direct helper**

Call the typed append helper directly instead of building a request.

**Step 2: Update window direct helpers**

Call the typed window helper directly and skip payload decode.

### Task 4: Verify regressions

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
