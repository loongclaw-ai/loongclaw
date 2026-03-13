# Session Observability Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a thick observability layer for delegated sessions by persisting terminal outcomes and exposing `session_events` and `session_wait` without introducing async background delegation.

**Architecture:** Extend the sqlite-backed session repository with a durable terminal-outcome read model, wire that data into session inspection tools, and add a bounded async `session_wait` path in the default app dispatcher. Keep delegated child tool views narrow and leave synchronous `delegate` execution semantics unchanged.

**Tech Stack:** Rust, Tokio, rusqlite, serde_json, existing LoongClaw app-layer tool/session runtime

---

### Task 1: Add durable terminal outcome persistence

**Files:**
- Modify: `crates/app/src/memory/sqlite.rs`
- Modify: `crates/app/src/session/repository.rs`
- Test: `crates/app/src/session/repository.rs`

**Step 1: Write the failing test**

Add repository tests for:

- upserting a terminal outcome for a session
- loading it back with stable status/payload fields
- replacing an existing terminal outcome for the same session

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_terminal_outcome -- --nocapture`

Expected: FAIL because the schema and repository methods do not exist yet.

**Step 3: Write minimal implementation**

Implement:

- `session_terminal_outcomes` table creation in sqlite schema bootstrap
- repository record type and helpers
- `upsert_terminal_outcome(...)`
- `load_terminal_outcome(...)`

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_terminal_outcome -- --nocapture`

Expected: PASS

### Task 2: Expose terminal outcomes in session inspection tools

**Files:**
- Modify: `crates/app/src/tools/session.rs`
- Test: `crates/app/src/tools/session.rs`

**Step 1: Write the failing test**

Add tests for:

- `session_status` includes `terminal_outcome`
- `session_status` returns `terminal_outcome = null` when no durable outcome exists

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_status_returns_state_and_last_error -- --nocapture`

Expected: FAIL because `session_status` does not currently expose terminal outcomes.

**Step 3: Write minimal implementation**

Update the session-status payload builder to read the repository outcome row and include it in the response shape.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_status_returns_state_and_last_error -- --nocapture`

Expected: PASS

### Task 3: Add `session_events`

**Files:**
- Modify: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `crates/app/src/provider/mod.rs`
- Test: `crates/app/src/tools/session.rs`
- Test: `crates/app/src/tools/mod.rs`
- Test: `crates/app/src/provider/mod.rs`

**Step 1: Write the failing test**

Add tests for:

- root runtime tool view includes `session_events`
- delegated child tool view still excludes `session_events`
- `session_events` returns ascending ordered events
- `session_events` respects `after_id`

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_events -- --nocapture`

Expected: FAIL because the tool is not registered or executable yet.

**Step 3: Write minimal implementation**

Implement:

- catalog/definition entry for `session_events`
- app-tool dispatch path in `tools/mod.rs`
- repository query helper for `after_id` polling
- tool payload/result builder in `tools/session.rs`
- provider/tool-schema expectations

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_events -- --nocapture`

Expected: PASS

### Task 4: Add async `session_wait`

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`
- Modify: `crates/app/src/tools/catalog.rs`
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/session.rs`
- Modify: `crates/app/src/provider/mod.rs`
- Test: `crates/app/src/conversation/tests.rs`
- Test: `crates/app/src/tools/mod.rs`
- Test: `crates/app/src/provider/mod.rs`

**Step 1: Write the failing test**

Add tests for:

- `session_wait` returns `ok` for an already terminal session
- `session_wait` returns `timeout` for a non-terminal session
- root runtime tool view includes `session_wait`
- delegated child tool view excludes `session_wait`

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_wait -- --nocapture`

Expected: FAIL because the tool is not yet advertised or executed.

**Step 3: Write minimal implementation**

Implement:

- catalog/definition entry for `session_wait`
- async handling in `DefaultAppToolDispatcher`
- bounded polling helper with visibility enforcement
- provider/tool-schema expectations

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_wait -- --nocapture`

Expected: PASS

### Task 5: Persist terminal outcomes from synchronous delegate execution

**Files:**
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/tools/delegate.rs`
- Test: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing test**

Add delegate tests for:

- successful child delegate stores terminal outcome
- failed child delegate stores terminal outcome
- timed-out child delegate stores terminal outcome

**Step 2: Run test to verify it fails**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_terminal_outcome -- --nocapture`

Expected: FAIL because delegate currently updates session state and events but does not persist a durable terminal outcome.

**Step 3: Write minimal implementation**

Persist the exact returned delegate tool outcome into the new repository table before returning success/error/timeout from `execute_delegate_tool(...)`.

**Step 4: Run test to verify it passes**

Run: `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_terminal_outcome -- --nocapture`

Expected: PASS

### Task 6: Update docs and run full verification

**Files:**
- Modify: `docs/product-specs/index.md`
- Modify: `docs/roadmap.md`

**Step 1: Update docs**

Document:

- `session_events`
- `session_wait`
- durable terminal delegate outcomes
- continued synchronous-only delegate execution

**Step 2: Run focused regression**

Run:

- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_events -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_wait -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_terminal_outcome -- --nocapture`

Expected: PASS

**Step 3: Run full verification**

Run:

- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app -- --nocapture`
- `/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all --check`
- `git diff --check`

Expected:

- formatting clean
- full `loongclaw-app` suite green
- no whitespace or merge-marker issues

Plan complete and saved to `docs/plans/2026-03-12-session-observability-implementation-plan.md`. Execution path for this session: continue locally in the current worktree with TDD against this plan.
