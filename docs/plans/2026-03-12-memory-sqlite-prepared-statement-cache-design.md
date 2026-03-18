# Memory SQLite Prepared Statement Cache Design

## Context

`crates/app/src/memory/sqlite.rs` now keeps one long-lived `rusqlite::Connection` per normalized database path. That removed repeated open/bootstrap overhead, but the hot SQL paths still call `prepare(...)` or `execute(...)` on every operation. In practice that means the same SQL text is still parsed and compiled into SQLite VM bytecode repeatedly even though the connection itself is already being reused.

This is now the next obvious bottleneck in the local memory hot path:

- `window_direct*` repeatedly prepares the active-window query.
- summary rebuild/catch-up repeatedly prepares the same checkpoint queries.
- append/clear flows still recompile stable write statements inside transactions.

For low-code runtime behavior, this matters because the memory layer is supposed to behave like an always-on local primitive. Repeated prepare/finalize cycles add latency variance, extra allocator churn, and unnecessary CPU work on the same connection.

## Goal

Reuse prepared SQLite statements on the existing per-path long-lived connection so repeated memory reads/writes avoid redundant SQL compilation while keeping the external memory API and transaction semantics unchanged.

## Options Considered

### Option 1: Switch hot SQL to `prepare_cached` on the existing connection

Use `rusqlite::Connection::prepare_cached` for stable SQL in the hot path, set an explicit statement-cache capacity during connection setup, and keep the existing `SqliteRuntime` ownership model unchanged.

Pros:

- Very small surface-area change.
- Piggybacks on the long-lived connection already introduced.
- Keeps the code low-complexity and aligned with `rusqlite`'s intended cache model.
- Easy to validate with test-only inspection of prepared statement residency on the connection handle.

Cons:

- Less explicit control than a fully manual statement registry.
- Cache is still connection-global rather than operation-specific.

### Option 2: Build a manual statement registry inside `SqliteRuntime`

Store preprepared statements or statement slots in a dedicated runtime-owned structure.

Pros:

- Maximum control over statement lifecycle.
- Could attach per-statement metrics later.

Cons:

- Considerably more borrow/lifetime complexity with `rusqlite`.
- Much larger maintenance burden for very little extra payoff right now.
- Higher risk of subtle invalidation bugs.

### Option 3: Skip statement caching and only tune SQL/indexes further

Leave `prepare(...)` behavior as-is and optimize around query shapes or indexes only.

Pros:

- Lowest implementation risk.

Cons:

- Leaves obvious repeated SQL compilation cost in place.
- Misses the natural optimization unlocked by runtime reuse.

## Recommended Design

Use Option 1.

Specifically:

- Set an explicit prepared statement cache capacity on every newly opened SQLite connection.
- Replace stable `prepare(...)` / hot `execute(...)` paths with cached statement preparation where it is safe and local:
  - recent window query
  - summary rebuild query
  - summary catch-up query
  - summary checkpoint load/upsert/delete
  - append-turn insert
  - clear-session delete
- Keep transaction boundaries exactly as they are now.
- Keep all public APIs unchanged.

## Verification Strategy

This optimization needs an observable regression test instead of a "trust me" refactor.

Test-only helpers will inspect the SQLite connection handle and count prepared statements resident on the long-lived connection after operations complete. That lets us prove the hot path leaves reusable prepared statements behind once caching is enabled.

Planned regression coverage:

- a window read warms and retains a cached prepared statement on the runtime connection
- a summary-enabled append path retains multiple cached statements after overflow/materialization

## Risk Notes

- Cached statements increase steady-state connection memory slightly, so cache capacity must stay intentionally small.
- Cached statements must not change transaction semantics or leave statements busy after operations complete.
- Tests should assert behavior in the scoped temp DB under the runtime lock to avoid interference from parallel tests.
