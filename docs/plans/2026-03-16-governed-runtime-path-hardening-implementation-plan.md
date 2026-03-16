# Governed Runtime Path Hardening Implementation Plan

**Goal:** Close the highest-value conversation runtime governed/direct drift by
preserving async delegate child binding, failing closed for kernel-bound history
reads, and updating docs to match the real architecture state.

**Architecture:** Keep the patch local to conversation runtime spawn/history
helpers and architecture/security docs. Use explicit `ConversationRuntimeBinding`
as the authority boundary instead of implicit downgrade behavior.

**Tech Stack:** Rust, Tokio tests, `loongclaw-app`, GitHub issue-first workflow

---

### Task 1: Lock the scope in docs and GitHub issue text

**Files:**
- Create: `docs/plans/2026-03-16-governed-runtime-path-hardening-design.md`
- Create: `docs/plans/2026-03-16-governed-runtime-path-hardening-implementation-plan.md`

**Step 1: Confirm the target seams**

Run:
- `rg -n "ConversationRuntimeBinding::direct\\(\\)" crates/app/src/conversation/runtime.rs crates/app/src/conversation/tests.rs`
- `rg -n "load_assistant_contents_from_session_window" crates/app/src/conversation/session_history.rs`

Expected: the delegate-child direct override and the session-history fallback
site are both enumerated.

**Step 2: Draft the delivery issue**

Open a GitHub bug issue describing:
- async delegate child binding drift
- kernel-bound history fallback drift
- the scoped plan to harden those paths first

Expected: issue exists before PR creation and uses the repository template.

### Task 2: Add RED tests for the hardening slice

**Files:**
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Add delegate-child binding regression tests**

Cover both:
- async delegate spawn request records the inherited binding
- local child runtime spawn forwards the same binding into
  `run_started_delegate_child_turn_with_runtime(...)`

**Step 2: Add kernel-bound history fail-closed regression**

Add a test that binds a kernel context whose memory adapter fails the window
request and assert `load_turn_checkpoint_event_summary(...)` returns an error
instead of silently reading sqlite.

**Step 3: Run targeted tests to confirm RED**

Run:
- `cargo test -p loongclaw-app delegate_async -- --test-threads=1`
- `cargo test -p loongclaw-app load_turn_checkpoint_event_summary -- --test-threads=1`

Expected: FAIL before implementation because binding is still dropped and
kernel-bound history still falls back.

### Task 3: Preserve binding through async delegate spawn

**Files:**
- Modify: `crates/app/src/conversation/runtime.rs`
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Extend the spawn request**

Add owned inherited kernel authority to `AsyncDelegateSpawnRequest` so detached
spawns can reconstruct `ConversationRuntimeBinding` without borrowing parent
stack state.

**Step 2: Forward the inherited binding**

Pass the current binding from async delegate scheduling into the spawn request
and then into `run_started_delegate_child_turn_with_runtime(...)`.

**Step 3: Update tests and fakes**

Adjust fake/local async delegate spawners to preserve and/or record the binding.

### Task 4: Fail closed for kernel-bound history reads

**Files:**
- Modify: `crates/app/src/conversation/session_history.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Separate direct mode from kernel failure**

Make `load_assistant_contents_from_session_window(...)` branch on
`ConversationRuntimeBinding` directly.

**Step 2: Return explicit errors for governed failures**

If the kernel memory-window call errors or returns non-`ok`, return a clear
history load error instead of hitting sqlite fallback.

**Step 3: Keep direct mode stable**

Retain the direct sqlite path only for `ConversationRuntimeBinding::Direct`.

### Task 5: Refresh architecture/security docs

**Files:**
- Modify: `ARCHITECTURE.md`
- Modify: `docs/SECURITY.md`

**Step 1: Remove overclaiming**

Replace "all execution paths route through the kernel" / "no shadow paths" with
language that matches the current governed-versus-direct split.

**Step 2: Describe the current enforcement boundary**

Document that conversation runtime now uses explicit binding semantics, while
some outer app/channel paths still remain direct follow-up work.

### Task 6: Verify, commit, and deliver

**Files:**
- Modify only the files in this scoped slice

**Step 1: Run targeted verification**

Run:
- `cargo test -p loongclaw-app delegate_async -- --test-threads=1`
- `cargo test -p loongclaw-app load_turn_checkpoint_event_summary -- --test-threads=1`

Expected: PASS

**Step 2: Run full package verification**

Run:
- `cargo fmt --all`
- `cargo fmt --all -- --check`
- `cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings`
- `cargo test -p loongclaw-app --all-features -- --test-threads=1`

Expected: PASS

**Step 3: Review the scoped diff and commit**

Run:
- `git status --short`
- `git diff --cached --name-only`
- `git diff --cached`

Expected: only the governed-runtime hardening slice is staged before commit.
