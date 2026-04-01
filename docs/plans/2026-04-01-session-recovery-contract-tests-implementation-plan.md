# Session Recovery Contract Tests Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add direct unit tests for `session::recovery` so delegate recovery payload and fallback semantics are locked by local contract tests.

**Architecture:** Keep production behavior unchanged unless a test exposes a genuine contract bug. Add tests next to `session::recovery` for payload construction, event-vs-error precedence, kind synthesis, and JSON projection. Do not widen this pass into read-side ownership refactors.

**Tech Stack:** Rust, `serde_json`, existing session recovery types, focused unit tests in `crates/app/src/session/recovery.rs`, workspace cargo verification.

---

### Task 1: Add the failing unit tests for the recovery helper contract

**Files:**
- Modify: `crates/app/src/session/recovery.rs`
- Create: `docs/plans/2026-04-01-session-recovery-contract-tests-design.md`
- Create: `docs/plans/2026-04-01-session-recovery-contract-tests-implementation-plan.md`

**Step 1: Write the failing tests**

Add tests that cover:
- async spawn failure recovery payload fields
- newest recovery event winning over `last_error`
- `last_error` fallback synthesis when no event exists
- known prefix to recovery-kind mapping
- `recovery_json(...)` null projection for empty event kind and zero timestamp

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app --lib session::recovery
```

Expected:
- at least one new recovery test fails before the helper behavior or visibility is adjusted

**Step 3: Keep the red state local**

Do not commit or push a broken tree. Confirm the failing signal locally before any implementation step.

### Task 2: Make the helper contract explicit with minimal code changes

**Files:**
- Modify: `crates/app/src/session/recovery.rs`

**Step 1: Implement the smallest behavior or visibility adjustment needed**

Only change production code if a test reveals a real gap.

Keep each line atomic:
- one lookup per line
- one conversion per line
- one condition per line

**Step 2: Re-run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app --lib session::recovery
```

Expected:
- all new recovery tests pass

### Task 3: Verify nearby behavior still holds

**Files:**
- Verify only

**Step 1: Run the closest session-tool and coordinator recovery tests**

Run:

```bash
cargo test -p loongclaw-app --all-features session_status_synthesizes_recovery
cargo test -p loongclaw-app --all-features finalize_async_delegate_spawn_failure
```

Expected:
- existing higher-level recovery behavior remains green

**Step 2: Run lint on the touched crate**

Run:

```bash
cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings
```

Expected:
- no warnings

### Task 4: Run broad verification

**Files:**
- Verify only

**Step 1: Run workspace tests**

```bash
cargo test --workspace --locked
```

**Step 2: Run all-feature workspace tests**

```bash
cargo test --workspace --all-features --locked
```

Expected:
- workspace verification passes
- if an unrelated blocker appears, stop and document it explicitly before claiming completion

### Task 5: Commit and deliver cleanly

**Files:**
- Modify only the files touched by this plan

**Step 1: Inspect staged scope**

Run:

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

Expected:
- only the intended recovery test and planning files are staged

**Step 2: Commit**

```bash
git add docs/plans/2026-04-01-session-recovery-contract-tests-design.md
git add docs/plans/2026-04-01-session-recovery-contract-tests-implementation-plan.md
git add crates/app/src/session/recovery.rs
git commit -m "test(app): lock session recovery helper contracts"
```

**Step 3: Push and open the stacked PR**

Use the issue template and PR template workflow:
- issue first
- English GitHub text
- `--body-file` for multi-line Markdown
- explicit stacked-branch note in the PR body
