# Child Session Self-Inspection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let delegated child sessions inspect their own status and history while keeping session browsing restricted to self-only semantics.

**Architecture:** Extend the child tool view to expose `session_status` and `sessions_history`, then derive an effective app-tool policy from `SessionContext` so child sessions always execute session tools with `visibility = self`. This preserves root semantics and avoids a new config surface.

**Tech Stack:** Rust, `loongclaw-app`, SQLite-backed session repository, app-layer tool dispatcher, existing turn-loop and session-tool tests.

---

### Task 1: Add Failing Tests for Child Self-Inspection Tool Views

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- delegated child tool views include `session_status`
- delegated child tool views include `sessions_history`
- delegated child tool views still exclude `sessions_list`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app child_session_self_inspection -- --nocapture
```

Expected: FAIL because child tool views do not currently expose session self-inspection tools.

**Step 3: Write minimal implementation**

Update child tool-view builders to include the self-inspection app tools.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p loongclaw-app child_session_self_inspection -- --nocapture
```

Expected: PASS.

### Task 2: Add Failing Tests for Child Self-Only Execution Policy

**Files:**
- Modify: `crates/app/src/conversation/tests.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `crates/app/src/conversation/turn_engine.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- a child session can call `session_status` for itself
- a child session can call `sessions_history` for itself
- a child session cannot call `sessions_list`
- a child session cannot inspect a descendant session and receives `visibility_denied`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app child_session_self_only -- --nocapture
```

Expected: FAIL because child sessions currently inherit the broader global session visibility policy.

**Step 3: Write minimal implementation**

Add an effective tool-policy helper keyed by `SessionContext` and apply it in the default app dispatcher so child sessions force `tools.sessions.visibility = self`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p loongclaw-app child_session_self_only -- --nocapture
```

Expected: PASS.

### Task 3: Update Docs and Run Final Verification

**Files:**
- Modify: `docs/product-specs/index.md`
- Optionally Modify: `docs/roadmap.md`

**Step 1: Write the doc updates**

Document that child sessions now have self-inspection only:

- `session_status`
- `sessions_history`
- no `sessions_list`
- no parent/descendant session inspection

**Step 2: Run focused regression**

Run:

```bash
cargo test -p loongclaw-app session_status sessions_history child_session -- --nocapture
```

Expected: PASS.

**Step 3: Run full verification**

Run:

```bash
cargo fmt --all
cargo test -p loongclaw-app -- --nocapture
cargo fmt --all --check
git diff --check
```

Expected: all commands pass cleanly.

Plan complete and saved to `docs/plans/2026-03-12-child-session-self-inspection-implementation-plan.md`. Execution path for this session: continue locally with TDD against this plan.
