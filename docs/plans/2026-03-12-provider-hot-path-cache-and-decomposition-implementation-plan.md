# Provider Hot Path Cache And Decomposition Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce repeated auto-model discovery overhead and split provider hot-path orchestration into focused modules without changing behavior.

**Architecture:** Keep `provider/mod.rs` as the public orchestration surface, move model-list caching and request-state-machine internals into focused submodules, and validate with targeted regression tests plus full repository verification.

**Tech Stack:** Rust, reqwest, tokio, serde_json, sha2

---

### Task 1: Lock the cache behavior with a failing test

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`

**Step 1: Write the failing test**

Add a test that serves a model list exactly once from a local TCP listener, then calls `fetch_available_models_with_policy(...)` twice.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app provider::model_selection::tests::fetch_available_models_with_policy_reuses_recent_model_list_without_refetching -- --exact`

Expected: FAIL on the second fetch because the implementation still re-requests the network endpoint.

**Step 3: Write minimal implementation**

Add an in-process cache for model-list results keyed by endpoint and request identity.

**Step 4: Run test to verify it passes**

Run the same targeted command and confirm the second fetch succeeds without a second network round trip.

### Task 2: Make cache scope and memory behavior explicit

**Files:**
- Create: `crates/app/src/provider/model_cache.rs`
- Modify: `crates/app/src/provider/model_selection.rs`

**Step 1: Add cache key and storage**

Implement:

- endpoint + header fingerprint keying
- 60-second TTL
- 32-entry cap
- oldest-entry eviction after expired-entry pruning

**Step 2: Add regression coverage**

Cover:

- credential-separated cache entries
- explicit invalidation helper behavior for internal cache maintenance

**Step 3: Wire model selection through the cache**

Read cache before remote fetch and write cache only after successful model-list resolution.

**Step 4: Verify focused tests**

Run: `cargo test -p loongclaw-app provider::model_selection:: -- --nocapture`

Expected: all model-selection tests pass.

### Task 3: Decompose provider request hot path

**Files:**
- Create: `crates/app/src/provider/messages.rs`
- Create: `crates/app/src/provider/request_context.rs`
- Create: `crates/app/src/provider/completion.rs`
- Create: `crates/app/src/provider/turn.rs`
- Modify: `crates/app/src/provider/mod.rs`

**Step 1: Extract provider message assembly**

Move system-message building and memory-window projection to `messages.rs`.

**Step 2: Extract request context primitives**

Move shared request client/context/error definitions to `request_context.rs`.

**Step 3: Extract completion and turn retry loops**

Move the completion state machine to `completion.rs` and the tool-capable turn state machine to `turn.rs`.

**Step 4: Keep `provider/mod.rs` as orchestration only**

Retain public request entrypoints and existing exports.

### Task 4: Verify repository behavior and architecture constraints

**Files:**
- No additional code changes required unless verification reveals regressions

**Step 1: Run provider regression suite**

Run: `cargo test -p loongclaw-app provider:: -- --nocapture`

Expected: PASS

**Step 2: Run full workspace verification**

Run: `cargo test --workspace --all-features`

Expected: PASS

**Step 3: Run architecture boundary check**

Run: `./scripts/check_architecture_boundaries.sh`

Expected: `provider_mod` and `memory_mod` both within budget
