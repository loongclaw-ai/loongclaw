# Provider Model Discovery In-Flight Coalescing Design

## Background

`crates/app/src/provider/model_selection.rs` already avoids repeated sequential `/models`
discovery by reusing the bounded process-local cache introduced in the previous optimization
slice. That removes steady-state refetches, but it does not eliminate duplicate remote requests
during concurrent cold starts.

When multiple turns hit provider auto-model mode at the same time and the cache is empty or
expired, each request currently performs its own remote model-list fetch. In bursty low-code and
operator-driven flows, that turns a single logical discovery event into N identical network calls,
N retry loops, and N body decodes.

## Problem Statement

The remaining hot-path waste is concurrent duplication, not sequential reuse:

1. multiple concurrent callers can still race through the cache-miss path
2. each caller allocates its own request/retry state for the same endpoint and credentials
3. remote latency spikes or provider throttling are amplified by fan-out on `/models`

This is especially visible when the runtime fans out multiple workflows against a shared provider
configuration.

## Goals

- collapse concurrent model-list cache misses into one remote fetch per cache identity
- preserve the existing bounded cache semantics and authorization-aware scoping
- broadcast the same success or failure result to all waiters
- keep the implementation localized to provider model discovery internals

## Non-Goals

- no persistent or cross-process coordination
- no speculative provider prefetching
- no config-surface changes for coalescing tunables in this slice
- no changes to provider completion or turn fallback semantics

## Approaches Considered

### A. Shared in-flight entry with waiter fan-out

Maintain a short-lived in-flight map keyed by the same cache identity as the model cache. The
first caller becomes the leader and performs the real fetch. Followers subscribe to the same
result and do not issue their own `/models` request.

Pros:

- removes duplicate concurrent fetches entirely
- composes directly with the existing cache key and cache invalidation logic
- keeps the optimization local to model discovery

Cons:

- requires careful cleanup on success, failure, and dropped leaders

### B. Watch/notify state machine

Store an explicit fetch state plus wake-up primitive for each key and have followers poll or await
state transitions.

Pros:

- highly explicit state model

Cons:

- more code and more cancellation edge cases
- no practical advantage over one-shot waiter fan-out for this narrow use case

### C. TTL tuning or background warming only

Rely on shorter refresh windows or prefetching to reduce misses.

Pros:

- smallest code change

Cons:

- does not solve concurrent miss storms
- still duplicates remote work whenever the cache is cold

## Chosen Approach

Choose Approach A: add an in-flight model discovery registry beside the existing bounded cache.

Implementation shape:

- reuse the existing `ModelListCacheKey` identity: endpoint + normalized header fingerprint +
  authorization fingerprint input
- add a short-lived in-flight map from key to waiter list
- leader performs the actual remote fetch
- followers await a one-shot result instead of sending their own request
- success writes the normal cache before releasing waiters
- failure is broadcast unchanged and does not populate the cache
- leader drop/cancellation removes the in-flight entry and releases waiters with a synthetic error

## Why This Design

This is the smallest change that removes the real remaining inefficiency. The current cache already
optimizes sequential reuse well enough. The next leverage point is concurrent fan-out, and that
requires sharing an in-flight result, not more TTL tuning.

Using the existing cache key preserves the same isolation properties:

- no credential bleed across tokens
- no endpoint bleed across providers
- no unbounded memory growth because in-flight entries live only for the duration of one fetch

## Risk Analysis

### Leader cancellation

Risk: the leading fetch task drops before publishing a result, leaving followers blocked.

Mitigation:

- hold leader ownership in a guard
- on guard drop without completion, remove the in-flight entry and broadcast a synthetic error to
  all waiters

### Duplicate success publication

Risk: stale in-flight state could outlive the fetch and race with cache writes.

Mitigation:

- write the successful result into the normal cache before releasing waiters
- remove the in-flight entry atomically during completion

### Memory retention under failure

Risk: repeated failures could leave abandoned in-flight entries.

Mitigation:

- never retain failures beyond the single fetch lifecycle
- always remove the in-flight entry on success, failure, or dropped leader

## Validation Strategy

- add a red/green concurrency test proving two concurrent callers produce only one remote `/models`
  request
- keep the existing sequential cache reuse regression
- keep the existing authorization-scoped cache separation regression
- run provider-targeted tests, full workspace tests, and the architecture boundary check

## Expected Outcome

- concurrent auto-model cold starts collapse to one provider discovery request per key
- fewer transient allocations and retry loops on burst traffic
- lower tail latency and less external throttling pressure in low-code style fan-out execution
- a cleaner landing zone for future provider capability caching or materialized discovery state
