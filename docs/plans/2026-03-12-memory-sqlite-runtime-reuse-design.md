# Memory SQLite Runtime Reuse Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Remove the remaining SQLite system-overhead hot path from the memory subsystem
by reusing a process-local runtime per resolved database path instead of opening
and initializing a fresh SQLite connection for every read and write.

## Problem

The previous memory slices removed repeated summary rebuilding and duplicate
prompt-context hydration work, but the SQLite backend still pays avoidable
runtime cost on every operation:

- `append_turn`, `load_window`, `load_context_snapshot`, and `clear_session`
  all call `open_memory_connection(...)`
- `open_memory_connection(...)` still performs `Connection::open(...)`
- schema creation is re-executed on every call
- connection PRAGMA state is not preserved across operations

Under low-code or operator-heavy sessions this means the system still burns CPU,
syscalls, file-handle churn, and allocator work even when the logical memory
operation is small.

## Constraints

- Preserve the public memory API and all current call sites.
- Keep `turns` and `memory_summary_checkpoints` semantics unchanged.
- Preserve transactional atomicity for:
  - `append_turn` turn insert + checkpoint maintenance
  - `clear_session` source delete + checkpoint delete
- Keep `window_only`, `profile_plus_window`, and `window_plus_summary`
  outward behavior unchanged.
- Support multiple SQLite paths inside one process.
- Keep recovery simple: the runtime must be replaceable if the cached entry is
  explicitly reset or becomes unusable.

## Options Considered

### Option A: Keep per-call connections and only trim helper overhead

Pros:

- smallest code change
- no global runtime state

Cons:

- does not remove `Connection::open(...)` from the hot path
- repeats schema setup and connection bootstrapping forever
- leaves the main remaining SQLite overhead untouched

### Option B: Per-path background worker thread with request channel

Pros:

- strong serialization guarantee per database path
- clear ownership of a long-lived connection

Cons:

- materially larger implementation surface
- adds channel hops and thread lifecycle complexity
- failure and shutdown handling are more expensive to verify

### Option C: Per-path runtime registry with serialized `Connection` access

Pros:

- removes repeated open/init work while keeping the current call graph intact
- gives one-time schema init and one-time PRAGMA setup per database path
- preserves transaction semantics by executing each operation while holding the
  runtime's connection lock
- simpler than a worker thread and lower overhead on the hot path

Cons:

- per-path operations are serialized inside one process
- requires a small amount of process-global runtime state

## Decision

Implement Option C now.

Create a process-local SQLite runtime manager keyed by resolved database path.
Each runtime owns one long-lived `rusqlite::Connection` protected by a mutex.
Callers keep the same public entry points, but internal memory operations route
through the cached runtime instead of creating a new connection every time.

The earlier worker-thread concept is intentionally not chosen for this slice.
The optimization target is repeated connection/bootstrap overhead, not
cross-process scheduling. A serialized in-process connection holder achieves the
same correctness guarantees with less indirection and lower steady-state cost.

## Architecture

### Runtime registry

Add a process-local registry:

- key: resolved SQLite `PathBuf`
- value: shared runtime handle for that path

Responsibilities:

- reuse an existing runtime when the same path is requested again
- lazily create a runtime on first use
- support test-only reset hooks so recreation behavior can be verified

### Runtime object

Each runtime owns:

- the resolved database path
- a single long-lived `rusqlite::Connection`
- serialized execution helpers for read and write closures

This makes every per-path memory operation deterministic and keeps the SQLite
handle hot inside the process.

### One-time connection bootstrapping

Runtime creation performs the expensive setup exactly once:

- create parent directories if needed
- open the SQLite connection
- apply connection PRAGMAs:
  - `journal_mode = WAL`
  - `synchronous = NORMAL`
  - `foreign_keys = ON`
  - `busy_timeout`
- execute schema initialization for:
  - `turns`
  - `memory_summary_checkpoints`

After creation, normal reads and writes run against the cached connection only.

## Data Flow

### Read paths

`load_window(...)` and `load_context_snapshot(...)`:

1. resolve the SQLite path
2. acquire the cached runtime for that path
3. execute the read closure against the runtime connection
4. return the same payload shape as today

### Write paths

`append_turn(...)` and `clear_session(...)`:

1. resolve the SQLite path
2. acquire the cached runtime for that path
3. run the full transaction while holding the runtime's connection lock
4. commit or fail atomically

This preserves the current guarantee that source-turn writes and derived summary
checkpoint updates cannot diverge.

### Startup path

`ensure_memory_db_ready(...)` should route through the same runtime acquisition
path. This keeps startup warmup and hot-path execution aligned and avoids
booting a second redundant connection.

## Error Handling And Recovery

- Runtime creation failures return the current operation error immediately.
- Operation failures do not silently fall back to opening an uncached
  connection.
- The registry must support dropping a cached runtime so tests can prove a
  fresh runtime is created afterward.
- If a runtime becomes unusable in the future, the same reset path can be used
  to recreate it without changing external APIs.

## Why This Design

This is the highest-value next slice after materialized summary checkpoints:

- it attacks the remaining repeated SQLite system cost directly
- it keeps the public API stable for low-code consumers
- it improves latency, allocation pressure, and file-handle churn together
- it creates a clean foundation for any later prepared-statement or batching
  optimization

For low-code maturity this is important because prompt hydration is not just a
feature path, it is infrastructure. The backend should behave like a warmed
runtime service, not like a cold-start database bootstrap on every turn.

## Verification

- Add a failing test proving repeated operations against one path reuse a single
  runtime and perform one connection bootstrap.
- Add a failing test proving different SQLite paths create distinct runtimes.
- Add a failing test proving resetting a cached runtime causes the next access
  to recreate it.
- Keep existing behavior tests green for:
  - `window_only`
  - `profile_plus_window`
  - `window_plus_summary`
- Run targeted memory tests, provider regression tests, full workspace tests,
  and architecture boundary checks.
