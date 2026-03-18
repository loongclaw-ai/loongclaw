# Memory Local Fast Path Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Remove unnecessary request-construction and JSON encode/decode overhead from
local in-process memory calls by turning the existing `*_direct` helpers into
real typed fast paths.

## Problem

The recent SQLite runtime reuse slice removed connection bootstrap overhead, but
there is still avoidable protocol overhead on local hot paths:

- `append_turn_direct(...)` currently builds a `MemoryCoreRequest`
- it then calls `execute_memory_core_with_config(...)`
- the core dispatch reparses the operation string and payload object

`window_direct_with_options(...)` does the same thing in reverse:

- construct request payload
- route through core dispatch
- deserialize `payload.turns` back into typed Rust structs

This is the wrong abstraction boundary for local calls. Inside one process we
already have typed arguments, so pretending every local operation is an RPC adds
allocation, branching, and serde overhead without adding correctness.

## Constraints

- Preserve the public memory-core request API for kernel and adapter use.
- Preserve the behavior of `append_turn`, `window`, `clear_session`, and
  `read_context` when invoked through `execute_memory_core_with_config(...)`.
- Preserve current local helper signatures:
  - `append_turn_direct(...)`
  - `window_direct(...)`
  - `window_direct_with_options(...)`
- Keep tests and provider/conversation call sites behaviorally unchanged.
- Do not weaken validation or transaction semantics.

## Options Considered

### Option A: Keep local helpers as thin wrappers over core dispatch

Pros:

- no structural change
- one code path for everything

Cons:

- preserves the exact overhead we want to remove
- keeps local hot paths coupled to string-based protocol dispatch

### Option B: Extract typed internal helpers and make request handlers adapt into them

Pros:

- request-based API remains stable at the boundary
- local direct helpers stop paying protocol overhead
- correctness stays centralized in one typed implementation

Cons:

- small refactor inside `memory/sqlite.rs`
- requires tests that explicitly lock the no-dispatch guarantee

### Option C: Replace the memory core protocol with typed interfaces everywhere

Pros:

- maximal internal simplification

Cons:

- crosses kernel boundary contracts
- far too large for this optimization slice

## Decision

Implement Option B.

Create typed internal helpers for the actual SQLite operations. The request
handlers remain boundary adapters:

- parse payload once
- validate external inputs
- delegate to typed helpers
- serialize the outcome back into the existing protocol shape

Local direct helpers call the typed helpers directly and skip protocol dispatch
entirely.

## Architecture

### Boundary adapter layer

These functions stay request-shaped:

- `append_turn(request, config)`
- `load_window(request, config)`
- `clear_session(request, config)`

Responsibilities:

- parse and validate `MemoryCoreRequest`
- preserve existing outward payload format
- call typed internals

### Typed internal layer

Introduce or extract typed helpers for:

- append turn
- load recent window with explicit options

These helpers operate only on typed Rust values and return typed Rust results.

### Direct local layer

`append_turn_direct(...)` and `window_direct_with_options(...)` should call the
typed internal helpers directly. This keeps low-code local workflows on the
fastest path while leaving kernel/core integrations untouched.

## Error Handling

- Request handlers keep the current validation error strings for malformed
  payloads.
- Typed helpers assume already-validated input and return storage/runtime
  failures only.
- No fallback to protocol dispatch is allowed on local direct paths.

## Why This Design

This is the right follow-up after SQLite runtime reuse:

- previous slice removed DB bootstrap waste
- this slice removes in-process protocol waste
- together they make local memory operations behave like true local functions
  rather than emulated RPC calls

For low-code runtime quality this matters because orchestration-heavy sessions
can generate a large number of local memory mutations and history reads. Each
avoidable JSON/request round-trip compounds tail latency and allocator churn.

## Verification

- Add a failing test proving `append_turn_direct(...)` does not invoke memory
  core dispatch.
- Add a failing test proving `window_direct(...)` does not invoke memory core
  dispatch.
- Keep existing memory semantics tests green.
- Run targeted memory tests, provider tests, full workspace tests, and
  architecture boundary checks.
