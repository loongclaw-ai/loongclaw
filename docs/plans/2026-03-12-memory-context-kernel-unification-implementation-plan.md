# Memory Context Kernel Unification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make kernel-routed conversation message building hydrate the same memory context as the non-kernel provider path.

**Architecture:** Add a new additive memory-core prompt-context operation, route kernel-backed message building through it, and decode the returned typed memory context entries into provider messages. Keep the first slice limited to semantic alignment and architecture cleanup.

**Tech Stack:** Rust, serde/serde_json, existing `loongclaw-app` memory/conversation runtime modules, cargo tests, architecture boundary checks.

---

### Task 1: Add failing tests for kernel-routed prompt context

**Files:**
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing test**

Add tests that assert:

- `window_plus_summary` returns a summary system message when `kernel_ctx` is present
- `profile_plus_window` returns a profile system message when `kernel_ctx` is present

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app build_messages_routes_prompt_context_through_kernel -- --nocapture`

Expected: FAIL because kernel-routed path still requests only `window`.

**Step 3: Write minimal implementation**

Only after the tests fail.

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app build_messages_routes_prompt_context_through_kernel -- --nocapture`

Expected: PASS.

### Task 2: Add an additive memory-core prompt-context operation

**Files:**
- Modify: `crates/app/src/memory/mod.rs`
- Modify: `crates/app/src/memory/sqlite.rs`

**Step 1: Write the failing test**

Add decode/execute tests for a new prompt-context operation returning typed
entries.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: FAIL because the new operation is not implemented.

**Step 3: Write minimal implementation**

Add:

- a memory-core operation constant for prompt context
- request builder and decoder for prompt-context payloads
- backend dispatch to `load_prompt_context(...)`

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app memory:: -- --nocapture`

Expected: PASS.

### Task 3: Route conversation runtime through prompt-context operation

**Files:**
- Modify: `crates/app/src/conversation/runtime.rs`

**Step 1: Write the failing test**

Use the tests from Task 1.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app build_messages_routes_prompt_context_through_kernel -- --nocapture`

Expected: FAIL because `build_messages(...)` still requests `window`.

**Step 3: Write minimal implementation**

Replace raw `window` loading in the `kernel_ctx` branch with prompt-context
loading and decode the returned structured entries into messages.

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app build_messages_routes_prompt_context_through_kernel -- --nocapture`

Expected: PASS.

### Task 4: Verify full regression and architecture boundary improvement

**Files:**
- Modify: `crates/app/src/conversation/tests.rs`
- Modify: `crates/app/src/memory/mod.rs`
- Modify: `crates/app/src/conversation/runtime.rs`

**Step 1: Run targeted tests**

Run: `cargo test -p loongclaw-app build_messages_routes_prompt_context_through_kernel -- --nocapture`

Expected: PASS.

**Step 2: Run full app/workspace tests**

Run: `cargo test --workspace --all-features`

Expected: PASS.

**Step 3: Run architecture check**

Run: `./scripts/check_architecture_boundaries.sh`

Expected: PASS with reduced memory boundary warnings for this slice.
