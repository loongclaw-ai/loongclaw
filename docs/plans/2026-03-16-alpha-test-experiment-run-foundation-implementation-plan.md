# Alpha-Test Experiment-Run Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a minimal `runtime-experiment` record layer that lets operators create, finish, and inspect snapshot-based experiment-run artifacts on `alpha-test`.

**Architecture:** Reuse the snapshot lineage artifact contract introduced by issue `#208` / PR `#211`, add one new daemon CLI module for experiment-run artifact lifecycle, keep the schema file-based and record-oriented, and avoid any live runtime mutation or command execution in this slice.

**Tech Stack:** Rust, Clap, serde, existing LoongClaw daemon integration-test harness, Markdown docs.

---

### Task 1: Rebase onto the snapshot-lineage base and land the planning artifacts

**Files:**
- Create: `docs/plans/2026-03-16-alpha-test-experiment-run-foundation-design.md`
- Create: `docs/plans/2026-03-16-alpha-test-experiment-run-foundation-implementation-plan.md`
- Modify: `docs/ROADMAP.md`

**Step 1: Verify the implementation base exists**

Run: `rg -n "RuntimeRestore|RuntimeSnapshotArtifactDocument|parent_snapshot_id" crates/daemon/src/lib.rs crates/daemon/src/runtime_restore_cli.rs`

Expected: matches from the snapshot-lineage slice are present. If not, rebase after issue `#208` / PR `#211` lands.

**Step 2: Verify the planning artifacts exist**

Run: `test -f docs/plans/2026-03-16-alpha-test-experiment-run-foundation-design.md && test -f docs/plans/2026-03-16-alpha-test-experiment-run-foundation-implementation-plan.md`

Expected: success

### Task 2: Add failing CLI parsing coverage for `runtime-experiment`

**Files:**
- Modify: `crates/daemon/src/lib.rs`
- Modify: `crates/daemon/tests/integration/cli_tests.rs`

**Step 1: Write the failing tests**

Add parsing tests that prove:

- `runtime-experiment start` accepts `--snapshot`, `--output`,
  `--mutation-summary`, `--experiment-id`, `--label`, `--tag`, and `--json`
- `runtime-experiment finish` accepts `--run`, `--result-snapshot`,
  `--evaluation-summary`, repeated `--metric`, repeated `--warning`,
  `--decision`, `--status`, and `--json`
- `runtime-experiment show` accepts `--run` and `--json`

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_cli_parses --manifest-path Cargo.toml`

Expected: FAIL because the new command family does not exist yet.

**Step 3: Write minimal implementation**

- add a `RuntimeExperiment` command family to the daemon CLI
- add typed option structs for `start`, `finish`, and `show`
- keep command naming aligned with `runtime-snapshot` and `runtime-restore`

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_cli_parses --manifest-path Cargo.toml`

Expected: PASS

### Task 3: Add failing artifact-schema and start-flow tests

**Files:**
- Create: `crates/daemon/src/runtime_experiment_cli.rs`
- Modify: `crates/daemon/tests/integration/mod.rs`
- Create: `crates/daemon/tests/integration/runtime_experiment_cli.rs`

**Step 1: Write the failing tests**

Add integration tests that prove:

- `start` creates a new experiment-run artifact from a baseline snapshot file
- the run record inherits `experiment_id` from the baseline snapshot when present
- `start` requires explicit `--experiment-id` when the baseline snapshot has no
  experiment id
- the run artifact starts with status `planned` and decision `undecided`
- the run artifact includes baseline snapshot lineage summary and mutation
  summary

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_start --manifest-path Cargo.toml`

Expected: FAIL because the experiment-run executor and schema do not exist yet.

**Step 3: Write minimal implementation**

- define the experiment-run artifact structs and schema version constants
- implement `start`:
  - load baseline snapshot artifact
  - resolve or require `experiment_id`
  - compute deterministic `run_id`
  - persist the run artifact to the requested output path
- add text and JSON rendering helpers

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_start --manifest-path Cargo.toml`

Expected: PASS

### Task 4: Add failing finish-flow and integrity tests

**Files:**
- Modify: `crates/daemon/src/runtime_experiment_cli.rs`
- Modify: `crates/daemon/tests/integration/runtime_experiment_cli.rs`

**Step 1: Write the failing tests**

Add integration tests that prove:

- `finish` attaches result snapshot lineage and persists evaluation summary
- `finish` parses repeated numeric `--metric key=value` pairs into a flat metric
  map
- `finish` records warnings passed on the command line
- `finish` rejects result snapshots whose explicit `experiment_id` conflicts
  with the run record
- `finish` records a warning when the result snapshot has no `experiment_id`
- `finish` rejects attempts to mutate a run already marked `completed` or
  `aborted`

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_finish --manifest-path Cargo.toml`

Expected: FAIL because the finish path and integrity gates do not exist yet.

**Step 3: Write minimal implementation**

- implement `finish` artifact loading and validation
- parse and validate `metric=value` inputs
- add result snapshot summary extraction
- enforce experiment-id consistency rules
- update run status, decision, evaluation, and warnings in one atomic rewrite

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_finish --manifest-path Cargo.toml`

Expected: PASS

### Task 5: Add failing show-rendering tests

**Files:**
- Modify: `crates/daemon/src/runtime_experiment_cli.rs`
- Modify: `crates/daemon/tests/integration/runtime_experiment_cli.rs`

**Step 1: Write the failing tests**

Add tests that prove:

- `show --json` round-trips the persisted artifact cleanly
- text rendering surfaces the decision-critical fields first:
  - run id
  - experiment id
  - baseline snapshot id
  - result snapshot id
  - status
  - decision
  - metrics
  - warnings

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_show --manifest-path Cargo.toml`

Expected: FAIL because the show renderer does not exist yet.

**Step 3: Write minimal implementation**

- implement `show` to load and render the artifact
- keep text output compact and operator-oriented
- avoid adding a separate viewer abstraction in the first slice

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment_show --manifest-path Cargo.toml`

Expected: PASS

### Task 6: Document the new experiment-management slice

**Files:**
- Modify: `docs/ROADMAP.md`
- Modify: `docs/product-specs/index.md`
- Create if needed: `docs/product-specs/runtime-experiment.md`

**Step 1: Update docs after behavior is green**

- describe `runtime-experiment` as the record layer above snapshot/restore
- keep the boundaries explicit: record-oriented, no shell execution, no
  automatic skill mutation, no automatic promotion
- position it as the prerequisite for later evaluator and optimizer work

**Step 2: Verify docs reference the new slice**

Run: `rg -n "runtime-experiment|experiment-run|snapshot-based experiment" docs`

Expected: matches for the new product surface and roadmap notes

### Task 7: Full verification and delivery

**Files:**
- Modify only what the previous tasks require

**Step 1: Run focused verification**

Run: `cargo test -p loongclaw-daemon --test integration runtime_experiment --manifest-path Cargo.toml`

Expected: PASS

**Step 2: Run broader verification**

Run: `cargo fmt --all --manifest-path Cargo.toml`
Run: `cargo clippy --workspace --all-targets --all-features --manifest-path Cargo.toml -- -D warnings`
Run: `cargo test -p loongclaw-daemon --test integration --manifest-path Cargo.toml`
Run: `cargo test --workspace --all-features --manifest-path Cargo.toml`

Expected: PASS

**Step 3: Commit**

```bash
git add docs crates/daemon
git commit -m "feat(daemon): add runtime experiment run records"
```
