# Memory SQLite Runtime Path Canonicalization Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Ensure the process-local SQLite runtime registry treats different path spellings
that point to the same database file as one logical runtime.

## Problem

The current SQLite runtime reuse work caches runtimes by raw `PathBuf`. That is
good enough only when every caller passes the exact same path spelling. In
practice low-code and orchestration-heavy flows can reach the same database via:

- relative vs absolute paths
- paths containing `.` or `..`
- paths resolved from different current working directories
- paths whose parent directory is symlinked or canonicalized differently

When that happens, the current registry can materialize multiple runtimes for
one SQLite file. That weakens the previous optimization slices by duplicating:

- long-lived SQLite connections
- WAL handles and busy timeouts
- process-local memory overhead
- runtime bootstrap work

## Constraints

- Preserve the public memory API and existing call sites.
- Keep the SQLite database path returned in outward payloads stable and usable.
- Preserve current transaction semantics and runtime reuse behavior for already
  normalized paths.
- Support databases that do not exist yet.
- Avoid failing just because the target DB file is not created yet.

## Options Considered

### Option A: Keep raw `PathBuf` keys

Pros:

- zero change

Cons:

- same-file aliasing remains
- duplicated runtimes waste the exact resources we just optimized

### Option B: Normalize to absolute path strings only

Pros:

- simple
- removes relative/absolute duplication

Cons:

- does not collapse `..` or symlink aliasing reliably
- still allows parent-directory drift

### Option C: Canonicalize runtime paths with best-effort fallback

Pros:

- collapses the major same-file alias classes
- works for both existing and not-yet-created databases
- keeps runtime identity stable and deterministic

Cons:

- slightly more path-resolution logic
- needs careful test coverage for missing-file and parent-directory cases

## Decision

Implement Option C.

Add one internal path-normalization function that resolves the runtime key as
follows:

1. if the DB file exists, canonicalize the file path directly
2. otherwise:
   - absolutize the path
   - canonicalize the parent directory when possible
   - join the original file name back onto that canonical parent
3. if canonicalization is unavailable, fall back to the absolutized path

Use this normalized path everywhere runtime identity matters.

## Architecture

### Runtime identity

The normalized path becomes the sole key for:

- the runtime registry
- test bootstrap counters
- test-only runtime drop/reset helpers

This means runtime identity is no longer coupled to caller-specific path
spelling.

### Startup and hot path

Both `ensure_memory_db_ready(...)` and normal memory operations should route
through the same normalized path resolver before runtime lookup. That keeps
startup warmup and hot-path runtime reuse aligned.

### Outward path reporting

The runtime should store and expose the normalized path it actually uses. This
is the most honest and stable path for diagnostics because it matches the cache
identity and the real file location.

## Error Handling

- Failure to canonicalize a path is not fatal by itself.
- Missing DB files must still succeed as long as parent resolution can fall back
  to an absolute path.
- Parent-directory canonicalization should be best-effort, not a hard failure.

## Why This Design

This is the necessary follow-up to runtime reuse. Without stable runtime
identity, the cache is only partially effective and low-code workloads can
accidentally fan out multiple runtimes for one database depending on how config
or working-directory state is assembled.

Canonicalization is therefore not cosmetic; it is part of making the runtime
cache operationally correct.

## Verification

- Add a failing test proving an absolute path and an equivalent relative path
  reuse one runtime.
- Add a failing test proving a path containing `..` and the normalized path
  reuse one runtime.
- Keep existing runtime reuse, summary checkpoint, and direct fast-path tests
  green.
- Run targeted memory tests, provider tests, full workspace tests, and
  architecture boundary checks.
