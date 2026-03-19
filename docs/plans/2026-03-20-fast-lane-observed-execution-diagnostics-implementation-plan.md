# Fast-Lane Observed Execution Diagnostics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend fast-lane tool batch diagnostics so persisted events and CLI summaries expose observed execution concurrency and latency in addition to configured parallel execution metadata.

**Architecture:** Keep the existing `fast_lane_tool_batch` event as the single source of truth, enrich `ToolBatchExecutionTrace` during real execution, correct planned `execution_mode` when the effective in-flight cap is `1`, and extend analytics/CLI summary folding to surface configured-vs-observed comparisons without introducing thresholds or new event types.

**Tech Stack:** Rust, Tokio async execution, serde/JSON analytics folding, CLI summary formatting, memory-sqlite conversation integration tests

---

### Task 1: Write the design and plan artifacts

**Files:**
- Create: `docs/plans/2026-03-20-fast-lane-observed-execution-diagnostics-design.md`
- Create: `docs/plans/2026-03-20-fast-lane-observed-execution-diagnostics-implementation-plan.md`

**Step 1: Verify the design artifact exists**

Run: `test -f docs/plans/2026-03-20-fast-lane-observed-execution-diagnostics-design.md`
Expected: exit 0

**Step 2: Verify the implementation plan artifact exists**

Run: `test -f docs/plans/2026-03-20-fast-lane-observed-execution-diagnostics-implementation-plan.md`
Expected: exit 0

### Task 2: Add failing trace-level execution coverage

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`

**Step 1: Add a failing observed-metrics test**

Add a `turn_engine` test that executes a mixed fast-lane batch through a custom
dispatcher and asserts:

- batch observed peak in-flight is tracked
- batch observed wall time is populated
- parallel segments capture observed peak greater than one
- sequential segments capture observed peak of one
- `max_in_flight=1` does not classify a segment as planned `parallel`

**Step 2: Run the targeted test to verify RED**

Run: `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app observed_fast_lane_execution -- --test-threads=1`
Expected: FAIL because observed metrics are not captured yet

### Task 3: Add failing analytics and CLI summary coverage

**Files:**
- Modify: `crates/app/src/conversation/analytics.rs`
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Add failing analytics tests**

Extend fast-lane analytics tests to assert:

- latest observed peak in-flight and wall time are parsed
- aggregate observed peak and wall-time rollups are accumulated
- degraded parallel segments are counted from segment observations

**Step 2: Add failing CLI summary assertions**

Extend fast-lane summary formatting tests to assert:

- configured max in-flight and observed peak are both rendered
- observed wall-time rollups are rendered
- latest segment formatting includes observed metrics when present

**Step 3: Add failing conversation persistence assertions**

Extend fast-lane event persistence coverage to assert that persisted batch
events include the new observed batch and segment fields.

**Step 4: Run the focused test set to verify RED**

Run:

- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app summarize_fast_lane_tool_batch_events -- --test-threads=1`
- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app fast_lane_summary -- --test-threads=1`
- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app persists_fast_lane_tool_batch_event -- --test-threads=1`

Expected: FAIL because the new observed fields are not emitted or summarized yet

### Task 4: Implement observed execution trace capture

**Files:**
- Modify: `crates/app/src/conversation/turn_engine.rs`

**Step 1: Extend the trace model**

Add observed batch and segment fields and helper methods for event payload
serialization.

**Step 2: Correct effective parallel classification**

Update segment execution-mode selection so planned `parallel` requires an
effective in-flight cap greater than one.

**Step 3: Capture observed metrics during execution**

Record:

- batch wall time
- batch observed peak in-flight
- per-segment wall time
- per-segment observed peak in-flight

Do this in the real execution path so partially executed batches still produce a
truthful trace.

**Step 4: Run the targeted trace test to verify GREEN**

Run: `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app observed_fast_lane_execution -- --test-threads=1`
Expected: PASS

### Task 5: Extend analytics folding and fast-lane summary output

**Files:**
- Modify: `crates/app/src/conversation/analytics.rs`
- Modify: `crates/app/src/chat.rs`
- Modify: `crates/app/src/conversation/tests.rs`

**Step 1: Extend analytics summary types and parsing**

Add optional/latest and aggregate observed fields while keeping summary parsing
backward-compatible with older event payloads.

**Step 2: Extend fast-lane CLI rendering**

Render configured-vs-observed execution values clearly and keep the output
compact enough for operator use.

**Step 3: Extend persisted event assertions**

Assert the observed fields survive the full persistence path.

**Step 4: Run the focused test set to verify GREEN**

Run:

- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app summarize_fast_lane_tool_batch_events -- --test-threads=1`
- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app fast_lane_summary -- --test-threads=1`
- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app persists_fast_lane_tool_batch_event -- --test-threads=1`

Expected: PASS

### Task 6: Run broader verification and prepare clean delivery

**Files:**
- Review all touched files

**Step 1: Run focused fast-lane regression coverage**

Run: `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test -p loongclaw-app fast_lane -- --test-threads=1`
Expected: PASS

**Step 2: Run repo-level formatting and all-features verification**

Run:

- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo fmt --all -- --check`
- `env HOME=/tmp/loongclaw-test-home USERPROFILE=/tmp/loongclaw-test-home RUSTUP_HOME=/Users/chum/.rustup CARGO_HOME=/Users/chum/.cargo /Users/chum/.cargo/bin/cargo test --workspace --all-features -- --test-threads=1`

Expected: PASS

**Step 3: Inspect final scope**

Run:

- `git status --short`
- `git diff --stat`
- `git diff`

Expected: only fast-lane observed diagnostics code, tests, and plan artifacts are changed
