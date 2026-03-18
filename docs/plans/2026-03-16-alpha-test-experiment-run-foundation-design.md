# Alpha-Test Experiment-Run Foundation Design

## Problem

`alpha-test` now has a clearer runtime-state substrate:

- `runtime-snapshot` captures a reproducible runtime checkpoint
- the follow-up runtime replay slice adds lineage metadata and reversible
  restore semantics

That closes the "checkpoint and replay" gap, but it does not yet close the
next experiment-management gap.

An operator still has no first-class artifact that records one experiment run as
an object with:

- a baseline snapshot
- the intended mutation or hypothesis
- a result snapshot
- evaluation notes or metrics
- an explicit promotion or rejection decision

Without that layer, snapshot lineage remains necessary but insufficient. The
system can say "this runtime state existed" and "this runtime state can be
restored", but it still cannot say "this particular experiment branch was tried,
compared, and either accepted or rejected".

For the broader "agent matrix" idea, this is the next missing middle service.
It turns reversible runtime state into comparable experiment runs.

## Goal

Define the smallest LoongClaw-native experiment-run slice that:

1. turns snapshot lineage into explicit experiment-run records
2. lets operators record a baseline snapshot, a mutation summary, a result
   snapshot, and an evaluation outcome in one artifact
3. keeps the first delivery record-oriented and local-first
4. stays safe enough to ship before any autonomous optimizer or command runner
5. provides a stable schema and CLI contract for later automation

## Non-Goals

- executing arbitrary commands as part of the experiment-run command
- automatically mutating skills or provider/runtime settings
- automatically promoting or rolling back branches
- replacing snapshot artifacts with a larger archive bundle
- embedding full snapshot payloads inside experiment-run artifacts
- building a full experiment graph browser or dashboard in the first slice

## Constraints

- This slice depends on the runtime snapshot lineage and restore contract from
  issue `#208` / PR `#211`. It should be implemented only after that base lands
  on `alpha-test`.
- The first artifact format must remain plain JSON, operator-readable, and easy
  to diff in git or local storage.
- The command surface should not require live runtime mutation or config writes.
  It should operate on experiment artifacts, not on the running daemon state.
- The first schema should be strict about identifiers and metrics, but light on
  policy. The system must not pretend to enforce automated selection logic that
  has not been designed yet.
- Snapshot artifacts remain the source of truth for replayable runtime state.
  Experiment-run artifacts are orchestration records, not restore sources.

## Approach Options

### Option A: Add only lineage browsing and snapshot diff surfaces

This would add better visibility over snapshot ancestry and perhaps a small
viewer for parent/child relations.

Pros:

- smallest implementation cost
- no new artifact type
- useful for operators doing manual inspection

Cons:

- still does not record experiment intent or evaluation outcome
- still leaves promotion and rejection decisions in ad-hoc notes
- improves observability, not experiment management

### Option B: Add a minimal experiment-run record artifact and CLI

This adds a dedicated experiment-run document that references snapshot lineage,
records mutation intent, attaches result snapshots, and stores operator-entered
evaluation and decision data.

Pros:

- creates the missing middle layer between checkpointing and optimization
- keeps the first slice local, deterministic, and operator-controlled
- avoids coupling the new surface to shell execution or automatic mutation
- gives later automation a stable record contract to build on

Cons:

- adds another JSON artifact type that must be explained clearly
- creates lightweight duplication between snapshot lineage summaries and the
  experiment-run record
- does not yet automate anything by itself

### Option C: Jump directly to a full autonomous experiment runner

This would let LoongClaw restore a snapshot, execute a benchmark or command,
capture a result snapshot, score it, and optionally promote it.

Pros:

- highest headline value
- closest to the long-term self-optimizing-agent vision

Cons:

- couples orchestration, execution, evaluation, policy, and promotion into one
  slice
- raises immediate questions about shell execution, approvals, rollback
  semantics, and evaluator trust
- creates the highest slop-debt risk before the record contract is proven

## Decision

Choose Option B.

The next LoongClaw slice should add a minimal experiment-run record layer first.

This is the smallest change that turns runtime snapshot lineage into a usable
experiment-management service. It is materially more useful than a viewer-only
step, and materially safer than jumping straight to autonomous optimization.

The core design principle is:

> snapshot artifacts describe replayable runtime state; experiment-run artifacts
> describe one attempt to evolve or evaluate that state.

## Proposed Surface

### Command family

Add a new top-level daemon command family:

- `loongclaw runtime-experiment start`
- `loongclaw runtime-experiment finish`
- `loongclaw runtime-experiment show`

This keeps naming aligned with `runtime-snapshot` and `runtime-restore`.

### Start command

Purpose:

- create a new experiment-run artifact from a baseline snapshot artifact

Suggested interface:

```text
loongclaw runtime-experiment start \
  --snapshot path/to/baseline.json \
  --output path/to/run.json \
  --mutation-summary "enable browser companion preview skill" \
  [--experiment-id exp-browser-preview] \
  [--label browser-preview-a]
  [--tag browser]
  [--tag preview]
  [--json]
```

Behavior:

- load the baseline snapshot artifact
- inherit `experiment_id` from the baseline snapshot when present
- require explicit `--experiment-id` when the baseline snapshot has no
  experiment id
- generate a stable `run_id`
- persist a new experiment-run artifact with:
  - status `planned`
  - decision `undecided`
  - baseline snapshot summary
  - operator-entered mutation summary and tags

### Finish command

Purpose:

- attach the result of a completed experiment run

Suggested interface:

```text
loongclaw runtime-experiment finish \
  --run path/to/run.json \
  --result-snapshot path/to/result.json \
  --evaluation-summary "tool discoverability improved; cost unchanged" \
  --metric task_success=1 \
  --metric token_delta=0 \
  --decision promoted \
  [--warning "manual verification only"] \
  [--status completed]
  [--json]
```

Behavior:

- load the existing run artifact
- require the current run status to be `planned`
- load the result snapshot artifact when provided
- if the result snapshot has an explicit `experiment_id`, require it to match
  the run's `experiment_id`
- if the result snapshot has no `experiment_id`, allow the finish to proceed but
  record a warning
- parse `--metric key=value` pairs as numeric values
- persist:
  - result snapshot summary
  - evaluation summary
  - metrics
  - warnings
  - final status (`completed` or `aborted`)
  - decision (`undecided`, `promoted`, `rejected`)

### Show command

Purpose:

- render the experiment-run artifact without mutation

Suggested interface:

```text
loongclaw runtime-experiment show --run path/to/run.json [--json]
```

Behavior:

- load and print the run artifact
- text output should surface the most decision-relevant fields first:
  - run id
  - experiment id
  - baseline snapshot id
  - result snapshot id
  - status
  - decision
  - metrics
  - warnings

## Proposed Artifact Model

The first schema should stay narrow and avoid embedding full snapshots.

Suggested document shape:

```json
{
  "schema": {
    "version": 1,
    "surface": "runtime_experiment_run",
    "purpose": "snapshot_based_experiment_tracking"
  },
  "run": {
    "run_id": "a73c...",
    "experiment_id": "exp-browser-preview",
    "label": "browser-preview-a",
    "status": "planned",
    "decision": "undecided",
    "created_at": "2026-03-16T14:00:00Z",
    "updated_at": "2026-03-16T14:00:00Z"
  },
  "baseline": {
    "snapshot_id": "f03d...",
    "snapshot_path": "artifacts/baseline.json",
    "label": "baseline",
    "created_at": "2026-03-16T13:30:00Z"
  },
  "mutation": {
    "summary": "enable browser companion preview skill",
    "tags": ["browser", "preview"]
  },
  "result": null,
  "evaluation": null,
  "warnings": []
}
```

Key points:

- `baseline` and `result` store snapshot lineage summaries plus optional local
  file paths
- `mutation.summary` stays as a single operator-entered string in the first
  slice
- `evaluation.metrics` is a flat numeric map, not a nested scoring system
- `warnings` belongs at the run level so it can carry cross-field integrity
  notes such as experiment-id drift

## Why Not Extend Snapshot Artifacts Directly

Snapshot artifacts should remain about one checkpointed runtime state and its
restore contract.

Adding evaluation outcome, promotion state, or mutation intent directly into the
snapshot artifact would create two problems:

1. one snapshot may participate in multiple experiment branches
2. replay state and experiment outcome would become tightly coupled even though
   they evolve on different timelines

Keeping a separate experiment-run artifact preserves this boundary:

- snapshot = replayable state
- experiment-run = one comparison attempt over replayable states

## ID and Consistency Rules

The first slice should enforce a few small but important integrity rules:

1. `run_id` is deterministic from baseline snapshot id, created-at timestamp,
   experiment id, label, and mutation summary.
2. `experiment_id` must exist on the run record, either inherited or explicitly
   passed.
3. `finish` refuses malformed metrics and unknown decisions or statuses.
4. `finish` refuses result snapshots whose explicit `experiment_id` conflicts
   with the run record.
5. `finish` refuses to mutate a run already marked `completed` or `aborted`.

These rules are intentionally small. They protect operator trust without turning
the first slice into a policy engine.

## Error Handling

The first slice should prefer deterministic operator-facing errors:

- missing baseline snapshot file -> hard error
- snapshot artifact schema mismatch -> hard error
- missing experiment id on both the baseline snapshot and the `start` command ->
  hard error with guidance
- malformed metric argument -> hard error naming the offending token
- conflicting result snapshot experiment id -> hard error
- missing result snapshot experiment id -> warning recorded on the run artifact

No background repair or silent coercion should occur.

## Testing Strategy

The implementation should land with integration coverage for:

1. CLI parsing for `runtime-experiment start|finish|show`
2. `start` inheriting `experiment_id` from the baseline snapshot
3. `start` requiring explicit `--experiment-id` when the baseline snapshot lacks
   one
4. `finish` attaching result snapshot lineage and metrics
5. `finish` rejecting conflicting experiment ids
6. `finish` recording a warning when the result snapshot lacks an experiment id
7. text rendering surfacing status, decision, snapshot ids, and metrics
8. JSON payload stability for the run artifact schema

## Rollout

This slice should land only after the runtime snapshot lineage/restore work is
merged to `alpha-test`.

Once it exists, LoongClaw gains a clean sequence:

1. capture baseline snapshot
2. run a manual or scripted experiment outside LoongClaw
3. capture result snapshot
4. persist one experiment-run record with evaluation and decision

That gives the project a truthful stepping stone toward later work on:

- automated experiment execution
- evaluator pipelines
- promotion policies
- graph views over experiment branches

## Recommendation

Implement the experiment-run record layer before any autonomous optimizer.

It is the smallest slice that turns checkpoint lineage into a reusable
experiment-management service while staying aligned with LoongClaw's current
architecture and minimal-change philosophy.
