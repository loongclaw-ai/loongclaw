# Provider Model Selection Client Reuse Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Reduce avoidable provider request overhead in auto-model mode by reusing the
same HTTP client across model discovery and the subsequent completion/turn
request path.

## Problem

Today `request_completion(...)` and `request_turn(...)` build a `reqwest::Client`
before resolving candidate models, but `resolve_request_models(...)` delegates to
`fetch_available_models_with_policy(...)`, which builds another client for the
same provider request policy.

That means one logical provider request can allocate two clients before the real
model call is even sent.

## Constraints

- keep public provider behavior unchanged
- keep auto-model ranking and retry semantics unchanged
- avoid introducing global caches in this slice
- keep the patch reviewable and local to provider internals

## Options Considered

### Option A: Reuse the already-built client

Thread an existing `reqwest::Client` through model-selection helpers.

Pros:

- smallest safe change
- no cache invalidation or lifecycle complexity
- immediately removes duplicate client construction

Cons:

- does not reduce repeated remote model-list fetches across separate requests

### Option B: Add a process-local model-list cache

Cache the discovered model list keyed by provider endpoint/config.

Pros:

- larger steady-state latency savings

Cons:

- requires cache key, TTL, and invalidation policy
- larger correctness surface than needed for this slice

## Decision

Implement Option A now.

This slice will:

- add client-aware model-selection helpers
- reuse the same client in `request_completion(...)`, `request_turn(...)`, and
  explicit `fetch_available_models(...)`
- keep ranking, retry, and fallback behavior unchanged

This slice will not yet:

- add model-list TTL caches
- change provider request policies
- refactor provider module boundaries

## Verification

- add failing tests for the new client-aware helper boundary
- run targeted provider tests
- run `cargo test --workspace --all-features`
- run `./scripts/check_architecture_boundaries.sh`
