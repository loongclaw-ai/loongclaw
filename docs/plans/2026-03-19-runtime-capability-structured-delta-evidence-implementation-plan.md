# Runtime Capability Structured Delta Evidence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist snapshot-backed runtime delta evidence inside runtime-capability artifacts and surface a compact digest in `show`, `index`, and `plan` without adding any mutation path.

**Architecture:** Reuse the existing runtime-experiment snapshot-delta machinery as the single source of truth, attach the optional delta to `RuntimeCapabilitySourceRunSummary`, derive a compact family-level digest from candidate deltas, and keep the slice backward-compatible for old artifacts and no-snapshot runs.

**Tech Stack:** Rust, Cargo, Clap, Serde, existing daemon integration tests

---

### Task 1: Lock the design artifacts into the branch

**Files:**
- Create: `docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-design.md`
- Create: `docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-implementation-plan.md`

**Step 1: Verify the design doc exists**

Run: `test -f docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-design.md`
Expected: exit code 0

**Step 2: Verify the implementation plan exists**

Run: `test -f docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-implementation-plan.md`
Expected: exit code 0

**Step 3: Commit the plan docs when implementation is complete**

```bash
git add docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-design.md \
  docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-implementation-plan.md
```

### Task 2: Add failing CLI/integration tests for delta evidence capture

**Files:**
- Modify: `crates/daemon/tests/integration/runtime_capability_cli.rs`
- Modify: `crates/daemon/tests/integration/cli_tests.rs`

**Step 1: Extend CLI parse coverage only if new flags or output schema contracts need explicit assertions**

If no CLI flag changes are needed, keep `cli_tests.rs` unchanged.

**Step 2: Write a failing integration test for propose-with-recorded-snapshots**

Add a test that:

- creates a finished runtime experiment with recorded baseline/result snapshots
- runs `execute_runtime_capability_propose_command`
- asserts `candidate.source_run.snapshot_delta.is_some()`
- asserts at least one expected changed surface is present in the persisted
  delta

**Step 3: Run the targeted test to verify RED**

Run: `cargo test -p loongclaw-daemon runtime_capability_propose_persists_snapshot_delta --test integration -- --nocapture`
Expected: FAIL because the capability artifact does not yet expose
`snapshot_delta`

**Step 4: Write a failing integration test for the no-snapshot path**

Add a test that:

- creates a finished runtime experiment without recorded comparable snapshots
- runs `execute_runtime_capability_propose_command`
- asserts `candidate.source_run.snapshot_delta.is_none()`

**Step 5: Run the targeted no-snapshot test to verify RED or coverage gap**

Run: `cargo test -p loongclaw-daemon runtime_capability_propose_leaves_snapshot_delta_empty_without_recorded_snapshots --test integration -- --nocapture`
Expected: FAIL until the new field exists

**Step 6: Write a failing integration test for broken recorded snapshots**

Add a test that:

- creates a finished runtime experiment with recorded snapshot paths
- removes or corrupts one referenced snapshot file
- runs `execute_runtime_capability_propose_command`
- asserts the command returns an error that makes the missing snapshot evidence
  explicit

**Step 7: Run the broken-snapshot test to verify RED**

Run: `cargo test -p loongclaw-daemon runtime_capability_propose_rejects_broken_recorded_snapshot_delta --test integration -- --nocapture`
Expected: FAIL until propose validates the recorded snapshot delta path

### Task 3: Add failing aggregation tests for index and plan

**Files:**
- Modify: `crates/daemon/tests/integration/runtime_capability_cli.rs`

**Step 1: Write a failing family-index test for aggregated delta digest**

Add a test that:

- creates two compatible accepted candidates with snapshot delta evidence
- runs `execute_runtime_capability_index_command`
- asserts the matching family reports:
  - `delta_candidate_count == 2`
  - sorted `changed_surfaces` union from the underlying deltas

**Step 2: Run the family-index test to verify RED**

Run: `cargo test -p loongclaw-daemon runtime_capability_index_reports_delta_evidence_digest --test integration -- --nocapture`
Expected: FAIL because the evidence digest does not yet include the new fields

**Step 3: Write a failing planner test for surfaced delta digest**

Add a test that:

- creates a promotable family with snapshot delta evidence
- runs `execute_runtime_capability_plan_command`
- asserts the emitted evidence includes the same `delta_candidate_count` and
  `changed_surfaces`

**Step 4: Run the planner test to verify RED**

Run: `cargo test -p loongclaw-daemon runtime_capability_plan_surfaces_delta_evidence_digest --test integration -- --nocapture`
Expected: FAIL until plan reuses the new digest

### Task 4: Implement the optional candidate-level snapshot delta

**Files:**
- Modify: `crates/daemon/src/runtime_capability_cli.rs`
- Modify: `crates/daemon/src/runtime_experiment_cli.rs`

**Step 1: Expose or extract one reusable snapshot-delta helper from the runtime-experiment path**

Keep one source of truth for snapshot comparison semantics.

**Step 2: Extend `RuntimeCapabilitySourceRunSummary`**

Add:

```rust
pub snapshot_delta: Option<RuntimeExperimentSnapshotDelta>
```

**Step 3: Populate the field during propose**

Implement minimal logic:

- `None` when the source run has no usable recorded snapshots
- `Some(delta)` when the run has matching recorded snapshots
- error on broken recorded snapshot references

**Step 4: Re-run the propose-focused tests**

Run:

- `cargo test -p loongclaw-daemon runtime_capability_propose_persists_snapshot_delta --test integration -- --nocapture`
- `cargo test -p loongclaw-daemon runtime_capability_propose_leaves_snapshot_delta_empty_without_recorded_snapshots --test integration -- --nocapture`
- `cargo test -p loongclaw-daemon runtime_capability_propose_rejects_broken_recorded_snapshot_delta --test integration -- --nocapture`

Expected: PASS

### Task 5: Implement the family-level delta digest

**Files:**
- Modify: `crates/daemon/src/runtime_capability_cli.rs`
- Modify: `crates/daemon/tests/integration/runtime_capability_cli.rs`

**Step 1: Extend `RuntimeCapabilityEvidenceDigest`**

Add:

- `delta_candidate_count: usize`
- `changed_surfaces: Vec<String>`

**Step 2: Derive the compact digest from candidate-level deltas**

Rules:

- count only candidates with `snapshot_delta.is_some()`
- expose a stable sorted union of changed surface names
- keep the digest compact; do not inline all before/after values at family level

**Step 3: Re-run the failing index test**

Run: `cargo test -p loongclaw-daemon runtime_capability_index_reports_delta_evidence_digest --test integration -- --nocapture`
Expected: PASS

### Task 6: Surface the new evidence in `show` and `plan`

**Files:**
- Modify: `crates/daemon/src/runtime_capability_cli.rs`
- Modify: `crates/daemon/tests/integration/runtime_capability_cli.rs`

**Step 1: Update text rendering for `show`**

Render compact candidate-level delta lines only when evidence exists.

**Step 2: Ensure `plan` reuses the evidence digest unchanged**

Do not invent a second delta summary model.

**Step 3: Re-run the planner-focused failing test**

Run: `cargo test -p loongclaw-daemon runtime_capability_plan_surfaces_delta_evidence_digest --test integration -- --nocapture`
Expected: PASS

### Task 7: Update product docs and roadmap

**Files:**
- Modify: `docs/product-specs/runtime-capability.md`
- Modify: `docs/ROADMAP.md`

**Step 1: Update the product spec**

Document that capability artifacts may persist snapshot-backed runtime delta
evidence and that the review/planning ladder now carries structured change
evidence below any future promotion executor.

**Step 2: Update the roadmap**

Clarify that the runtime-capability track now preserves structured delta
evidence as the prerequisite for later promotion-materialization work.

**Step 3: Verify the docs mention the new layer**

Run: `rg -n "runtime-capability|snapshot delta|structured delta" docs/product-specs/runtime-capability.md docs/ROADMAP.md`
Expected: matches in both files

### Task 8: Run focused verification, then full CI-parity verification

**Files:**
- Modify: `crates/daemon/src/runtime_capability_cli.rs`
- Modify: `crates/daemon/src/runtime_experiment_cli.rs`
- Modify: `crates/daemon/tests/integration/runtime_capability_cli.rs`
- Modify: `docs/product-specs/runtime-capability.md`
- Modify: `docs/ROADMAP.md`

**Step 1: Run the focused runtime-capability suite**

Run: `cargo test -p loongclaw-daemon runtime_capability_ --test integration -- --nocapture`
Expected: PASS

**Step 2: Run format check**

Run: `cargo fmt --all -- --check`
Expected: PASS

**Step 3: Run strict clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS

**Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS

**Step 5: Run all-features workspace tests**

Run: `cargo test --workspace --all-features`
Expected: PASS

### Task 9: Commit and publish the slice

**Files:**
- Modify only the runtime-capability delta-evidence files from this plan

**Step 1: Inspect worktree isolation**

Run:

- `git status --short`
- `git diff --cached --name-only`
- `git diff --cached`

Expected: only the planned files are included

**Step 2: Commit**

```bash
git add crates/daemon/src/runtime_capability_cli.rs \
  crates/daemon/src/runtime_experiment_cli.rs \
  crates/daemon/tests/integration/runtime_capability_cli.rs \
  crates/daemon/tests/integration/cli_tests.rs \
  docs/product-specs/runtime-capability.md \
  docs/ROADMAP.md \
  docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-design.md \
  docs/plans/2026-03-19-runtime-capability-structured-delta-evidence-implementation-plan.md
git commit -m "feat: capture runtime capability delta evidence"
```

**Step 3: Push and open the stacked PR**

Base branch: `feat/runtime-capability-index-readiness-gate`

PR body must include:

- `Closes #346`
- summary of the new structured delta evidence layer
- exact validation commands
