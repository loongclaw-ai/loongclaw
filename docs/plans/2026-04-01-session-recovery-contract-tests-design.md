# Session Recovery Contract Tests Design

**Date:** 2026-04-01

**Goal:** Add direct unit coverage for `crates/app/src/session/recovery.rs` so recovery payload construction, fallback synthesis, and JSON projection cannot drift behind broader coordinator and session-tool behavior.

## Problem

`session::recovery` currently owns the lowest-level recovery contract for:
- async spawn failure recovery payloads
- missing-recovery observation rules
- `last_error` to recovery-kind synthesis
- JSON projection returned to higher layers

Those semantics are exercised today only through higher-level coordinator and session-tool tests. That coverage is valuable, but it is indirect. A future refactor could change helper behavior while still keeping most end-to-end tests green because the surrounding orchestration remains intact.

## Proposed Scope

This iteration adds focused unit tests under `crates/app/src/session/recovery.rs`.

It will verify:
- `build_async_spawn_failure_recovery_payload(...)` preserves the expected shape
- `observe_missing_recovery(...)` prefers the newest recovery event over `last_error`
- `observe_missing_recovery(...)` falls back to `last_error` synthesis when no recovery event exists
- `recovery_kind_from_last_error(...)` maps each known prefix to the correct recovery kind
- `recovery_json(...)` emits `null` for empty event kinds and zero timestamps

## Non-Goals

This iteration will not:
- extract a new read-side helper for delegate child boundary reconstruction
- change `conversation/runtime.rs`
- change `conversation/turn_coordinator.rs`
- change operator runtime ownership boundaries
- redesign recovery payload fields

## Why This Cut

I considered two alternatives:

1. Extract the delegate read-side helper first.

That would move more architecture forward, but it would touch more behavior at once and would still leave the recovery helper contract under-tested.

2. Combine read-side extraction and recovery tests in one pass.

That would be faster in raw throughput, but it would produce a wider review surface and blur whether failures come from helper drift or ownership refactoring.

The chosen cut is smaller and cleaner. It strengthens the test fence first, which lowers the risk of the next ownership extraction.

## Validation Plan

The change should be validated with:
- targeted `session::recovery` unit tests
- `cargo test -p loongclaw-app --lib session::recovery`
- `cargo test --workspace --locked`
- `cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings`

## Expected Outcome

After this lands, recovery contract drift should fail fast in a local unit test instead of surfacing later through coordinator or session-tool regressions.
