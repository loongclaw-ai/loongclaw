# Conversation Turn Middleware Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce a minimal app-layer turn middleware foundation that composes cross-cutting conversation behaviors without changing LoongClaw's kernel trust boundary or replacing the existing `ConversationContextEngine` authority.

**Architecture:** Add a new ordered turn-middleware seam in `crates/app/src/conversation/` and adapt the current coordinator/turn-loop flow to invoke it around the existing context-engine and tool-execution path. The first slice should keep current behavior stable by shipping compatibility middleware adapters rather than inventing new product behaviors in the same change set.

**Tech Stack:** Rust, async conversation runtime traits, `loongclaw-app` tests, cargo test, cargo clippy

---

### Task 1: Lock the scope in docs and tests

**Files:**
- Create: `docs/design-docs/reference-runtime-comparison.md`
- Create: `docs/plans/2026-03-18-conversation-turn-middleware-foundation-implementation-plan.md`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Re-read the current app-layer orchestration hotspots**

Run:

```bash
rg -n "ContextEngine|assemble_context|after_turn|compact_context|prepare_subagent_spawn|on_subagent_ended|handle_turn_with_runtime|turn_loop" crates/app/src/conversation
```

Expected:

- the current cross-cutting behavior entrypoints are enumerated

**Step 2: Add failing tests that define the foundation boundary**

Add focused tests proving:

- middleware ordering is deterministic
- middleware can mutate turn-local metadata without mutating kernel policy state
- compatibility adapters preserve existing context-engine hooks

**Step 3: Run targeted tests to confirm RED**

Run:

```bash
cargo test -p loongclaw-app turn_middleware_ -- --test-threads=1
```

Expected:

- FAIL because the middleware seam does not exist yet

### Task 2: Add the middleware trait and registry

**Files:**
- Create: `crates/app/src/conversation/turn_middleware.rs`
- Create: `crates/app/src/conversation/turn_middleware_registry.rs`
- Modify: `crates/app/src/conversation/mod.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Introduce the smallest stable middleware contract**

Add a trait that supports ordered hooks around the current turn lifecycle, such
as:

- before context assembly
- after context assembly
- before model dispatch
- after turn completion
- before subagent spawn
- after subagent completion

Keep the request/response context explicit and typed. Do not hide mutable state
behind globals.

**Step 2: Add an ordered registry**

Implement a registry with:

- normalized middleware ids
- deterministic ordering
- metadata listing for diagnostics
- env/config-independent registration rules

Use the same registry style already used by context engines and memory systems.

**Step 3: Run targeted middleware tests**

Run:

```bash
cargo test -p loongclaw-app turn_middleware_registry_ -- --test-threads=1
```

Expected:

- PASS

### Task 3: Bridge the current context-engine hooks through compatibility middleware

**Files:**
- Modify: `crates/app/src/conversation/context_engine.rs`
- Modify: `crates/app/src/conversation/context_engine_registry.rs`
- Modify: `crates/app/src/conversation/turn_middleware.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Add compatibility adapters**

Create middleware implementations that delegate to the existing context-engine
hooks for:

- bootstrap
- ingest
- after-turn
- compaction
- subagent lifecycle

**Step 2: Keep context assembly ownership unchanged**

Do not move final prompt/context projection out of `ConversationContextEngine`.
Middleware may enrich inputs and outputs, but context engines remain the
authority for assembled prompt messages.

**Step 3: Run compatibility regression tests**

Run:

```bash
cargo test -p loongclaw-app context_engine_ turn_middleware_ -- --test-threads=1
```

Expected:

- PASS

### Task 4: Integrate middleware into coordinator and turn loop

**Files:**
- Modify: `crates/app/src/conversation/turn_coordinator.rs`
- Modify: `crates/app/src/conversation/turn_loop.rs`
- Modify: `crates/app/src/conversation/runtime.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Add the failing end-to-end regression tests**

Add tests that prove:

- middleware runs in the expected order during a normal provider turn
- no-kernel execution rules stay unchanged
- subagent hook ordering remains deterministic

**Step 2: Wire the middleware stack into the existing turn flow**

Refactor the conversation runtime so middleware:

- runs around existing turn stages
- receives the same turn-local data for CLI and channel-driven turns
- never bypasses kernel-mediated tool execution

**Step 3: Keep the slice narrow**

Do not add new product features in this change. No scheduler, no new tools, no
new channels.

**Step 4: Run targeted conversation coverage**

Run:

```bash
cargo test -p loongclaw-app handle_turn_with_runtime safe_lane turn_middleware_ -- --test-threads=1
```

Expected:

- PASS

### Task 5: Add diagnostics and docs for the new seam

**Files:**
- Modify: `crates/daemon/src/main.rs`
- Modify: `crates/daemon/tests/integration/cli_tests.rs`
- Modify: `docs/ROADMAP.md`

**Step 1: Expose a lightweight diagnostic surface**

Add a daemon command such as `list-turn-middleware --json` or fold middleware
metadata into an existing runtime-inspection command.

**Step 2: Update docs**

Document that:

- turn middleware is an app-layer composition seam
- it does not replace context engines or kernel policy
- future automation and memory enrichment work should use this seam

**Step 3: Run CLI parse and doc verification**

Run:

```bash
cargo test -p loongclaw-daemon turn_middleware -- --test-threads=1
rg -n "turn middleware|middleware foundation" docs/ROADMAP.md docs/design-docs/reference-runtime-comparison.md
```

Expected:

- daemon tests PASS
- docs contain the intended wording

### Task 6: Run full verification before delivery

**Files:**
- Modify: `crates/app/src/conversation/*`
- Modify: `crates/daemon/src/main.rs`
- Modify: `crates/daemon/tests/integration/cli_tests.rs`
- Modify: `docs/ROADMAP.md`

**Step 1: Format**

Run:

```bash
cargo fmt --all -- --check
```

Expected:

- PASS

**Step 2: Run app crate verification**

Run:

```bash
cargo test -p loongclaw-app -- --test-threads=1
cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings
```

Expected:

- PASS

**Step 3: Run daemon verification**

Run:

```bash
cargo test -p loongclaw-daemon -- --test-threads=1
cargo clippy -p loongclaw-daemon --all-targets --all-features -- -D warnings
```

Expected:

- PASS

**Step 4: Run workspace CI parity**

Run:

```bash
cargo test --workspace
cargo test --workspace --all-features
```

Expected:

- PASS

**Step 5: Review the scoped diff**

Run:

```bash
git diff -- crates/app/src/conversation crates/daemon/src/main.rs crates/daemon/tests/integration/cli_tests.rs docs/design-docs/reference-runtime-comparison.md docs/plans/2026-03-18-conversation-turn-middleware-foundation-implementation-plan.md docs/ROADMAP.md
```

Expected:

- only the intended middleware-foundation slice is present
