# Turn Loop Kernelization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the fast-lane conversation turn loop into an explicit behavior-preserving kernel with typed round evaluation and decision boundaries.

**Architecture:** Keep `ConversationTurnLoop` as the public entrypoint, but move the current implicit control flow into explicit internal session-state, round-evaluation, and next-step decision helpers. Preserve current external behavior while reducing control-flow coupling between tool-loop detection, follow-up prompt assembly, completion fallback, and persistence.

**Tech Stack:** Rust, async conversation runtime traits, existing conversation tests, cargo test

---

### Task 1: Lock the kernelization target in docs

**Files:**
- Modify: `docs/plans/2026-03-13-turn-loop-kernelization-design.md`
- Create: `docs/plans/2026-03-13-turn-loop-kernelization.md`

**Step 1: Re-read the current turn loop and tests**

Run: `rg -n "handle_turn_with_runtime|ToolLoopSupervisor|append_tool_followup_messages" crates/app/src/conversation`
Expected: current kernelization surface is fully enumerated.

**Step 2: Confirm the design and implementation-plan docs exist**

Run: `ls docs/plans/2026-03-13-turn-loop-kernelization-design.md docs/plans/2026-03-13-turn-loop-kernelization.md`
Expected: both files exist.

### Task 2: Add failing kernel-level tests for next-step decisions

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Test: `crates/app/src/conversation/turn_loop.rs`

**Step 1: Write a failing test for tool-result rounds that should continue with follow-up**

Add a focused unit test around the new decision helper.

**Step 2: Run the targeted test to verify it fails**

Run: `cargo test -p loongclaw-app turn_loop::tests::<new_test_name> -- --test-threads=1`
Expected: FAIL because the new kernel decision helper or type does not exist yet.

**Step 3: Repeat for hard-stop and raw-output bypass cases**

Add separate failing tests for:
- hard-stop with tool result
- hard-stop with tool failure
- raw-output mode bypassing second-pass completion

### Task 3: Introduce explicit turn-loop kernel types

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`

**Step 1: Add typed internal structures**

Introduce minimal internal types for:
- session state
- round evaluation
- next-step decision
- finalization mode

**Step 2: Keep constructors narrow and local**

Implement the smallest helper methods needed to derive decisions from existing round data.

**Step 3: Run targeted tests**

Run: `cargo test -p loongclaw-app turn_loop -- --test-threads=1`
Expected: the new decision tests still fail until the main loop consumes the kernel.

### Task 4: Rewrite the main turn loop around the kernel

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`

**Step 1: Route each round through explicit evaluation + decision helpers**

Replace the large repeated match branches in `handle_turn_with_runtime(...)` with:
- build round evaluation
- derive decision
- apply continue/finalize action

**Step 2: Preserve current side effects**

Ensure the refactor still:
- persists inline provider errors correctly
- persists successful replies exactly once
- uses second-pass completion only on the same paths as before
- preserves round-limit fallback behavior

**Step 3: Run targeted tests to verify the refactor passes**

Run: `cargo test -p loongclaw-app turn_loop -- --test-threads=1`
Expected: PASS.

### Task 5: Re-run existing high-level conversation coverage

**Files:**
- Modify: `crates/app/src/conversation/tests.rs` (only if current coverage reveals a missing kernel case)
- Test: `crates/app/src/conversation/tests.rs`

**Step 1: Run the existing turn-loop/conversation behavior tests**

Run: `cargo test -p loongclaw-app handle_turn_with_runtime -- --test-threads=1`
Expected: PASS.

**Step 2: Add one regression test only if a previously implicit branch is not already covered**

Keep this minimal and behavior-focused.

### Task 6: Run broader verification and clean up

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/conversation/tests.rs` (if needed)

**Step 1: Run package-level verification**

Run: `cargo test -p loongclaw-app -- --test-threads=1`
Expected: PASS.

**Step 2: Run lint for the touched crate**

Run: `cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings`
Expected: PASS.

**Step 3: Review the final diff**

Run: `git diff -- crates/app/src/conversation/turn_loop.rs crates/app/src/conversation/tests.rs docs/plans/2026-03-13-turn-loop-kernelization-design.md docs/plans/2026-03-13-turn-loop-kernelization.md`
Expected: only the intended kernelization scope is present.
