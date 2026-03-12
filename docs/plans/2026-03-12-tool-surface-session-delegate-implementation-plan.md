# LoongClaw Session and Delegate Tool Surface Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a thick app-layer session and delegation tool surface to `loongclaw` by introducing dynamic tool views, a session registry, session inspection tools, and synchronous nested delegation.

**Architecture:** Keep kernel core tools for stateless execution, but add an app-layer dispatcher for orchestration tools. Back session metadata with lightweight SQLite tables in the existing memory database, then build `sessions_list`, `sessions_history`, `session_status`, and `delegate` on top of that session repository and a per-session `ToolView`.

**Tech Stack:** Rust, existing `loongclaw-app` conversation runtime and provider stack, SQLite via `rusqlite`, existing turn-loop integration tests, app config TOML, kernel-backed core tool execution.

---

### Task 1: Add Tool Catalog and Tool View Primitives

**Files:**
- Create: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Test: `crates/app/src/tools/mod.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- the catalog distinguishes core tools from app tools
- a root tool view includes the expected first-phase tools
- a child delegate tool view excludes `delegate` and `sessions_*`
- `provider_tool_definitions` can be derived from a restricted tool view

Suggested test names:

```rust
#[test]
fn tool_catalog_marks_core_and_app_tools() {}

#[test]
fn child_tool_view_excludes_delegate_and_session_tools() {}

#[test]
fn provider_tool_definitions_follow_tool_view() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app tools:: -- --nocapture
```

Expected: FAIL because the catalog and view types do not exist yet.

**Step 3: Write minimal implementation**

Add:

- `ToolExecutionKind`
- `ToolDescriptor`
- `ToolCatalog`
- `ToolView`

Update `tools/mod.rs` so catalog lookups replace hard-coded name checks for new code paths.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app tools:: -- --nocapture
```

Expected: PASS for the new catalog and view tests.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/tools/catalog.rs crates/app/src/tools/mod.rs
git commit -m "feat: add tool catalog and tool view primitives"
```

---

### Task 2: Make Provider Tool Schema and Capability Snapshot Dynamic

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/provider/mod.rs`
- Modify: `crates/app/src/conversation/runtime.rs`
- Test: `crates/app/src/provider/mod.rs`
- Test: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- capability snapshot output changes when tool view changes
- provider turn requests include only visible tools
- child sessions do not advertise forbidden tools in request bodies

Suggested test names:

```rust
#[test]
fn capability_snapshot_respects_tool_view() {}

#[test]
fn build_turn_request_body_respects_restricted_tool_view() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app provider:: conversation:: -- --nocapture
```

Expected: FAIL because provider schema generation is currently global.

**Step 3: Write minimal implementation**

Thread `ToolView` into:

- capability snapshot generation
- provider tool definition generation
- runtime message building for system prompt disclosure
- turn-request construction for tool schema

Avoid changing unrelated provider behavior.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app provider:: conversation:: -- --nocapture
```

Expected: PASS for the new dynamic-schema tests.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/tools/mod.rs crates/app/src/provider/mod.rs crates/app/src/conversation/runtime.rs crates/app/src/conversation/tests.rs
git commit -m "feat: derive provider tool schema from per-session tool views"
```

---

### Task 3: Introduce Session Repository and SQLite Metadata Tables

**Files:**
- Create: `crates/app/src/session/repository.rs`
- Modify: `crates/app/src/memory/sqlite.rs`
- Modify: `crates/app/src/memory/mod.rs`
- Test: `crates/app/src/session/repository.rs`
- Test: `crates/app/src/memory/mod.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- session metadata can be inserted and loaded
- state transitions persist
- parent-child relationships persist
- session events do not appear in transcript window queries

Suggested test names:

```rust
#[test]
fn session_repository_creates_and_loads_session_rows() {}

#[test]
fn session_repository_updates_state_and_last_error() {}

#[test]
fn transcript_window_excludes_session_events() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app session:: memory:: -- --nocapture
```

Expected: FAIL because the session repository and new tables do not exist yet.

**Step 3: Write minimal implementation**

Add:

- session row model
- session event model
- schema creation for `sessions` and `session_events`
- repository helpers for create, update, list, and event append

Keep transcript history in `turns` unchanged.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app session:: memory:: -- --nocapture
```

Expected: PASS for the new repository tests.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/session/repository.rs crates/app/src/memory/sqlite.rs crates/app/src/memory/mod.rs
git commit -m "feat: add session repository and metadata tables"
```

---

### Task 4: Add Session Context and App Tool Dispatcher

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/conversation/runtime.rs`
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/channel/mod.rs`
- Test: `crates/app/src/conversation/tests.rs`
- Test: `crates/app/src/conversation/integration_tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- turn execution can route app tools without sending them through the kernel core tool adapter
- current session context is available during tool execution
- root sessions and child sessions receive different tool views

Suggested test names:

```rust
#[tokio::test]
async fn turn_engine_routes_app_tools_through_dispatcher() {}

#[tokio::test]
async fn conversation_turn_uses_tool_view_from_session_context() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app conversation:: -- --nocapture
```

Expected: FAIL because the dispatcher and session context do not exist yet.

**Step 3: Write minimal implementation**

Introduce:

- session context model
- app tool dispatcher trait or module
- turn-engine branching by `ToolExecutionKind`
- root session context construction in CLI and channel entrypoints

Keep core-tool execution behavior unchanged for `file.read`, `file.write`, and `shell.exec`.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app conversation:: -- --nocapture
```

Expected: PASS for dispatcher routing tests and no regression in existing core-tool tests.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/conversation/turn_engine.rs crates/app/src/conversation/turn_loop.rs crates/app/src/conversation/runtime.rs crates/app/src/chat.rs crates/app/src/channel/mod.rs crates/app/src/conversation/tests.rs crates/app/src/conversation/integration_tests.rs
git commit -m "feat: add session context and app tool dispatch"
```

---

### Task 5: Implement `sessions_list`, `sessions_history`, and `session_status`

**Files:**
- Create: `crates/app/src/tools/session.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/session/repository.rs`
- Test: `crates/app/src/tools/session.rs`
- Test: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- `sessions_list` returns current session and visible children
- `sessions_history` returns transcript rows only
- `session_status` returns state and last error
- unrelated sessions are hidden by visibility rules

Suggested test names:

```rust
#[tokio::test]
async fn sessions_list_returns_current_session_and_children() {}

#[tokio::test]
async fn sessions_history_returns_transcript_without_control_events() {}

#[tokio::test]
async fn session_status_returns_state_and_last_error() {}

#[tokio::test]
async fn session_tools_reject_invisible_sessions() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app session_status -- --nocapture
```

Expected: FAIL because the session tools do not exist yet.

**Step 3: Write minimal implementation**

Implement first-phase app tools:

- `sessions_list`
- `sessions_history`
- `session_status`

Wire them into the catalog and app dispatcher.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app session -- --nocapture
```

Expected: PASS for new session tool behavior and visibility tests.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/tools/session.rs crates/app/src/tools/mod.rs crates/app/src/session/repository.rs crates/app/src/conversation/tests.rs
git commit -m "feat: add session inspection tools"
```

---

### Task 6: Implement Synchronous `delegate`

**Files:**
- Create: `crates/app/src/tools/delegate.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/session/repository.rs`
- Test: `crates/app/src/tools/delegate.rs`
- Test: `crates/app/src/conversation/tests.rs`
- Test: `crates/app/src/conversation/integration_tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- `delegate` creates a child session
- child sessions run with a restricted tool view
- child completion returns structured output to the parent
- timeout sets session state to `timed_out`
- child cannot call `delegate`

Suggested test names:

```rust
#[tokio::test]
async fn delegate_creates_child_session_and_returns_structured_result() {}

#[tokio::test]
async fn delegate_child_uses_restricted_tool_view() {}

#[tokio::test]
async fn delegate_timeout_sets_child_session_to_timed_out() {}

#[tokio::test]
async fn delegate_child_cannot_reenter_delegate() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app delegate -- --nocapture
```

Expected: FAIL because the delegate tool does not exist yet.

**Step 3: Write minimal implementation**

Implement:

- child session creation
- state transition to `running`
- child event logging
- nested `ConversationTurnLoop` execution
- structured delegate result
- timeout and failure mapping

Do not add background execution, wait handles, or channel announcements.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app delegate -- --nocapture
```

Expected: PASS for delegate happy path, timeout path, and tool-visibility restrictions.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/tools/delegate.rs crates/app/src/tools/mod.rs crates/app/src/conversation/turn_loop.rs crates/app/src/session/repository.rs crates/app/src/conversation/tests.rs crates/app/src/conversation/integration_tests.rs
git commit -m "feat: add synchronous delegate sessions"
```

---

### Task 7: Add Config Surface for Session and Delegate Policy

**Files:**
- Modify: `crates/app/src/config/tools_memory.rs`
- Modify: `crates/app/src/config/runtime.rs`
- Test: `crates/app/src/config/runtime.rs`
- Test: `crates/app/src/config/tools_memory.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- default config enables session and delegate tools with safe defaults
- `visibility = "children"` is parsed correctly
- child shell execution defaults to disabled
- config round-trips through TOML

Suggested test names:

```rust
#[test]
fn tool_config_defaults_enable_safe_session_and_delegate_policy() {}

#[test]
fn tool_config_parses_children_visibility() {}

#[test]
fn tool_config_round_trips_session_and_delegate_settings() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app config:: -- --nocapture
```

Expected: FAIL because the new config sections do not exist yet.

**Step 3: Write minimal implementation**

Extend `ToolConfig` with nested config types for:

- session tool visibility and limits
- delegate enablement, timeout, depth, and child allowlist

Keep the config shape intentionally small.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app config:: -- --nocapture
```

Expected: PASS for defaults and TOML round-trip behavior.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/config/tools_memory.rs crates/app/src/config/runtime.rs
git commit -m "feat: add config for session and delegate policy"
```

---

### Task 8: Cover Legacy Session Fallback and Final Regression Cases

**Files:**
- Modify: `crates/app/src/session/repository.rs`
- Modify: `crates/app/src/memory/sqlite.rs`
- Test: `crates/app/src/session/repository.rs`
- Test: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing tests**

Add tests that assert:

- `sessions_list` can surface a legacy session inferred from `turns`
- legacy rows do not require backfill
- inferred legacy kind uses known prefixes when possible

Suggested test names:

```rust
#[test]
fn sessions_list_infers_legacy_rows_from_turn_history() {}

#[test]
fn inferred_legacy_session_kind_uses_known_prefixes() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-app legacy_session -- --nocapture
```

Expected: FAIL because legacy fallback is not implemented yet.

**Step 3: Write minimal implementation**

Add best-effort legacy-row inference by scanning `turns` for sessions missing from the `sessions` table.

Do not rewrite historical rows.

**Step 4: Re-run the tests**

Run:

```bash
cargo test -p loongclaw-app legacy_session -- --nocapture
```

Expected: PASS for inferred legacy session coverage.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/session/repository.rs crates/app/src/memory/sqlite.rs
git commit -m "feat: preserve legacy sessions in session listing"
```

---

### Task 9: Update Docs and Validate the Final Slice

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`
- Optionally Modify: `docs/releases/TEMPLATE.md`
- Test: existing app tests

**Step 1: Write the doc changes**

Document:

- new session tools
- delegate semantics
- current limitations of the synchronous child-session model
- future work explicitly deferred

**Step 2: Run the focused regression suite**

Run:

```bash
cargo test -p loongclaw-app tools:: conversation:: session:: config:: -- --nocapture
```

Expected: PASS for the new surface and existing core behavior.

**Step 3: Run the broader app test suite**

Run:

```bash
cargo test -p loongclaw-app -- --nocapture
```

Expected: PASS, or else stop and fix regressions before claiming completion.

**Step 4: Review git scope**

Run:

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

Expected: only session/delegate tool-surface work is staged.

**Step 5: Commit**

Run:

```bash
git add docs/product-specs/index.md docs/roadmap.md docs/releases/TEMPLATE.md
git commit -m "docs: describe session and delegate tool surface"
```

---

Plan complete and saved to `docs/plans/2026-03-12-tool-surface-session-delegate-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch a fresh subagent per task, review between tasks, and iterate quickly.

**2. Parallel Session (separate)** - Open a new session in this worktree and execute the plan task-by-task with checkpoints.

Recommended: **Subagent-Driven (this session)**, because the work splits cleanly between tool catalog, session repository, and delegate integration, but still benefits from stepwise review between commits.
