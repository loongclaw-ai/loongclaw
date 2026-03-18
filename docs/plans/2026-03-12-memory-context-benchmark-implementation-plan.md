# Memory Context Benchmark Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a repository-native benchmark CLI that measures memory prompt-context hydration across `window_only`, `summary_rebuild`, and `summary_steady_state` scenarios and emits a JSON report with an optional speedup gate.

**Architecture:** Reuse the existing `crates/bench` reporting model and `crates/daemon` subcommand exposure. Seed deterministic SQLite history through the public direct memory APIs, then measure `load_prompt_context(...)` across distinct benchmark scenarios and publish structured numeric stats and gate results.

**Tech Stack:** Rust, rusqlite via `loongclaw-app`, clap, serde/serde_json, cargo test

---

### Task 1: Add failing benchmark tests

**Files:**
- Modify: `crates/bench/src/lib.rs`
- Modify: `crates/daemon/src/main.rs`
- Test: `crates/bench/src/lib.rs`
- Test: `crates/daemon/src/tests.rs`

**Step 1: Write the failing tests**

Add tests that prove:

- memory context benchmark rejects invalid iteration/window parameters
- memory context benchmark writes a parseable JSON report containing all three
  scenarios
- daemon CLI parses and dispatches the new `benchmark-memory-context` command

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p loongclaw-bench memory_context_benchmark -- --nocapture
cargo test -p loongclaw-daemon benchmark_memory_context -- --nocapture
```

Expected:

- tests fail before implementation exists

### Task 2: Implement benchmark report generator

**Files:**
- Modify: `crates/bench/Cargo.toml`
- Modify: `crates/bench/src/lib.rs`

**Step 1: Write the minimal implementation**

- add a minimal `loongclaw-app` dependency with only the memory feature needed
- implement:
  - report structs
  - deterministic history seeding helpers
  - `window_only`, `summary_rebuild`, and `summary_steady_state` samplers
  - JSON report writer
  - optional p95 speedup gate

**Step 2: Run benchmark crate tests to verify they pass**

Run:

```bash
cargo test -p loongclaw-bench memory_context_benchmark -- --nocapture
```

Expected:

- new benchmark tests pass

### Task 3: Expose daemon CLI

**Files:**
- Modify: `crates/daemon/src/main.rs`
- Modify: `crates/daemon/src/tests.rs` if needed

**Step 1: Add the new subcommand**

- add `BenchmarkMemoryContext` clap command
- wire it to `run_memory_context_benchmark_cli(...)`
- keep defaults aligned with the other benchmark commands

**Step 2: Run daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon benchmark_memory_context -- --nocapture
```

Expected:

- CLI parse/dispatch tests pass

### Task 4: End-to-end benchmark verification

**Files:**
- Verify: `crates/bench/src/lib.rs`
- Verify: `crates/daemon/src/main.rs`

**Step 1: Format**

```bash
cargo fmt --all
```

**Step 2: Run repository verification**

```bash
cargo test -p loongclaw-bench -- --nocapture
cargo test -p loongclaw-daemon -- --nocapture
cargo test --workspace --all-features
./scripts/check_architecture_boundaries.sh
```

Expected:

- formatting clean
- benchmark crate passes
- daemon crate passes
- workspace passes
- architecture boundary script passes

**Step 3: Run one real benchmark**

Run:

```bash
cargo run -p loongclaw-daemon -- benchmark-memory-context --output target/benchmarks/memory-context-report.json
```

Expected:

- JSON report is written
- CLI prints summary metrics and gate status

### Task 5: Commit cleanly

**Files:**
- Modify: `crates/bench/Cargo.toml`
- Modify: `crates/bench/src/lib.rs`
- Modify: `crates/daemon/src/main.rs`
- Modify: `crates/daemon/src/tests.rs` if needed

**Step 1: Inspect isolation**

```bash
git status --short
git diff --cached --name-only
git diff --cached
```

**Step 2: Commit**

```bash
git add crates/bench/Cargo.toml crates/bench/src/lib.rs crates/daemon/src/main.rs crates/daemon/src/tests.rs
git commit -m "feat(bench): add memory context benchmark"
```
