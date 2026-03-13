# Session Batch Remediation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend `session_recover` and `session_cancel` with safe batch targeting and `dry_run`
preview while preserving existing single-session behavior.

**Architecture:** Keep repository state transitions unchanged and add a thin tool-layer
normalization/aggregation path. Single-target non-preview calls continue down the legacy path;
batch or preview calls use a shared aggregated result model with per-target classifications.

**Tech Stack:** Rust, serde_json, sqlite-backed session repository, cargo test

---

### Task 1: Document the chosen operator model

**Files:**
- Create: `docs/plans/2026-03-13-session-batch-remediation-design.md`
- Create: `docs/plans/2026-03-13-session-batch-remediation-implementation-plan.md`

**Step 1: Write the design doc**

Capture goals, non-goals, options, chosen request/response shape, compatibility rules, and testing
scope.

**Step 2: Save the implementation plan**

Record exact implementation tasks, TDD flow, and verification commands.

### Task 2: Add provider-schema red tests

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/catalog.rs`

**Step 1: Write the failing test**

Extend `provider_tool_definitions_are_stable_and_complete` so `session_recover` and
`session_cancel` must expose:

- `session_id`
- `session_ids`
- `dry_run`

and a schema rule requiring exactly one target field.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app provider_tool_definitions_are_stable_and_complete -- --nocapture --test-threads=1`

Expected: FAIL because the current schema only exposes `session_id`.

**Step 3: Write minimal implementation**

Update the provider definitions in `crates/app/src/tools/catalog.rs`.

**Step 4: Run test to verify it passes**

Run the same focused test and confirm PASS.

### Task 3: Add `session_recover` batch red tests

**Files:**
- Modify: `crates/app/src/tools/session.rs`

**Step 1: Write the failing tests**

Add focused tests for:

- batch `session_recover` dry-run returning `would_apply` and skipped classifications
- batch `session_recover` apply returning `applied` and skipped classifications while mutating only
  applicable targets

**Step 2: Run tests to verify they fail**

Run:

`cargo test -p loongclaw-app session_recover_batch_ -- --nocapture --test-threads=1`

Expected: FAIL because the tool currently requires a single `session_id` and has no aggregated
result path.

**Step 3: Write minimal implementation**

Add request parsing helpers and a shared aggregated execution path for batch or preview recover
calls.

**Step 4: Run tests to verify they pass**

Run the same focused recover batch tests and confirm PASS.

### Task 4: Add `session_cancel` batch red tests

**Files:**
- Modify: `crates/app/src/tools/session.rs`

**Step 1: Write the failing tests**

Add focused tests for:

- batch `session_cancel` dry-run returning queued/running `would_apply` results
- batch `session_cancel` apply returning `applied` for queued and running targets while preserving
  per-target state semantics

**Step 2: Run tests to verify they fail**

Run:

`cargo test -p loongclaw-app session_cancel_batch_ -- --nocapture --test-threads=1`

Expected: FAIL because the tool currently only accepts a single `session_id`.

**Step 3: Write minimal implementation**

Extend the shared request/result helpers and apply existing cancel logic per target.

**Step 4: Run tests to verify they pass**

Run the same focused cancel batch tests and confirm PASS.

### Task 5: Update product docs

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Update acceptance criteria / roadmap bullets**

Document batch targeting and `dry_run` preview on the existing remediation tools.

**Step 2: Verify wording matches shipped behavior**

Confirm docs describe no new top-level tool and preserve current limits.

### Task 6: Run completion verification

**Files:**
- Modify: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Run focused verification**

Run the focused schema and batch tests:

- `cargo test -p loongclaw-app provider_tool_definitions_are_stable_and_complete -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_recover_batch_ -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_cancel_batch_ -- --nocapture --test-threads=1`

**Step 2: Run broader session-tool verification**

Run:

- `cargo test -p loongclaw-app session_recover_ -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_cancel_ -- --nocapture --test-threads=1`
- `cargo test -p loongclaw-app session_ -- --nocapture --test-threads=1`

**Step 3: Run full repository verification**

Run:

- `cargo fmt --all`
- `cargo test --workspace --all-features -- --test-threads=1`
- `cargo fmt --all --check`
- `git diff --check`

**Step 4: Inspect final git state**

Run:

- `git status --short`
- `git diff --cached --name-only`
- `git diff --cached`
