# Channel Trait Capability Refinement Design

Date: 2026-04-04
Issue: `#404`
Status: follow-up refinement proposal

> This document is a follow-up refinement proposal for the merged adapter-level
> trait direction. It does not replace or reopen the original Option A
> direction.

## Current State

Issue `#404` no longer needs a design for establishing the initial trait-based
channel abstraction. That work has already landed incrementally:

- traits layer established in `channel/traits/`
- Feishu API module moved under `channel/feishu/api/`
- `MessagingApi` implemented on `FeishuAdapter`
- `DocumentsApi` implemented on `FeishuAdapter`

## Goal

The current merged trait layer solved the primary `#404` problem: tools and
adapters now have a platform-agnostic API surface. However, `MessagingApi` and
`DocumentsApi` are still relatively wide interfaces. Platforms that support
only a subset of methods may still need to expose `ApiError::NotSupported`
branches.

This document evaluates a narrower question:

**Should the merged `MessagingApi` and `DocumentsApi` be decomposed into
smaller capability traits so partial platform support is modeled more directly
at the type boundary?**

## Background

The current channel trait layer already establishes several important constraints:

- traits are implemented at the adapter layer, not directly on low-level HTTP clients
- messaging traits reuse normalized routing and session types already used by the channel system
- the abstraction is additive and compatible with incremental migration

### Adapter boundary remains correct

The old #404 abstraction work established an important boundary that should not be reopened in this
follow-up:

- low-level clients remain focused on transport, auth, and retry
- adapters own channel-facing context such as token refresh, normalized routing, and compatibility
  with existing `ChannelAdapter` flows
- the trait layer is therefore implemented on adapters, not directly on low-level clients

That boundary is still the right one for capability refinement. Narrower traits should continue to
be adapter-level capabilities.

### Relationship with `ChannelAdapter`

The current trait layer coexists with `ChannelAdapter` rather than replacing it in a single cutover.

That coexistence remains intentional:

- `ChannelAdapter` still owns the legacy protocol-oriented receive/send boundary
- the trait layer provides a more focused capability surface for newer code
- capability refinement should preserve this staged migration model rather than forcing a second
  big-bang transition

Relevant current sources:

- `crates/app/src/channel/traits/mod.rs`
- `crates/app/src/channel/traits/messaging.rs`
- `crates/app/src/channel/traits/documents.rs`
- `crates/app/src/channel/feishu/adapter.rs`

## Problem

The remaining pain is narrower than the original #404 problem.

### 1. Wide traits still blur real platform support

`MessagingApi` and `DocumentsApi` expose a broad set of methods. Some platforms implement most of
the trait but still have to reject individual methods at runtime.

This keeps part of the platform capability matrix implicit in implementation details instead of
making it visible at the type boundary.

### 2. Callers cannot express minimal dependency precisely

A caller that only needs "append document content" or "send message" still depends on a wider
trait surface than it actually uses.

That is not catastrophic, but it weakens one of the expected benefits of the abstraction layer:
small, focused interfaces that are easy to inject and easy to mock.

### 3. `NotSupported` remains necessary in places where the type system could help

`ApiError::NotSupported` is still the right fallback for compatibility surfaces and genuinely
dynamic paths, but it should not be the dominant way to model stable capability gaps between
platforms.

## Non-Goals

This proposal is intentionally narrow. It does **not** do the following:

- redesign `ApiError`
- redesign `Pagination`
- redesign `ChannelOutboundTarget`
- redesign `ChannelSession`
- redesign `MessageContent` or `DocumentContent`
- introduce a second routing vocabulary alongside the existing channel routing model
- introduce a new kernel capability, policy, or audit model
- revisit the Option A vs Option B decision from #404
- move trait implementations from adapters back down into low-level clients
- define the full future trait model for wiki, file, drive, or chat APIs

## Design Constraints

Any refinement in this area must respect repository architecture and the already merged #404 work.

### 1. Additive first

The existing trait layer is already merged and in use. Refinement must be additive by default.
New traits are acceptable. Breaking replacement of the current trait layer is not.

### 2. Reuse existing channel vocabulary

The current trait layer intentionally reuses normalized routing and session types. Refinement must
continue to use:

- existing `ApiError`
- existing `Pagination`
- existing `ChannelOutboundTarget`
- existing `ChannelSession`
- existing `MessageContent`
- existing `DocumentContent`

This proposal must not create a second normalized surface for the same domain.

### 3. No shadow capability system

Channel capability modeling is not kernel capability authorization.

Kernel capability, policy, and audit boundaries remain authoritative for security-sensitive
execution paths. Channel trait decomposition is only an API-shape decision inside the app/channel
layer.

### 4. Single source of truth

If capability metadata is added later, it must not create two independent truth sources such as:

- one path saying a capability is supported
- another path returning `None` or `NotSupported` for the same capability

Refinement should prefer simple, mechanically consistent designs.

## Current Baseline

### Current messaging trait

`MessagingApi` currently includes:

- `send_message`
- `reply`
- `get_message`
- `list_messages`
- `search_messages`
- `edit_message`
- `delete_message`

`RichMessagingApi` extends it for card-oriented behavior.

### Current documents trait

`DocumentsApi` currently includes:

- `create_document`
- `get_document`
- `get_document_content`
- `update_document`
- `append_to_document`
- `list_documents`
- `search_documents`
- `delete_document`
- `move_document`

The narrower document capability split initially considered separate append and mutate traits, but
the implementation has since converged on a single `DocumentWriteApi` so that post-creation write
operations stay grouped under a trait name that matches their actual responsibility.

### Current Feishu reality

Feishu now implements the traits at the adapter layer, which is the correct merged abstraction
boundary. But support is not uniform across all methods.

For example, the current implementation still uses `ApiError::NotSupported` for several document
operations such as:

- full document update
- document list
- document search
- document delete
- document move

That makes Feishu a good initial case study for whether smaller traits would improve the API
boundary.

## Proposal

### Recommendation

Keep the current `MessagingApi` and `DocumentsApi` as compatibility surfaces, and introduce a
small number of narrower capability traits on top of them.

This is an additive decomposition, not a rewrite.

### Proposed document capability traits

First-pass split:

- `DocumentCreateApi`
- `DocumentReadApi`
- `DocumentWriteApi`
- `DocumentSearchApi`

Suggested ownership of methods:

- `DocumentCreateApi`
  - `create_document`
- `DocumentReadApi`
  - `get_document`
  - `get_document_content`
- `DocumentWriteApi`
  - `update_document`
  - `append_to_document`
  - `delete_document`
  - `move_document`
- `DocumentSearchApi`
  - `list_documents`
  - `search_documents`

Reasoning:

- `append_to_document` and `update_document` are both write capabilities over an existing
  document, so they belong together under a truthful write-oriented trait name.
- `list` and `search` are often paired as discovery-style operations and can remain together.
- `delete` and `move` are also write-side document mutations, so grouping them into
  `DocumentWriteApi` avoids a misleading split between "append" and other mutations while still
  keeping creation separate from post-creation writes.

### Proposed messaging capability traits

First-pass split:

- `MessageSendApi`
- `MessageQueryApi`
- `MessageEditApi`
- `MessageDeleteApi`

Suggested ownership of methods:

- `MessageSendApi`
  - `send_message`
  - `reply`
- `MessageQueryApi`
  - `get_message`
  - `list_messages`
  - `search_messages`
- `MessageEditApi`
  - `edit_message`
- `MessageDeleteApi`
  - `delete_message`

Reasoning:

- send/reply are both write-path message creation flows and belong together
- read/query methods form a coherent read capability surface
- edit and delete are often the most platform-variable operations and benefit from separation

### Why this split is intentionally conservative

This proposal does not attempt to model every micro-capability in the first pass.

It is deliberately optimized for:

- reducing the most obvious `NotSupported` mismatches
- giving callers smaller traits to depend on
- avoiding a combinatorial explosion of tiny traits
- staying close to the current merged `MessagingApi` and `DocumentsApi`

## Explicit Non-Recommendation: Second Vocabulary Rewrite

The following direction is explicitly out of scope and not recommended for this phase:

- defining a new `SendDestination` abstraction in parallel with `ChannelOutboundTarget`
- defining a new `ChannelSession` abstraction in parallel with the current normalized session type
- defining a new `Pagination` type in parallel with the existing validated pagination model
- defining a new `ApiError` hierarchy in parallel with the current channel trait errors
- defining a new `MessagePayload` hierarchy in parallel with `MessageContent`

That direction would reopen already merged #404 decisions and create a second trait-surface
vocabulary inside the same subsystem.

## Capability Discovery

Runtime capability discovery is not the primary recommendation for this phase.

The preferred calling model remains:

- inject the smallest trait the caller needs
- let adapter implementation availability determine what can be wired

If runtime capability metadata becomes necessary later, it should be introduced as a lightweight
descriptor generated from the same implementation boundary, not as a second independently maintained
discovery system.

This proposal therefore does **not** recommend:

- a `supports_*` matrix plus access/downcast traits
- dual truth sources for capability support
- an AI-agent-oriented runtime probing protocol as the main API surface

## Error Semantics

The current `ApiError` remains unchanged.

Refinement only changes where `NotSupported` is expected to appear.

### Preferred semantics after refinement

- callers using narrow capability traits should rarely see `ApiError::NotSupported`
- `NotSupported` remains acceptable on wide compatibility traits
- `NotSupported` remains acceptable in dynamic or platform-bridging code paths where the exact
  capability surface is not statically selected

This means trait decomposition should **reduce** the visible `NotSupported` surface, not attempt to
abolish the error entirely.

## Feishu Case Study

Feishu is the best immediate test bed because it already sits on the merged #404 adapter-centered
architecture and still exhibits uneven method support.

### Current likely mapping

Feishu is a natural fit for:

- `DocumentCreateApi`
- `DocumentReadApi`
- `DocumentWriteApi`
- `MessageSendApi`
- `MessageQueryApi`

Feishu is currently a poor fit, or at least an incomplete fit, for:

- `DocumentSearchApi`
- parts of `DocumentWriteApi`
- message edit depending on content and endpoint restrictions

### Expected benefit

Under a narrower trait model:

- Feishu would implement fewer "present but unsupported" methods
- new callers could request only the write or read surfaces they actually need
- capability boundaries would be visible from injected trait types instead of hidden in method
  bodies

## Migration Plan

### Phase 1: Add narrow traits

Add the new capability traits alongside the existing wide traits.

No caller breakage. No DTO churn. No routing-model changes.

### Phase 2: Adapter adoption

Implement the new narrow traits for Feishu first, reusing the existing adapter implementation
logic.

This phase should be mostly extraction and signature shaping, not behavioral redesign.

### Phase 3: Caller trials

Update a small number of new or isolated callers to depend on narrow traits such as:

- `MessageSendApi`
- `DocumentReadApi`
- `DocumentWriteApi`

This phase is where the proposal must prove that it actually improves ergonomics.

### Phase 4: Re-evaluate wide traits

Only after real caller adoption should the project decide whether:

- the wide traits remain as stable compatibility surfaces
- the wide traits become secondary convenience traits composed from the narrow traits

This document does **not** recommend planning removal of the wide traits yet.

## Acceptance Criteria

This proposal is only worthwhile if all of the following are true:

- the new design is additive on top of the merged #404 trait layer
- no second channel routing/session/error/content vocabulary is introduced
- at least one real platform can implement a narrower, more truthful capability surface
- at least one real caller can depend on a smaller trait than `MessagingApi` or `DocumentsApi`
- the amount of platform-specific `ApiError::NotSupported` exposed through normal typed usage
  decreases
- the design keeps adapter layer ownership and does not blur adapter/client boundaries
- all Rust examples introduced by follow-up docs compile on stable Rust

## Trade-offs

| Pros | Cons |
|------|------|
| smaller traits express dependencies more clearly | more traits to name, document, and wire |
| platforms can expose a more truthful capability surface | some duplication between wide and narrow traits may remain |
| callers can mock narrower interfaces more easily | migration has to be staged carefully to avoid churn |
| reduces avoidable `NotSupported` branches in typed paths | over-splitting would make the API harder to learn |

## Decision

Recommended direction:

- keep the merged #404 trait layer
- do not redesign its core DTO vocabulary
- introduce a conservative set of narrower capability traits
- validate the value on Feishu and a small number of callers before expanding the model further

## Future Work

Not part of this phase, but reasonable follow-up topics later:

- richer capability descriptors if a real runtime selection use case emerges
- optional decomposition for calendar traits if uneven support appears there too
- wiki/file/chat trait families after the current messaging/documents surfaces stabilize
- tool-layer migration patterns once narrower traits prove useful in production code

## References

- `crates/app/src/channel/traits/mod.rs`
- `crates/app/src/channel/traits/messaging.rs`
- `crates/app/src/channel/traits/documents.rs`
- `crates/app/src/channel/traits/error.rs`
- `crates/app/src/channel/types.rs`
- `crates/app/src/channel/feishu/adapter.rs`
- `crates/app/src/channel/feishu/api/messaging_api.rs`
- `docs/plans/2026-04-04-channel-trait-capability-refinement-implementation-plan.md`
- Issue #404 comments and merged follow-up steps
