# Memory Context Kernel Unification Design

Date: 2026-03-12
Status: Approved for implementation

## Goal

Align memory-context semantics across conversation runtime paths so that
`kernel_ctx` and non-kernel execution both hydrate the same profile/summary/window
 context for the model.

## Problem

Today `DefaultConversationRuntime::build_messages(...)` routes memory reads through
the kernel only as a raw `window` request when `kernel_ctx` is present. The
non-kernel provider path uses `load_prompt_context(...)`, which can inject:

- `profile_note` for `profile_plus_window`
- deterministic summary block for `window_plus_summary`
- normal sliding-window turns

That creates a semantic split: the same configured `memory.profile` behaves
differently depending on whether the caller has a kernel context.

## Constraints

- Keep the public memory plane additive-only.
- Reuse the existing memory runtime config and SQLite backend.
- Do not introduce vector retrieval or LLM-generated summaries.
- Keep the implementation small enough for one reviewable patch slice.

## Decision

Add a new memory-core operation for prompt-context hydration and route
`DefaultConversationRuntime::build_messages(...)` through that operation when
`kernel_ctx` is present.

This slice will:

- add a shared memory operation constant for prompt context
- expose structured context entries from the memory core
- teach the conversation runtime to decode those entries into provider messages
- add tests that prove summary/profile entries now survive kernel-routed paths

This slice will not yet:

- split runtime events into a separate storage lane
- refactor SQLite into a long-lived engine
- introduce provider-runtime caching

## Rationale

This completes the architecture direction already documented in the approved
memory design: memory behavior should be profile-first, backend-second, and all
runtime paths should use a shared hydration layer.

## Verification

- Add failing tests for kernel-routed build-messages under
  `window_plus_summary` and `profile_plus_window`.
- Run the new targeted tests first.
- Run `cargo test --workspace --all-features`.
- Run `./scripts/check_architecture_boundaries.sh` and confirm the memory
  operation literal warning is reduced or eliminated for this slice.
