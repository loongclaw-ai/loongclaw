# Provider Model Discovery In-Flight Coalescing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate duplicate concurrent `/models` fetches in auto-model mode by coalescing cache misses behind a single in-flight request per provider identity.

**Architecture:** Extend `provider/model_cache.rs` with a short-lived in-flight registry keyed exactly like the existing model cache. `provider/model_selection.rs` reads the normal cache first, joins an in-flight fetch on concurrent misses, and only the leader performs the real network request. Success still flows through the existing bounded cache and failures fan out without caching.

**Tech Stack:** Rust, tokio sync primitives, reqwest, sha2

---

### Task 1: Lock concurrent miss behavior with a failing regression test

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`

**Step 1: Write the failing test**

Add a new async test that:

- starts a local `/models` listener
- holds the first response open long enough for a second concurrent caller to arrive
- launches two `fetch_available_models_with_policy(...)` calls concurrently
- asserts both return the same models
- asserts the listener only observed one real request

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p loongclaw-app provider::model_selection::tests::fetch_available_models_with_policy_coalesces_concurrent_cache_misses -- --exact --nocapture
```

Expected: FAIL because the current implementation issues duplicate remote fetches on concurrent cache miss.

### Task 2: Add in-flight leader and waiter plumbing

**Files:**
- Modify: `crates/app/src/provider/model_cache.rs`

**Step 1: Add an in-flight registry**

Create a bounded-lifetime map keyed by `ModelListCacheKey` that stores waiter channels for the
active fetch.

**Step 2: Add leader/waiter API**

Expose a small internal API that lets model selection:

- register as leader on the first miss
- subscribe as waiter on subsequent concurrent misses
- publish success or failure to all waiters
- clean up safely if the leader drops before completion

**Step 3: Add focused cache-internal tests**

Cover:

- followers receive the leader result
- dropped leaders release waiters with an error

### Task 3: Route model selection through coalesced discovery

**Files:**
- Modify: `crates/app/src/provider/model_selection.rs`

**Step 1: Join in-flight fetches before sending the request**

After the normal cache miss, attempt to register an in-flight leader. If another fetch is already
active for the same key, await the shared result and return it directly.

**Step 2: Preserve existing success semantics**

When the leader succeeds:

- parse models
- write the normal cache
- publish the result to followers

**Step 3: Preserve existing failure semantics**

When the leader fails:

- do not write the normal cache
- publish the same error to followers
- keep retry policy and error formatting unchanged

**Step 4: Verify targeted tests**

Run:

```bash
cargo test -p loongclaw-app provider::model_selection:: -- --nocapture
```

Expected: PASS

### Task 4: Final verification and isolation checks

**Files:**
- No new files unless verification exposes a regression

**Step 1: Run provider regression suite**

```bash
cargo test -p loongclaw-app provider:: -- --nocapture
```

Expected: PASS

**Step 2: Run workspace verification**

```bash
cargo test --workspace --all-features
```

Expected: PASS

**Step 3: Run architecture boundary script**

```bash
./scripts/check_architecture_boundaries.sh
```

Expected: `provider_mod` and `memory_mod` remain within budget
