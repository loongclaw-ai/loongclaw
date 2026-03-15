# Provider Hot Path Cache And Decomposition Design

## Background

`crates/app/src/provider/mod.rs` previously mixed four independent concerns into one file:

1. prompt/message assembly for provider requests
2. HTTP client and request context setup
3. completion / turn request retry state machines
4. provider module regression tests

At the same time, automatic model selection still performed a real remote model-list fetch for each request whenever `provider.model = "auto"` and the selected provider had no baked-in default model. That made the request path pay repeated network latency and extra retries even when the provider catalog had not changed.

## Problem Statement

The hot path had two issues:

1. structural coupling: provider request logic was concentrated in a single oversized boundary file, making future optimization work risky and slow
2. repeated remote discovery: auto-model mode re-fetched `/models` on every request, amplifying latency and network variance

This combination is especially expensive in low-code/operator-first runtime flows where the same provider config is reused across many turns.

## Goals

- reduce steady-state auto-model latency by reusing recent model-list results in-process
- keep cache memory bounded and authorization-aware
- split provider request hot-path logic into focused modules without changing external behavior
- preserve current provider fallback semantics, payload adaptation, and retry policy

## Non-Goals

- no cross-process or persistent cache
- no speculative prefetch
- no config-surface expansion for cache tuning in this slice
- no semantic change to provider request/response handling

## Chosen Approach

### 1. Bounded in-process model-list cache

Add a provider-internal cache keyed by:

- resolved models endpoint
- normalized request header fingerprint
- resolved authorization header fingerprint input

Cache characteristics:

- TTL: 60 seconds
- max entries: 32
- eviction: expired-first, then oldest last-accessed entry
- success-only writes; failures are never cached

This provides immediate latency reduction for repeated auto-model requests while keeping memory growth bounded and avoiding cache bleed across credentials.

### 2. Provider module decomposition

Split hot-path responsibilities out of `provider/mod.rs` into:

- `provider/messages.rs`
  - provider-facing prompt/message assembly
  - memory window projection into provider message format
- `provider/request_context.rs`
  - request client construction
  - lightweight request context and error carrier
- `provider/completion.rs`
  - completion request retry/adaptation loop
- `provider/turn.rs`
  - turn request retry/adaptation loop
- `provider/model_cache.rs`
  - model-list cache state and keying logic

`provider/mod.rs` remains the orchestration surface that preserves the existing public API.

## Why This Design

Alternative A was “only split files until the architecture budget passes.” That would improve maintainability, but it would leave the real latency leak untouched.

Alternative B was “only add model-list caching.” That would improve request latency, but it would further entrench hot-path complexity inside a single overgrown module.

The chosen design solves both:

- measurable runtime waste is removed from auto-model mode
- future provider work has smaller, clearer edit surfaces

## Risk Analysis

### Cache staleness

Risk: provider model catalogs may change during the TTL window.

Mitigation:

- use a short TTL of 60 seconds
- cache only model-list discovery, not actual completions
- explicit `provider.model` still bypasses discovery completely

### Credential bleed

Risk: different tokens against the same endpoint could see different catalogs.

Mitigation:

- include authorization input in the cache key fingerprint
- add regression coverage for credential-separated caches

### Memory growth

Risk: long-lived processes accumulate provider cache entries.

Mitigation:

- fixed 32-entry cap
- expired-entry pruning
- oldest-entry eviction

## Validation Strategy

- red/green test for “second fetch succeeds after server is gone”, proving real cache reuse
- regression test for authorization-scoped cache separation
- provider test suite
- workspace-wide test suite
- architecture boundary script

## Expected Outcome

- lower repeated auto-model discovery latency inside the same process
- fewer redundant network retries on steady-state request loops
- `provider/mod.rs` back under architecture budget
- clearer next-step landing zone for future work such as in-flight request coalescing or provider capability caches
