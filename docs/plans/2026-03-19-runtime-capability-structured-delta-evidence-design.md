# Runtime Capability Structured Delta Evidence Design

## Context

`runtime-capability` now gives LoongClaw a governed review ladder over finished
runtime experiments:

- `propose` records one candidate artifact
- `review` records one explicit operator decision
- `show` renders one persisted candidate
- `index` groups compatible candidates into readiness-scored families
- `plan` derives one deterministic dry-run promotion plan

That ladder is now blocked on one missing substrate: the capability artifact
keeps promotion intent and review metadata, but it does not keep the
machine-readable runtime delta that explains what actually changed.

Today that structured delta only exists transiently inside
`runtime-experiment compare` when matching baseline and result snapshots are
available.

## Problem

The next self-evolution layer should not jump directly to an executor.

An executor, materializer, or later automated loop needs deterministic evidence
for the promoted change, not only free-form summaries such as:

- `mutation_summary`
- `target_summary`
- `bounded_scope`

Without structured delta evidence inside the persisted capability artifact,
later layers would have to:

- recompute compare output on demand
- depend on external snapshot paths remaining available forever
- infer material changes from prose

That is exactly the kind of fragile, token-heavy, hard-to-audit path we should
avoid.

## Goals

Add the smallest read-only evidence layer that makes capability artifacts more
self-contained and more useful for later promotion-materialization work.

This slice should:

1. capture snapshot-backed runtime delta evidence during
   `runtime-capability propose` when the source run has matching snapshots
2. persist that evidence inside the capability artifact in a backward-compatible
   way
3. surface compact delta evidence in `show`, `index`, and `plan`
4. fail clearly when the source run claims recorded snapshots but the delta
   cannot be derived deterministically

## Non-Goals

This slice should not:

- add a promotion executor
- mutate managed skills, programmatic flows, or profile notes
- introduce background indexing or caches
- invent target-specific payload synthesis heuristics
- change readiness policy based on delta evidence in the first slice

## Options

### Option A: Persist snapshot-backed delta evidence in capability artifacts

During `runtime-capability propose`, derive the recorded snapshot delta from the
source experiment when possible and store it alongside the existing
source-run summary. Also expose a compact family-level digest so `index` and
`plan` surface the new evidence without duplicating full per-candidate deltas.

Pros:

- fixes the real missing substrate instead of papering over it
- keeps the capability artifact self-contained enough for later layers
- reuses existing snapshot-compare logic rather than inventing new inference
- stays read-only and audit-friendly

Cons:

- requires a schema extension on capability artifacts
- requires careful handling for old artifacts and missing snapshot paths

### Option B: Recompute delta evidence on demand in `show`, `index`, and `plan`

Keep capability artifacts unchanged. Whenever a later command needs structured
delta evidence, reload the source run, locate its snapshots, and recompute the
delta live.

Pros:

- avoids extending the capability-artifact schema now
- minimizes immediate write-path changes

Cons:

- leaves capability artifacts incomplete
- makes later layers depend on external snapshot availability
- duplicates expensive or failure-prone recomputation
- weakens auditability because rendered evidence is no longer artifact-local

### Option C: Skip evidence work and jump to a promotion executor

Introduce `apply` now and let it use summaries, tags, and plan metadata as the
input contract.

Pros:

- feels closer to an end-to-end self-evolution loop

Cons:

- solves the wrong problem
- would force the executor to guess or scaffold payloads from prose
- increases safety risk and slop debt before the evidence model is ready

## Decision

Choose Option A.

The blocker is not "missing mutation code". The blocker is that the persisted
promotion evidence is still too shallow. We should deepen the evidence layer
first so future materialization and apply paths can stay deterministic,
lightweight, and safe.

## Proposed Model

### Candidate-level evidence

Extend `RuntimeCapabilitySourceRunSummary` with:

- `snapshot_delta: Option<RuntimeExperimentSnapshotDelta>`

Rationale:

- the delta belongs to the source run, not to the abstract proposal
- the capability artifact already stores other source-run evidence
- the field can stay optional for backward compatibility and no-snapshot runs

### Family-level digest

Extend `RuntimeCapabilityEvidenceDigest` with a compact summary derived from all
candidate-level deltas:

- `delta_candidate_count`: number of candidates that include snapshot delta
- `changed_surfaces`: sorted union of changed runtime surfaces observed across
  the family

Rationale:

- `index` and `plan` need operator-facing signal without embedding every raw
  before/after pair into the family summary
- later layers can still drill into per-candidate artifacts when they need the
  full delta

## Command Behavior

### `runtime-capability propose`

Behavior:

- load the finished experiment run as today
- derive the optional snapshot delta from the source run's recorded baseline and
  result snapshots
- persist the delta under `source_run.snapshot_delta`

Delta rules:

- if the run has no result snapshot, store `None`
- if the run has result snapshot metadata but no recorded snapshot artifact
  paths, store `None`
- if both snapshot paths exist, compute the delta deterministically from those
  snapshots
- if both snapshot paths exist but the delta cannot be derived because the
  snapshot files are unreadable, mismatched, or malformed, fail the command

That last rule avoids false-success artifacts with silently degraded evidence.

### `runtime-capability show`

Add compact rendering for:

- whether snapshot delta evidence exists
- changed surface count
- changed surfaces

JSON output naturally includes the persisted `snapshot_delta` field.

### `runtime-capability index`

Aggregate candidate-level delta evidence into the family digest:

- `delta_candidate_count`
- `changed_surfaces`

This slice does not change readiness scoring yet. Delta evidence is surfaced for
operator review, not used as a hard gate.

### `runtime-capability plan`

Reuse the family digest so the dry-run plan shows which runtime surfaces
actually changed across the candidate family.

This makes the plan more actionable without pretending we already have a
materialized lower-layer payload.

## Reuse Strategy

Do not duplicate snapshot-delta logic inside `runtime-capability`.

Instead:

- extract or reuse the existing compare helpers from `runtime_experiment_cli`
- build one small helper that conditionally derives recorded snapshot delta from
  a finished run artifact

That keeps one source of truth for delta semantics.

## Error Handling

Expected outcomes:

- old capability artifacts without `snapshot_delta` continue to load
- no-snapshot experiment runs still produce valid capability artifacts
- broken recorded snapshots fail during `propose` rather than producing partial
  evidence

This preserves backward compatibility without hiding real source-evidence
corruption.

## Testing

Required coverage:

1. `runtime-capability propose` persists `snapshot_delta` when the source run
   has recorded snapshots
2. `runtime-capability propose` leaves `snapshot_delta=None` when no recorded
   snapshots are available
3. `runtime-capability propose` fails when recorded snapshot paths exist but are
   unreadable or inconsistent
4. `runtime-capability index` reports the expected `delta_candidate_count` and
   `changed_surfaces`
5. `runtime-capability plan` surfaces the same aggregated delta digest
6. older fixture artifacts without the new field still deserialize

## Why This Slice Next

This is the smallest step that:

- improves self-evolution fidelity
- keeps safety stronger than convenience
- reduces future recomputation cost
- creates a truthful substrate for later `materialize` or `apply` work

It deepens the evidence pyramid before we add any new mutation power.
