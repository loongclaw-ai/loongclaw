# Delegate Depth Lineage Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `tools.delegate.max_depth` semantically real by enabling synchronous nested delegation with lineage-based depth enforcement and descendant session visibility.

**Architecture:** Keep orchestration in the app-layer turn loop and compute delegate depth from the existing `sessions.parent_session_id` lineage. Extend the session repository to understand descendants, update child tool-view derivation to reflect remaining depth, and document the resulting synchronous nested-delegate semantics.

**Tech Stack:** Rust, `loongclaw-app`, SQLite via `rusqlite`, existing conversation turn-loop tests and config TOML surface.

---

### Task 1: Add Failing Tests for Depth-Aware Child Tool Views

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- a delegate child view can include `delegate` when remaining depth allows nesting
- a delegate child view excludes `delegate` when no further depth remains
- the child allowlist defaults only include currently supported runtime child tools

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app delegate_child -- --nocapture
```

Expected: FAIL because child tool-view construction is still hard-coded.

**Step 3: Write minimal implementation**

Update the catalog/tool-view helpers so child tool views are derived from:

- supported runtime core tools
- child allowlist
- shell policy
- whether nested delegate is still allowed

**Step 4: Re-run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app delegate_child -- --nocapture
```

Expected: PASS.

### Task 2: Add Failing Tests for Lineage Depth Enforcement

**Files:**
- Modify: `crates/app/src/session/repository.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- repository lineage depth is computed correctly for root/child/grandchild chains
- `max_depth = 2` allows root -> child -> grandchild
- exceeding the next delegate level returns `delegate_depth_exceeded`

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app delegate_depth -- --nocapture
```

Expected: FAIL because lineage depth helpers do not exist yet.

**Step 3: Write minimal implementation**

Add repository helpers for lineage depth and use them from `execute_delegate_tool(...)` before child creation.

**Step 4: Re-run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app delegate_depth -- --nocapture
```

Expected: PASS.

### Task 3: Add Failing Tests for Descendant Session Visibility

**Files:**
- Modify: `crates/app/src/session/repository.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- root `sessions_list` includes grandchild descendants
- root `session_status` can inspect a grandchild descendant
- non-ancestor sessions still cannot inspect unrelated sessions

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app descendant_session -- --nocapture
```

Expected: FAIL because visibility is still one-hop only.

**Step 3: Write minimal implementation**

Extend repository visibility helpers so `"children"` means descendant chain visibility.

**Step 4: Re-run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app descendant_session -- --nocapture
```

Expected: PASS.

### Task 4: Update Product Docs and Final Regression Coverage

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`
- Modify: `crates/app/src/config/tools_memory.rs`
- Modify: `crates/app/src/config/runtime.rs`

**Step 1: Write the doc updates**

Document:

- synchronous nested delegate semantics
- descendant session visibility
- current non-goals and deferred work

Also align config defaults/tests if the child allowlist is tightened to actual runtime tools.

**Step 2: Run focused regression**

Run:

```bash
cargo test -p loongclaw-app tools:: conversation:: session:: config:: -- --nocapture
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

Plan complete and saved to `docs/plans/2026-03-12-delegate-depth-lineage-implementation-plan.md`. Execution path for this session: continue locally with TDD against this plan.
