# Test Convention Overhaul Design

**Issue**: https://github.com/loongclaw-ai/loongclaw/issues/139
**Date**: 2026-03-15
**Scope**: All 7 workspace crates
**Delivery**: Single monolithic PR

## Convention

Every crate follows hybrid test organization:

- **Inline** `#[cfg(test)] mod tests {}` — pure unit tests that only need module-private access. Stay in `src/`.
- **External** `crates/<name>/tests/` — integration tests that use real IO, real kernel, full harness setup, or cross-module wiring. Compiled as separate binaries. Access only `pub` API + `test-support` feature exports.

**Classification heuristic**: If a test creates a temp directory, spawns a real kernel, hits the filesystem, or composes multiple subsystems — it's an integration test and belongs in `tests/`.

## Visibility Strategy: `test-support` Feature

Each crate that needs to expose internals for integration tests adds to its `Cargo.toml`:

```toml
[features]
test-support = []
```

And in `lib.rs`:

```rust
#[cfg(feature = "test-support")]
pub mod test_support;
```

The `test_support` module re-exports only what integration tests need. External tests enable it via:

```toml
[dev-dependencies]
loongclaw-app = { path = ".", features = ["test-support"] }
```

This follows the existing `crates/spec` precedent (`test-hooks` feature).

## Per-Crate Migration

### contracts (leaf crate)
- **Current**: Inline `#[cfg(test)]` blocks only
- **Action**: Audit for integration tests. Likely no external `tests/` needed — unit-only crate.

### protocol (leaf crate)
- **Current**: Inline `#[cfg(test)]` blocks only
- **Action**: Migrated — integration tests moved to `crates/protocol/tests/`, `test-support` feature added.

### kernel
- **Current**: `src/tests.rs` (2,046 lines), mock harnesses (`MockEmbeddedPiHarness`, `MockAcpHarness`)
- **Action**: Extract integration tests to `crates/kernel/tests/`. Keep unit tests inline. Add `test-support` feature to expose mock harnesses.

### spec
- **Current**: Inline tests, already has `test-hooks` feature
- **Action**: Extract integration tests to `crates/spec/tests/`. Extend `test-hooks` or add `test-support`.

### app (largest migration)
- **Current**:
  - `conversation/integration_tests.rs` (512 lines, 14 tests, `TurnTestHarness` + `FakeProviderBuilder`)
  - `conversation/tests.rs` (12,792 lines, 8 imports of `TurnTestHarness`)
  - `provider/tests.rs` (2,872 lines)
  - `memory/tests.rs` (277 lines)
  - `channel/feishu/payload/tests.rs`
- **Action**:
  1. Create `src/test_support.rs` gated by `feature = "test-support"`
  2. Move `TurnTestHarness` + `FakeProviderBuilder` into `test_support`
  3. Re-export `MvpToolAdapter`, `ToolPolicyExtension`, `FilePolicyExtension`, `MemoryRuntimeConfig` through `test_support`
  4. Move 9 integration tests from `integration_tests.rs` to `crates/app/tests/conversation_integration.rs`
  5. Update 8 import sites in `conversation/tests.rs` (from `super::integration_tests::` to `loongclaw_app::test_support::`)
  6. Audit `provider/tests.rs` and `memory/tests.rs` for integration-style tests and move those
  7. Remove `#[cfg(test)] mod integration_tests` from `conversation/mod.rs`

### daemon
- **Current**: `src/tests/` (15 files, `mod.rs` orchestrator)
- **Action**: Move entire `src/tests/` to `crates/daemon/tests/`. Update imports. Add `test-support` feature if shared helpers need re-export.

### bench
- **Current**: Benchmarking suite with integration-style tests
- **Action**: Migrated — integration tests moved to `crates/bench/tests/`, `test-support` feature added.

## Target Structure (app crate example)

```text
crates/app/
├── src/
│   ├── conversation/
│   │   ├── mod.rs                     # remove #[cfg(test)] mod integration_tests
│   │   └── tests.rs                   # unit tests stay, imports update
│   ├── test_support.rs                # NEW: gated by feature = "test-support"
│   └── lib.rs                         # add: #[cfg(feature = "test-support")] pub mod test_support;
├── tests/
│   ├── conversation_integration.rs    # 9 integration tests
│   ├── provider_integration.rs        # integration tests (if any)
│   └── memory_integration.rs          # integration tests (if any)
└── Cargo.toml                         # add test-support feature + dev-dep
```

## Error Handling

- `test_support` modules only re-export — no new logic
- If a test straddles the unit/integration boundary, default to keeping it inline (conservative)
- If promoting a type to `test_support` would require cascading visibility changes through 3+ layers, flag it as a blocker rather than force it

## Verification

- `cargo test --workspace` must pass after migration
- `cargo test --workspace --no-default-features` must pass (no test-support leak)
- No `pub(crate)` test helpers should remain orphaned after migration
- Each `tests/` file should compile independently
