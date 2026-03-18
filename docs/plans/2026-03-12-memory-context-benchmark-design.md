# Memory Context Benchmark Design

## Context

The recent SQLite memory work has materially changed the hot path for
`load_context_snapshot` and `load_prompt_context`:

- SQLite runtime reuse is now path-canonicalized and cached
- prepared statement preparation is cached
- summary materialization is streamed instead of rebuilding buffered
  `Vec<IndexedConversationTurn>` collections
- summary rows now short-circuit after budget saturation
- summary line construction now writes normalized content directly into the
  final buffer without a scratch normalization string

These changes should improve both first materialization and steady-state prompt
context hydration, but the repository does not yet have a dedicated benchmark
artifact that quantifies those gains or lets future changes guard against
regressions.

## Goals

- Add a repository-native benchmark for memory context hydration
- Quantify the difference between summary rebuild and summary steady-state reads
- Reuse the repository's existing benchmark delivery pattern:
  `crates/bench` report generator + `crates/daemon` CLI exposure
- Produce machine-readable JSON output suitable for local comparison and future
  CI gating
- Keep the benchmark focused on the real public memory entrypoint used by the
  app: `loongclaw_app::memory::load_prompt_context(...)`

## Non-Goals

- No schema changes
- No change to memory API behavior
- No Criterion integration in this slice
- No broad benchmark matrix DSL like programmatic pressure in this slice
- No direct optimization of runtime behavior in the same change unless the
  benchmark implementation reveals a correctness issue

## Approaches Considered

### Option 1: Native benchmark CLI in existing bench stack

- Add `run_memory_context_benchmark_cli(...)` to `crates/bench`
- Expose it through a new `benchmark-memory-context` subcommand in
  `crates/daemon`
- Seed deterministic SQLite history, then report:
  - `summary_rebuild` latency
  - `summary_steady_state` latency
  - `window_only` latency
  - optional steady-state speedup gate

Pros:
- Matches existing repository patterns
- Produces JSON artifacts like other benchmarks
- Easy to run locally and later promote into CI

Cons:
- Requires adding `loongclaw-app` as a dependency of `crates/bench`

### Option 2: Criterion / `cargo bench`

- Add micro-benchmarks under a separate benchmark harness

Pros:
- Familiar benchmarking style
- Rich statistical output

Cons:
- Does not match current repository benchmark UX
- Harder to integrate with current JSON report and gate model
- Adds a second benchmarking system

### Option 3: One-off profiling script

- Create an ad-hoc binary or script to print timing numbers

Pros:
- Lowest immediate implementation cost

Cons:
- Not regression-friendly
- Weak artifact quality
- Low long-term value for repository health

## Decision

Choose Option 1.

The repository already has a benchmark delivery language: stable CLI entrypoints
that generate JSON reports and optionally enforce simple gates. Extending that
system to memory context hydration keeps the benchmark operationally useful and
avoids splitting performance evidence across incompatible tools.

## Design Details

### 1. Benchmark shape

The benchmark command will seed a deterministic SQLite database with a long
conversation history using the public direct memory APIs. It will then measure
three scenarios:

- `window_only`
  - prompt hydration using window-only mode for the same history
- `summary_rebuild`
  - prompt hydration on a database with turns but without a materialized
    summary checkpoint, forcing summary rebuild
- `summary_steady_state`
  - prompt hydration on the same history after the summary checkpoint has
    already been built

This makes the benchmark useful both for product-facing latency analysis and
for validating the specific summary materialization optimizations landed in the
recent refactor chain.

### 2. Public API path

The benchmark will call:

- `loongclaw_app::memory::append_turn_direct(...)`
- `loongclaw_app::memory::ensure_memory_db_ready(...)`
- `loongclaw_app::memory::load_prompt_context(...)`

This keeps the benchmark aligned with the real application path while still
avoiding unrelated provider/channel overhead.

### 3. Report format

The JSON report should include:

- generation timestamp
- profile string
- output path
- benchmark knobs:
  - history turns
  - sliding window
  - summary max chars
  - words per turn
  - iteration counts
- numeric stats for each scenario
- summary entry size and hydrated entry counts
- observed `summary_rebuild_p95 / summary_steady_state_p95` speedup ratio
- optional gate summary

### 4. Gate model

Use a simple direct threshold like the existing Wasm cache benchmark:

- `--enforce-gate`
- `--min-steady-state-speedup-ratio`

If enabled, the benchmark fails when the observed p95 speedup ratio from rebuild
to steady-state falls below the configured threshold.

This is intentionally simpler than a matrix+baseline DSL because the goal here
is to establish a clean performance artifact first, not to solve generalized
benchmark orchestration in one step.

### 5. Seed strategy

To isolate summary behavior:

- create the seed history in a window-only database so no summary checkpoint is
  pre-materialized
- for rebuild samples:
  - copy the seeded database to a fresh path
  - pre-bootstrap the runtime
  - time `load_prompt_context(...)`
- for steady-state samples:
  - copy the seeded database once
  - warm up by materializing the summary checkpoint
  - then time repeated `load_prompt_context(...)` calls on the same runtime and
    database

This keeps runtime bootstrap noise mostly out of the measured region while still
representing realistic hot and cold summary behavior.

## Validation Strategy

- Unit tests for benchmark parameter validation
- Unit/integration-style test that the benchmark writes a parseable JSON report
- CLI parse/dispatch test for the new daemon subcommand
- `cargo test -p loongclaw-bench`
- `cargo test -p loongclaw-daemon`
- full workspace test run
- one real benchmark execution producing a report under `target/benchmarks/`

## Expected Impact

- Converts recent memory optimizations from anecdotal improvements into
  measurable evidence
- Provides a regression anchor for future SQLite summary refactors
- Gives a cleaner basis for deciding whether the next optimization target should
  stay in memory, shift to provider prompt building, or move elsewhere
