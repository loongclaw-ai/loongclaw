# Delegate Async Subprocess Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add non-blocking `delegate_async` using a subprocess one-shot worker while reusing session ids plus `session_wait/session_status/session_events` as the async handle and observability surface.

**Architecture:** Extend the app tool surface with `delegate_async`, add a pluggable async delegate spawner to the default app dispatcher, and add a daemon `run-turn` command for one-shot child-session execution. Parent-side async delegation queues child work and returns immediately; worker-side execution persists the normal delegate lifecycle and terminal outcomes.

**Tech Stack:** Rust, Tokio, rusqlite, clap, existing LoongClaw app/daemon crates

---

### Task 1: Add design-level failing tests for `delegate_async` tool visibility/schema

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/provider/mod.rs`

**Step 1: Write the failing test**

Add tests asserting:

- root runtime tool view includes `delegate_async`
- provider tool definitions include `delegate_async`
- child tool view includes `delegate_async` only when remaining depth allows

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async -- --nocapture`

Expected: FAIL because the tool is not registered yet.

**Step 3: Write minimal implementation**

Register the new tool in catalog/provider views only.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async -- --nocapture`

Expected: PASS

### Task 2: Add async delegate spawner abstraction and parent-side orchestration

**Files:**
- Modify: `crates/app/src/tools/delegate.rs`
- Modify: `crates/app/src/conversation/turn_engine.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing test**

Add tests asserting:

- `delegate_async` returns queued payload immediately
- spawn failure records child failure and `delegate_spawn_failed`

Use a fake spawner in tests.

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async_queue -- --nocapture`

Expected: FAIL because no async spawner path exists.

**Step 3: Write minimal implementation**

Implement:

- spawner trait + fake-test support
- parent-side session row creation and `delegate_queued`
- immediate queued tool outcome
- spawn-failure cleanup/event persistence

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async_queue -- --nocapture`

Expected: PASS

### Task 3: Add background child lifecycle helper

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing test**

Add tests for a delegate-child lifecycle helper covering:

- success
- provider/runtime failure
- timeout

Expected observations:

- session state transitions
- lifecycle events
- durable terminal outcomes

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_child_background_ -- --nocapture`

Expected: FAIL because the lifecycle helper does not exist.

**Step 3: Write minimal implementation**

Factor shared delegate-child execution/finalization logic into a reusable helper that can be called by both synchronous `delegate` and async worker execution.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_child_background_ -- --nocapture`

Expected: PASS

### Task 4: Add daemon `run-turn` worker command

**Files:**
- Modify: `crates/daemon/src/main.rs`
- Test: `crates/daemon/src/tests.rs`

**Step 1: Write the failing test**

Add daemon tests for:

- command parsing of `run-turn`
- `--delegate-child` path dispatch
- config path resolution from arg/env

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-daemon run_turn -- --nocapture`

Expected: FAIL because the command does not exist.

**Step 3: Write minimal implementation**

Implement the CLI command and route it to the child lifecycle helper or normal one-shot turn execution.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-daemon run_turn -- --nocapture`

Expected: PASS

### Task 5: Wire production subprocess spawning

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/channel/mod.rs`

**Step 1: Write the failing test**

Add tests for:

- entrypoints export `LOONGCLAW_CONFIG_PATH`
- production spawner command shape is stable

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app config_path -- --nocapture`

Expected: FAIL because async delegate production handoff is not wired.

**Step 3: Write minimal implementation**

Wire:

- config-path export from interactive/channel entrypoints
- subprocess delegate spawner using current executable + `run-turn`

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app config_path -- --nocapture`

Expected: PASS

### Task 6: Update docs and run full verification

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Update docs**

Document:

- `delegate_async`
- session id as async handle
- subprocess worker model
- current limits (no cancel, no durable queue)

**Step 2: Run focused regression**

Run:

- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_wait -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-daemon run_turn -- --nocapture`

Expected: PASS

**Step 3: Run full verification**

Run:

- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-daemon -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all --check`
- `git diff --check`

Expected:

- app and daemon suites green
- formatting clean
- no patch hygiene issues

Plan complete and saved to `docs/plans/2026-03-12-delegate-async-subprocess-implementation-plan.md`. Execution path for this session: continue locally in the current worktree with TDD against this plan.
