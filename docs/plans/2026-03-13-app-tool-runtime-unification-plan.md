# App Tool Runtime Unification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Unify non-`delegate` app-tool execution behind one authoritative async runtime helper so the advertised tool surface and actual execution boundary stay aligned.

**Architecture:** Keep `delegate` in the turn loop, but move reusable async delegate support into `tools::delegate` and introduce a central async app-tool executor in `tools::mod`. `DefaultAppToolDispatcher` becomes a thin visibility/config wrapper over that executor.

**Tech Stack:** Rust, Tokio, serde_json, sqlite-backed session repository, existing conversation and tool test harnesses

---

### Task 1: Lock the target runtime behavior with failing tests

**Files:**
- Modify: `crates/app/src/tools/mod.rs`

**Step 1: Write the failing tests**

Add async tests that expect:

- a new async app-tool runtime helper can execute `session_wait`
- the helper returns `sessions_send_not_configured` when `sessions_send` is requested without app config
- the helper returns `app_tool_requires_turn_loop_dispatch: delegate` for `delegate`

**Step 2: Run tests to verify they fail**

Run:

```bash
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app execute_app_tool_runtime_ -- --nocapture --test-threads=1
```

Expected: fail because the helper or support plumbing does not exist yet.

**Step 3: Commit**

```bash
git add crates/app/src/tools/mod.rs
git commit -m "test(app): lock app tool runtime unification behavior"
```

### Task 2: Move reusable delegate-async runtime pieces into the tools layer

**Files:**
- Modify: `crates/app/src/tools/delegate.rs`
- Modify: `crates/app/src/conversation/turn_engine.rs`

**Step 1: Add shared delegate-async runtime types and helpers**

Move or re-home:

- `AsyncDelegateSpawnRequest`
- `AsyncDelegateSpawner`
- async spawn failure persistence helpers
- detached spawn execution helper
- `execute_delegate_async_with_config(...)`

Keep the subprocess spawner implementation in `turn_engine.rs`, but make it implement the moved
trait from `tools::delegate`.

**Step 2: Run focused tests**

Run:

```bash
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async_ -- --nocapture --test-threads=1
```

Expected: delegate-async behavior remains green.

**Step 3: Commit**

```bash
git add crates/app/src/tools/delegate.rs crates/app/src/conversation/turn_engine.rs
git commit -m "refactor(app): move delegate async runtime helpers into tools layer"
```

### Task 3: Add the authoritative async app-tool runtime executor

**Files:**
- Modify: `crates/app/src/tools/mod.rs`
- Modify: `crates/app/src/tools/messaging.rs`

**Step 1: Implement the helper**

Add:

- a small runtime support struct for optional app config and async delegate spawner
- `execute_app_tool_with_runtime_support(...)` as the async authoritative executor for all
  non-`delegate` app tools

Route:

- session tools through the existing session helper
- `memory_search` through the existing memory helper
- `session_wait` through the existing async wait helper
- `sessions_send` through messaging with explicit missing-config error
- `delegate_async` through the shared delegate helper
- `delegate` to `app_tool_requires_turn_loop_dispatch: delegate`

**Step 2: Run focused tests**

Run:

```bash
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app execute_app_tool_runtime_ -- --nocapture --test-threads=1
```

Expected: the new helper tests pass.

**Step 3: Commit**

```bash
git add crates/app/src/tools/mod.rs crates/app/src/tools/messaging.rs
git commit -m "feat(app): centralize async app tool execution"
```

### Task 4: Simplify the default app dispatcher to use the centralized helper

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`

**Step 1: Replace dispatcher-local special cases**

Have `DefaultAppToolDispatcher::execute_app_tool(...)`:

- keep visibility enforcement
- compute effective tool config
- call the centralized async app-tool executor

Retain `TurnLoopAppToolDispatcher` interception for `delegate`.

**Step 2: Run focused dispatcher and conversation tests**

Run:

```bash
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app sessions_send_ -- --nocapture --test-threads=1
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app session_wait_ -- --nocapture --test-threads=1
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app delegate_async_ -- --nocapture --test-threads=1
```

Expected: existing dispatcher behavior remains green.

**Step 3: Commit**

```bash
git add crates/app/src/conversation/turn_engine.rs
git commit -m "refactor(app): route default dispatcher through central app executor"
```

### Task 5: Verify the full slice and update any affected docs

**Files:**
- Modify only if needed after code review: `docs/product-specs/index.md`, `docs/roadmap.md`

**Step 1: Run formatting and full verification**

Run:

```bash
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo fmt --all
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-app -- --nocapture --test-threads=1
/Users/chum/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo test -p loongclaw-daemon --no-run
```

**Step 2: Inspect worktree cleanliness**

Run:

```bash
git status --short
git log --oneline -8
```

**Step 3: Commit follow-up formatting or doc adjustments if needed**

```bash
git add <files>
git commit -m "style(rust): format app tool runtime unification changes"
```
