# Session Status Batch Inspection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend `session_status` with `session_ids` batch inspection while preserving the legacy
single-session response shape.

**Architecture:** Keep the existing single-target path unchanged and add a batch aggregation path
that loops over visible-session inspection using the current `session_inspection_payload`. Hidden
targets become per-item skipped results instead of failing the whole request.

**Tech Stack:** Rust, serde_json, sqlite-backed session repository, cargo test

---

### Task 1: Document the slice

**Files:**
- Create: `docs/plans/2026-03-13-session-status-batch-inspection-design.md`
- Create: `docs/plans/2026-03-13-session-status-batch-inspection-implementation-plan.md`

**Step 1: Write the design doc**

Capture request/response shape, compatibility rules, visibility semantics, and test scope.

**Step 2: Save the implementation plan**

Record the concrete TDD sequence and verification commands.

### Task 2: Add provider-schema red tests

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/catalog.rs`

**Step 1: Write the failing test**

Extend `provider_tool_definitions_are_stable_and_complete` so `session_status` must expose:

- `session_id`
- `session_ids`

with `oneOf` enforcing exactly one target field.

**Step 2: Run test to verify it fails**

Run:

`cargo test -p loongclaw-app provider_tool_definitions_are_stable_and_complete -- --nocapture --test-threads=1`

Expected: FAIL because the current schema only exposes `session_id`.

**Step 3: Write minimal implementation**

Update the provider definition in `crates/app/src/tools/catalog.rs`.

**Step 4: Run test to verify it passes**

Re-run the same focused schema test and confirm PASS.

### Task 3: Add `session_status` batch red tests

**Files:**
- Modify: `crates/app/src/tools/session.rs`

**Step 1: Write the failing tests**

Add focused coverage for:

- batch `session_status` returning full inspection payloads for visible targets
- hidden targets returning `skipped_not_visible`

**Step 2: Run tests to verify they fail**

Run:

`cargo test -p loongclaw-app session_status_batch_ -- --nocapture --test-threads=1`

Expected: FAIL because the tool currently requires a single `session_id`.

**Step 3: Write minimal implementation**

Add request parsing and aggregated batch response handling in `crates/app/src/tools/session.rs`.

**Step 4: Run tests to verify they pass**

Re-run the focused batch status tests and confirm PASS.

### Task 4: Update product docs

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Update acceptance criteria / roadmap**

Document batch inspection support on `session_status`.

**Step 2: Confirm wording preserves boundaries**

Verify docs still say no new top-level tool and no authority expansion.

### Task 5: Run completion verification

**Files:**
- Modify: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Run focused verification**

- `cargo test -p loongclaw-app provider_tool_definitions_are_stable_and_complete -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_status_batch_ -- --nocapture --test-threads=1`

**Step 2: Run broader session inspection verification**

- `cargo test -p loongclaw-app session_status_ -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_ -- --nocapture --test-threads=1`

**Step 3: Run full repository verification**

- `cargo fmt --all`
- `cargo test --workspace --all-features -- --test-threads=1`
- `cargo fmt --all --check`
- `git diff --check`

**Step 4: Inspect final git state**

- `git status --short`
- `git diff --cached --name-only`
- `git diff --cached`
