# Turn Checkpoint Persistence Design

Date: 2026-03-13
Branch: `feat/turn-loop-kernelization-20260313`
Scope: durable provider-turn checkpoint persistence without event-replay-first coupling
Status: implemented on this branch

## Goal

Persist a compact, versioned turn checkpoint around the provider-path finalization boundary so the
runtime can audit and recover finalization progress without introducing a replay-first execution
model or a new storage schema.

## Why this slice exists

The in-memory `TurnCheckpointSnapshot` seam already makes the outer provider turn path typed and
snapshot-friendly, but it still disappears if the process exits between:

1. user/assistant turn persistence
2. `after_turn(...)`
3. context compaction

That gap matters because reply durability and post-turn side effects are separate runtime phases.
If the system loses process state in that window, we want an auditable durable marker that says
which phase completed and where finalization stopped.

## Design choice

Use a versioned `conversation_event` named `turn_checkpoint` with a compact payload:

1. `schema_version`
2. `stage`
3. `checkpoint`
4. `finalization_progress`
5. optional `failure`

The `checkpoint` field serializes the existing typed provider-turn seam:

1. preparation summary
2. provider request decision
3. lane execution summary
4. reply decision summary
5. finalization summary

This keeps the durable contract aligned with the kernel seam instead of re-deriving control flow
from ad hoc assistant event histories.

## Stage model

The persisted protocol uses three stages:

1. `post_persist`
2. `finalized`
3. `finalization_failed`

### `post_persist`

Persisted immediately after user/assistant turns are durably written.

Meaning:

1. the user-visible reply is durable
2. `after_turn` may still be pending
3. context compaction may still be pending

### `finalized`

Persisted after all configured post-turn side effects finish.

Meaning:

1. the reply is durable
2. `after_turn` completed or was skipped
3. compaction completed, was skipped, or failed open

### `finalization_failed`

Persisted when a fail-closed post-turn step aborts finalization after reply persistence.

Meaning:

1. the reply is durable
2. the finalization boundary did not finish cleanly
3. the payload identifies the failed step and captured error string

## Progress model

`finalization_progress` tracks the per-step outcome with a small status vocabulary:

1. `pending`
2. `skipped`
3. `completed`
4. `failed`
5. `failed_open`

This lets the runtime distinguish:

1. an interrupted turn that stopped after reply persistence
2. a cleanly finalized turn
3. a fail-open compaction path
4. a fail-closed post-finalization failure

## Why not persist the full reply in the checkpoint

That would push the architecture toward replay/reconstruction from checkpoint payloads and would
duplicate the assistant reply in the durable protocol. This slice intentionally stops one boundary
later:

1. persist the real user/assistant turns first
2. persist a compact kernel checkpoint describing finalization state

That makes recovery meaningful for the `after_turn` / compaction window without inflating the
checkpoint into a second source of truth for the reply body.

## Why reuse `conversation_event`

Reusing the existing generic conversation-event channel keeps storage stable and avoids schema
churn. This is acceptable because provider history assembly already filters internal assistant
records with:

1. `type = "conversation_event"`
2. `type = "tool_decision"`
3. `type = "tool_outcome"`

So persisted checkpoints remain queryable in memory history without contaminating prompt context.

## Recovery value

This slice does not attempt full interrupted-turn replay.

It does provide durable evidence for the highest-value partial-finalization window:

1. if the latest checkpoint is `post_persist`, the reply is durable but finalization did not finish
2. if the latest checkpoint is `finalization_failed`, the reply is durable and the failed step is
   explicit
3. if the latest checkpoint is `finalized`, the provider turn reached a clean durable boundary

That is enough for future repair tooling, harness diagnostics, or recovery loops to avoid guessing.

The branch now also includes:

1. a typed `turn_checkpoint` analytics summary layer for latest-stage / latest-progress /
   failure-step recovery reads
2. a restart-style sqlite verification that proves checkpoint events survive durable reload while
   remaining filtered out of provider prompt history
3. a narrow session checkpoint reader plus CLI summary surface so operators can inspect
   `not_durable` / `pending_finalization` / `finalized` / `finalization_failed` state without
   introducing a replay subsystem or a new policy loop
4. a narrow tail-repair harness that only replays `after_turn` / compaction from the durable
   checkpoint boundary and persists a new terminal `turn_checkpoint` event
5. a manual CLI repair entrypoint (`/turn_checkpoint_repair`) so operators can run that tail
   repair without re-entering provider turn execution, including explicit refusal reasons such as
   `checkpoint_identity_missing` and `checkpoint_identity_mismatch`
6. a checkpoint turn-identity fingerprint so repair can verify that the rebuilt visible
   user/assistant tail still matches the durable checkpoint before rerunning side effects

## Non-goals

1. no giant orchestrator abstraction
2. no event-replay-first turn reconstruction
3. no new database schema
4. no policy retuning mixed into checkpoint persistence
5. no duplication of full assistant replies inside checkpoint events

## Tail repair semantics

The repair harness intentionally stays narrower than a workflow replay system.

It does:

1. load the latest durable `turn_checkpoint`
2. derive a typed recovery action from the persisted finalization state
3. rebuild current visible session context from durable memory
4. recover the latest durable user/assistant turn pair
5. verify that recovered pair against the checkpoint identity fingerprint
6. rerun only the missing tail steps (`after_turn`, compaction)
7. persist a new `finalized` or `finalization_failed` checkpoint event

It does not:

1. rerun provider inference
2. reconstruct the assistant reply from checkpoint payloads
3. replay arbitrary intermediate lane state
4. auto-retry when the durable reply turn itself is unavailable
5. guess when the rebuilt visible tail no longer matches the durable checkpoint

This keeps the recovery boundary aligned with the external durability guidance used for the
design:

1. resume from persisted boundaries, not arbitrary lines of code
2. keep side effects isolated and idempotent where possible
3. avoid monolithic replay of a larger turn block when only the finalization tail is incomplete

Automatic recovery recommendation is additionally gated by checkpoint identity presence. If the
latest durable tail requires recovery but no identity fingerprint is present, summary surfaces,
startup health, and the repair entrypoint all downgrade to manual inspection instead of attempting
tail execution on a weakly identified checkpoint.

## Validation hardening

The current validation matrix now covers durable restart behavior, not just in-memory repair flow.

It verifies:

1. a successful repair writes a new terminal `finalized` checkpoint into durable sqlite history
2. a repeated repair on that same session becomes a noop instead of rerunning `after_turn` or
   compaction side effects
3. a failed repair durably writes `finalization_failed`
4. a later retry resumes from that failed durable tail and only reruns the remaining step
