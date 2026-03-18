# Turn Checkpoint Kernel Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a compact typed turn checkpoint seam to the provider-path turn runtime so lane execution, reply decision, and finalization can be snapshotted without event replay.

**Architecture:** Keep the boundary narrow and behavior-preserving. Add snapshot structs around the existing provider turn stages, carry safe-lane terminal route provenance through the provider lane execution path, and make the finalization boundary consume the typed checkpoint instead of ad hoc local state.

**Tech Stack:** Rust, `loongclaw-app`, provider-path coordinator, unit tests, `cargo fmt`, `cargo test`, `cargo clippy`

---

### Task 1: Define the checkpoint seam with failing tests

**Files:**
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Test: `crates/app/src/conversation/turn_coordinator.rs`

**Step 1: Write the failing tests**

Add unit tests for:
- safe-lane terminal execution snapshot preserves route provenance and completion-pass reply mode
- inline provider error snapshot skips lane/reply stages and persists as inline error
- propagated provider error snapshot records return-error finalization with no persistence

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app turn_checkpoint_snapshot_ -- --test-threads=1`
Expected: FAIL with missing snapshot types/builders

**Step 3: Write minimal implementation**

Add compact checkpoint structs/enums and pure builders for:
- preparation summary
- provider request summary
- lane execution summary
- reply decision summary
- finalization summary

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app turn_checkpoint_snapshot_ -- --test-threads=1`
Expected: PASS

### Task 2: Carry safe-lane terminal provenance into the provider lane summary

**Files:**
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Test: `crates/app/src/conversation/turn_coordinator.rs`

**Step 1: Write the failing test**

Add a unit test showing that safe-lane terminal route provenance is available from the provider lane execution summary without inferring from failure codes.

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app safe_lane_turn_outcome_ -- --test-threads=1`
Expected: FAIL with missing typed outcome plumbing

**Step 3: Write minimal implementation**

Change the safe-lane execution path to return a typed outcome that includes:
- `TurnResult`
- optional terminal `SafeLaneFailureRoute`

Thread that summary through `ProviderTurnLaneExecution`.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app safe_lane_turn_outcome_ -- --test-threads=1`
Expected: PASS

### Task 3: Put the checkpoint on the finalization boundary

**Files:**
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Test: `crates/app/src/conversation/turn_coordinator.rs`

**Step 1: Write the failing test**

Add a unit test showing the provider finalization boundary consumes the typed checkpoint mode rather than a separate raw persistence mode argument.

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app finalize_provider_turn_reply_ -- --test-threads=1`
Expected: FAIL with signature/behavior mismatch

**Step 3: Write minimal implementation**

Refactor `finalize_provider_turn_reply(...)` to consume the checkpoint finalization summary while preserving behavior.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app finalize_provider_turn_reply_ -- --test-threads=1`
Expected: PASS

### Task 4: Align design note and run full verification

**Files:**
- Modify: `docs/plans/2026-03-13-hybrid-turn-kernel-convergence-design.md`
- Modify: `crates/app/src/conversation/turn_coordinator.rs`

**Step 1: Update design note**

Document the checkpoint seam and the decision to keep it compact, typed, and event-schema-neutral.

**Step 2: Run repository verification**

Run:
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all --check`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app -- --test-threads=1`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings`

Expected: all pass
