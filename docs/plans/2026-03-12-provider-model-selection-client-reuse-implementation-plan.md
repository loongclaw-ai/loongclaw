# Provider Model Selection Client Reuse Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reuse the same HTTP client across provider model discovery and the subsequent request path in auto-model mode.

**Architecture:** Add client-aware variants of the model-selection helpers and route the existing provider request entrypoints through them, without changing ranking, retry, or fallback behavior.

**Tech Stack:** Rust, reqwest, existing `loongclaw-app` provider modules, cargo tests, architecture boundary checks.

---

### Task 1: Add failing tests for client-aware model-selection entrypoints

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`

**Step 1: Write the failing test**

Add unit tests that call a client-aware ranking/resolution helper boundary and
prove the explicit-model fast path does not depend on any extra client
construction.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app provider::model_selection::tests:: -- --nocapture`

Expected: FAIL because the new helper boundary does not exist yet.

**Step 3: Write minimal implementation**

Only after the tests fail.

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app provider::model_selection::tests:: -- --nocapture`

Expected: PASS.

### Task 2: Reuse the request client during model discovery

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`
- Modify: `crates/app/src/provider/mod.rs`

**Step 1: Write the failing regression test**

Use existing provider tests and the Task 1 unit tests as the regression net.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app provider:: -- --nocapture`

Expected: FAIL or remain red until the client-aware path is wired in correctly.

**Step 3: Write minimal implementation**

Thread `&reqwest::Client` through:

- `resolve_request_models(...)`
- `fetch_available_models_with_policy(...)`
- provider request entrypoints that already build a client

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app provider:: -- --nocapture`

Expected: PASS.

### Task 3: Verify regression and summarize impact

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`
- Modify: `crates/app/src/provider/mod.rs`

**Step 1: Run targeted provider tests**

Run: `cargo test -p loongclaw-app provider:: -- --nocapture`

Expected: PASS.

**Step 2: Run full workspace tests**

Run: `cargo test --workspace --all-features`

Expected: PASS.

**Step 3: Run architecture check**

Run: `./scripts/check_architecture_boundaries.sh`

Expected: PASS with no new boundary regressions.
