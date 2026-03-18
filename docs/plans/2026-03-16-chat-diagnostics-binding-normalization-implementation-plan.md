# Chat Diagnostics Binding Normalization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Normalize the remaining chat diagnostic and discovery-first session-history helpers onto `ConversationRuntimeBinding<'_>`.

**Architecture:** Keep the slice narrow. Update only the user-facing chat diagnostic helpers and the discovery-first summary helper so they stop accepting raw optional kernel context and instead preserve explicit runtime binding semantics through their surface.

**Tech Stack:** Rust, Tokio tests, GitHub issue-first workflow

---

### Task 1: Add failing tests for explicit binding-based helper calls

**Files:**
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Write the failing test**

Add:

1. `chat.rs` async tests that assert real `/history`,
   `/safe_lane_summary`, and `/turn_checkpoint_summary` output for explicit
   direct and kernel bindings
2. a `conversation/tests.rs` async test that calls the normalized
   discovery-first helper with explicit `ConversationRuntimeBinding::direct()`
   and `ConversationRuntimeBinding::kernel(...)`
3. a `conversation/tests.rs` async compatibility test that still calls
   `load_discovery_first_event_summary(...)` with `None` and `Some(&ctx)`

**Step 2: Run test to verify it fails**

Run:

- `cargo test -p loongclaw-app chat::tests::print_history_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::safe_lane_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::turn_checkpoint_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_preserves_public_kernel_context_signature -- --exact --nocapture`

Expected: FAIL because the output builders and compatibility-preserving helper
split do not exist yet.

### Task 2: Normalize helper signatures and leaf conversions

**Files:**
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/conversation/session_history.rs`

**Step 1: Write minimal implementation**

1. Change the remaining chat diagnostic helper signatures to accept
   `ConversationRuntimeBinding<'_>`
2. Update command dispatch call sites to pass explicit binding values
3. Introduce binding-aware output builders underneath the `print_*` shells so
   tests can assert routing and formatted output directly
4. Keep `load_discovery_first_event_summary(...)` as the public compatibility
   wrapper while moving the binding-first implementation into an internal helper
5. Use `binding.kernel_context()` only at the leaf where a lower-level branch
   still truly needs optional kernel context

**Step 2: Run targeted tests to verify they pass**

Run:

- `cargo test -p loongclaw-app chat::tests::print_history_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::safe_lane_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::turn_checkpoint_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_preserves_public_kernel_context_signature -- --exact --nocapture`

Expected: PASS

### Task 3: Refresh docs and full verification

**Files:**
- Modify: `docs/SECURITY.md`
- Modify: `docs/plans/2026-03-16-chat-diagnostics-binding-normalization-design.md`
- Modify: `docs/plans/2026-03-16-chat-diagnostics-binding-normalization-implementation-plan.md`

**Step 1: Update docs**

Record that the chat diagnostic boundary now preserves explicit runtime binding
semantics, that the public discovery-first helper remains a compatibility
wrapper, and note any remaining follow-up seams outside this slice.

**Step 2: Run focused verification**

Run:

- `cargo test -p loongclaw-app chat::tests::print_history_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::safe_lane_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app chat::tests::turn_checkpoint_summary_output_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_accepts_explicit_runtime_binding -- --exact --nocapture`
- `cargo test -p loongclaw-app conversation::tests::load_discovery_first_event_summary_preserves_public_kernel_context_signature -- --exact --nocapture`
- `cargo test -p loongclaw-app load_turn_checkpoint_event_summary -- --test-threads=1`

Expected: PASS

**Step 3: Run full verification**

Run:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`

Expected: PASS

**Step 4: Commit**

```bash
git add docs/plans/2026-03-16-chat-diagnostics-binding-normalization-design.md \
        docs/plans/2026-03-16-chat-diagnostics-binding-normalization-implementation-plan.md \
        docs/SECURITY.md \
        crates/app/src/chat.rs \
        crates/app/src/conversation/session_history.rs \
        crates/app/src/conversation/tests.rs
git commit -m "refactor: normalize chat diagnostics runtime binding"
```
