# Channel Trait Capability Refinement Implementation Plan

**Goal:** Introduce a conservative set of narrower channel capability traits on top of the merged
#404 trait layer, validate them on the Feishu adapter, and prove value through at least one real
caller path without redesigning the current DTO or routing vocabulary.

**Architecture:** Keep `MessagingApi` and `DocumentsApi` as additive compatibility surfaces, add
narrow traits in the existing trait modules, implement only truthful subsets on `FeishuAdapter`,
and route one or more real Feishu send paths through helpers that depend on the smaller traits.

**Tech Stack:** Rust app crate, existing `channel/traits` DTOs, `async_trait`, Feishu adapter
tests, focused cargo test/clippy/fmt verification.

---

## Task 1: Persist the follow-up design and execution artifacts

**Files:**
- Existing: `docs/plans/2026-04-04-channel-trait-capability-refinement-design.md`
- Create: `docs/plans/2026-04-04-channel-trait-capability-refinement-implementation-plan.md`

**Step 1: Keep the refinement design document as the public design source**

Use `docs/plans/2026-04-04-channel-trait-capability-refinement-design.md` as the design baseline
for this slice.

**Step 2: Save this implementation plan under `docs/plans/`**

This plan is the execution companion for the follow-up refinement proposal.

## Task 2: Add the narrow capability traits without changing the merged DTO surface

**Files:**
- Modify: `crates/app/src/channel/traits/messaging.rs`
- Modify: `crates/app/src/channel/traits/documents.rs`
- Modify: `crates/app/src/channel/traits/mod.rs`

**Step 1: Add narrow messaging traits to the existing messaging module**

Add:

- `MessageSendApi`
- `MessageQueryApi`
- `MessageEditApi`
- `MessageDeleteApi`

Each trait should reuse the current types already exposed by `messaging.rs`:

- `Message`
- `MessageContent`
- `Pagination`
- `SendOptions`
- `ChannelOutboundTarget`
- `ChannelSession`

**Step 2: Add narrow document traits to the existing documents module**

Add:

- `DocumentCreateApi`
- `DocumentReadApi`
- `DocumentAppendApi`
- `DocumentSearchApi`
- `DocumentMutateApi`

Each trait should reuse the current document DTOs and `Pagination`.

**Step 3: Re-export the new traits from `channel/traits/mod.rs`**

Expose them alongside the current wide traits so new callers can adopt them without extra module
churn.

**Step 4: Preserve the current wide traits exactly**

Do not:

- replace `MessagingApi`
- replace `DocumentsApi`
- add new supertrait bounds to the existing wide traits
- redefine `ApiError`, `Pagination`, `ChannelOutboundTarget`, `ChannelSession`, `MessageContent`,
  or `DocumentContent`

This slice is additive only.

## Task 3: Implement truthful narrow-trait support on `FeishuAdapter`

**Files:**
- Modify: `crates/app/src/channel/feishu/adapter.rs`

**Step 1: Implement the clearly supported messaging traits**

Implement on `FeishuAdapter`:

- `MessageSendApi`
- `MessageQueryApi`

Implement `MessageDeleteApi` only if the current behavior is genuinely supported across the
existing adapter contract and does not merely preserve a compatibility-only `NotSupported` branch.

Defer `MessageEditApi` if edit support is still content- or endpoint-restricted enough that the
narrow trait would not be truthful.

**Step 2: Implement the clearly supported document traits**

Implement on `FeishuAdapter`:

- `DocumentCreateApi`
- `DocumentReadApi`
- `DocumentAppendApi`

Do not implement:

- `DocumentSearchApi`
- unsupported parts of `DocumentMutateApi`

unless the adapter can actually satisfy those methods without simply forwarding `NotSupported`.

**Step 3: Share existing adapter logic rather than forking behavior**

The narrow trait impls should delegate to the same underlying helper methods and API resources used
by the current wide trait impls.

This slice should mainly be about shaping interfaces, not changing platform semantics.

## Task 4: Add focused tests for the new narrow traits

**Files:**
- Modify: `crates/app/src/channel/feishu/adapter.rs`
- Modify: `crates/app/src/channel/traits/messaging.rs`
- Modify: `crates/app/src/channel/traits/documents.rs`

**Step 1: Add compile-time trait conformance checks**

Add small helper assertions in test code such as:

- `assert_message_send_api::<FeishuAdapter>()`
- `assert_message_query_api::<FeishuAdapter>()`
- `assert_document_create_api::<FeishuAdapter>()`
- `assert_document_read_api::<FeishuAdapter>()`
- `assert_document_append_api::<FeishuAdapter>()`

These checks ensure the expected trait ownership is explicit.

**Step 2: Add behavior tests through the narrow traits**

Add focused tests that call Feishu through the narrow traits rather than through the wide traits
for operations that are already supported:

- send message
- query message
- create document
- read document
- append document

**Step 3: Avoid fake symmetry tests**

Do not add tests that force unsupported narrow traits to exist just for API symmetry.

The absence of an impl is the point.

## Task 5: Route at least one real caller through a narrow trait

**Files:**
- Modify: `crates/app/src/channel/feishu/mod.rs`
- Modify: `crates/app/src/channel/feishu/webhook.rs`
- Modify: `crates/app/src/channel/feishu/adapter.rs` if helper extraction is needed

**Step 1: Introduce a small helper using `MessageSendApi`**

Extract a helper on an existing live send path that depends on `MessageSendApi` instead of the
wide `MessagingApi` or concrete method lookup on `FeishuAdapter`.

Candidate paths:

- `run_feishu_send` in `crates/app/src/channel/feishu/mod.rs`
- webhook reply send path in `crates/app/src/channel/feishu/webhook.rs`

**Step 2: Keep the caller trial narrow and local**

Do not wait for the full future `tools/channel/` extraction to prove value.

This slice only needs one real caller path to demonstrate:

- a smaller dependency surface
- no new DTO churn
- no routing-model rewrite

**Step 3: Defer document caller migration if no natural production seam exists yet**

If there is no equally natural documents caller in current production code, validate document
traits with adapter-focused tests first and defer document caller adoption to the subsequent tool
extraction slice.

## Task 6: Decide whether compatibility helpers are necessary

**Files:**
- Modify: `crates/app/src/channel/traits/messaging.rs` if needed
- Modify: `crates/app/src/channel/traits/documents.rs` if needed

**Step 1: Evaluate whether helper wrappers improve adoption**

After the narrow traits exist, decide whether small helper functions are needed for ergonomic
bridging in tests or callers.

Examples:

- helper functions generic over `MessageSendApi`
- helper functions generic over `DocumentReadApi`

**Step 2: Do not add blanket impls that misrepresent support**

Specifically avoid:

- blanket impls that automatically make every `MessagingApi` implement all narrow messaging traits
- blanket impls that automatically make every `DocumentsApi` implement all narrow document traits

That would recreate the original problem by making unsupported capability surfaces look statically
available.

## Task 7: Run verification

**Files:**
- Modify: none

**Step 1: Run focused trait and Feishu tests**

Run focused tests for the changed areas, for example:

- `cargo test -p loongclaw-app channel::feishu::adapter -- --nocapture`
- `cargo test -p loongclaw-app channel::traits -- --nocapture`

If the exact test filters change while implementing, keep the scope focused on the new narrow trait
surface and Feishu adapter paths.

**Step 2: Run repository verification**

Run:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --locked`
- `cargo test --workspace --all-features --locked`

**Step 3: Inspect the scoped diff**

Run:

- `git status --short`
- `git diff -- docs/plans/2026-04-04-channel-trait-capability-refinement-design.md`
- `git diff -- docs/plans/2026-04-04-channel-trait-capability-refinement-implementation-plan.md`
- `git diff -- crates/app/src/channel/traits/mod.rs crates/app/src/channel/traits/messaging.rs crates/app/src/channel/traits/documents.rs crates/app/src/channel/feishu/adapter.rs crates/app/src/channel/feishu/mod.rs crates/app/src/channel/feishu/webhook.rs`

## Exit Criteria

This slice is complete when:

- narrow traits exist and are re-exported
- `FeishuAdapter` implements only the narrow traits it can truthfully support
- at least one live send path depends on `MessageSendApi`
- no new DTO or routing vocabulary has been introduced
- the current wide traits still compile unchanged
- focused and full verification pass
