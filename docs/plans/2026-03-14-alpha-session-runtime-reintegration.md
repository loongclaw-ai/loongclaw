# Alpha Session Runtime Reintegration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the session runtime base natively on current `alpha-test` so the stacked session
tool PRs can land on a mergeable foundation.

**Architecture:** Reconstruct the hidden base in small alpha-native slices instead of rebasing the
old branch wholesale. Start with the tool catalog foundation, then port config, repository/runtime
primitives, execution routing, and finally the session tool behaviors.

**Tech Stack:** Rust, Cargo workspace, Tokio, serde_json, GitHub PR workflow

---

### Task 1: Catalog Foundation

**Files:**
- Create: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Test: `crates/app/src/tools/mod.rs`

**Step 1: Write the failing tests**

Add tests for:

- restricted `ToolView` snapshots
- restricted provider tool definitions
- planned tools being rejected from provider exposure

**Step 2: Run tests to verify they fail**

Run: `cargo test -p loongclaw-app tools::tests::capability_snapshot_for_view_only_lists_selected_tools`
Expected: FAIL because the catalog/view APIs do not exist yet.

**Step 3: Write minimal implementation**

Introduce catalog types and route the existing core tool surface through them without changing the
set of currently exposed tools.

**Step 4: Run tests to verify they pass**

Run the targeted tools tests and then `cargo fmt --all -- --check`.

**Step 5: Commit**

```bash
git add crates/app/src/tools/catalog.rs crates/app/src/tools/mod.rs docs/plans/2026-03-14-alpha-session-runtime-reintegration-design.md docs/plans/2026-03-14-alpha-session-runtime-reintegration.md
git commit -m "refactor(app): add catalog-backed tool surface foundation"
```

### Task 2: Config Surface For Session Runtime

**Files:**
- Modify: `crates/app/src/config/tools_memory.rs`
- Modify: `crates/app/src/config/mod.rs`
- Modify: `crates/app/src/config/runtime.rs`
- Test: corresponding config tests

**Step 1: Write the failing tests**

Add tests covering session/message/delegate config defaults and parsing.

**Step 2: Run test to verify it fails**

Run the relevant config tests and confirm the new config fields are missing.

**Step 3: Write minimal implementation**

Port the config surface from the hidden base, but keep future app tools hidden until runtime support
lands.

**Step 4: Run test to verify it passes**

Run targeted config tests and `cargo fmt --all -- --check`.

**Step 5: Commit**

```bash
git add crates/app/src/config/tools_memory.rs crates/app/src/config/mod.rs crates/app/src/config/runtime.rs
git commit -m "feat(config): add session runtime tool policy controls"
```

### Task 3: Session Runtime Primitives

**Files:**
- Create: `crates/app/src/runtime_env.rs`
- Create: `crates/app/src/session/mod.rs`
- Create: `crates/app/src/session/recovery.rs`
- Create: `crates/app/src/session/repository.rs`
- Modify: `crates/app/src/lib.rs`
- Test: new session repository/runtime tests

**Step 1: Write the failing tests**

Add repository/runtime tests for session persistence, lineage lookup, and runtime environment export.

**Step 2: Run test to verify it fails**

Run the targeted repository/runtime tests and confirm missing modules or missing behavior.

**Step 3: Write minimal implementation**

Port the repository/recovery primitives and shared runtime-env export without exposing app tools
yet.

**Step 4: Run test to verify it passes**

Run targeted tests plus `cargo fmt --all -- --check`.

**Step 5: Commit**

```bash
git add crates/app/src/runtime_env.rs crates/app/src/session crates/app/src/lib.rs
git commit -m "feat(app): add session runtime persistence foundation"
```

### Task 4: App Tool Execution Path

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/conversation/runtime.rs`
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Modify: `crates/app/src/provider/request_message_runtime.rs`
- Test: conversation and tools tests

**Step 1: Write the failing tests**

Add tests that verify the runtime can differentiate core tools from app tools and expose only the
correct view in provider schemas.

**Step 2: Run test to verify it fails**

Run the targeted conversation/tool tests and confirm app-tool execution is unavailable.

**Step 3: Write minimal implementation**

Add explicit app-tool execution routing and session-aware tool view plumbing into the current
conversation runtime/coordinator architecture.

**Step 4: Run test to verify it passes**

Run targeted conversation/tool tests and `cargo fmt --all -- --check`.

**Step 5: Commit**

```bash
git add crates/app/src/tools/mod.rs crates/app/src/conversation/runtime.rs crates/app/src/conversation/turn_coordinator.rs crates/app/src/provider/request_message_runtime.rs
git commit -m "feat(app): wire session-aware app tool execution"
```

### Task 5: Session Tool Behaviors

**Files:**
- Create/Modify: `crates/app/src/tools/session.rs`
- Create/Modify: `crates/app/src/tools/messaging.rs`
- Create/Modify: `crates/app/src/tools/delegate.rs`
- Modify: `crates/app/src/channel/mod.rs`
- Modify: `crates/app/src/chat.rs`
- Test: tool, channel, conversation, daemon tests

**Step 1: Write the failing tests**

Add behavior tests in narrow slices for each tool family before implementation.

**Step 2: Run test to verify it fails**

Run only the new targeted tests for the selected tool family.

**Step 3: Write minimal implementation**

Port one tool family at a time, keeping provider exposure aligned with actual runtime support.

**Step 4: Run test to verify it passes**

Run targeted tests, then broader workspace verification.

**Step 5: Commit**

```bash
git add <touched files>
git commit -m "<small scoped commit>"
```
