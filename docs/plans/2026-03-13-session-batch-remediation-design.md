# Session Batch Remediation Design

## Context

LoongClaw's current session operator surface is materially stronger than it was at the start of
this track:

- `sessions_list` can now discover visible delegate children with machine-readable filters
- `session_status` / `session_wait` expose stable delegate lifecycle metadata
- `session_cancel` handles queued cancellation and cooperative running cancellation
- `session_recover` handles overdue queued and overdue running async delegate children

That leaves one practical operator gap: once discovery returns several visible stale delegate
children, the operator still has to remediate them one-by-one. The underlying state transitions are
already race-safe, but the tool surface still forces repetitive single-target calls.

## Problem

We need a thicker operator remediation slice without adding new top-level tools or weakening the
existing visibility and state guards.

The design question is how to let operators preview and apply remediation across several visible
delegate children while preserving:

- backward compatibility for existing single-session callers
- race-safe conditional state transitions
- per-target evidence about why a requested action did or did not apply

## Goals

- Extend `session_recover` and `session_cancel` to support batch targeting.
- Add `dry_run` preview semantics for safe operator planning.
- Keep single-session existing calls backward compatible.
- Return machine-readable per-target outcomes instead of failing the whole batch on the first
  inapplicable target.
- Preserve visibility boundaries and repository-backed conditional state transitions.

## Non-Goals

- No new top-level tools such as `session_recover_many` or `session_cancel_many`.
- No hard kill, PID targeting, restart recovery, or worker lease management.
- No all-or-nothing multi-session transaction semantics.
- No pagination or saved query model for remediation batches.
- No broadening of delegated child authority.

## Options Considered

### Option 1: Add dedicated batch tools

Examples:

- `session_recover_many`
- `session_cancel_many`

Pros:

- explicit names
- leaves current tools untouched

Cons:

- expands tool count for behavior already owned by existing remediation tools
- duplicates schema and documentation surface
- encourages thin wrappers instead of strengthening the current operator path

Rejected.

### Option 2: Extend existing tools with `session_ids` and `dry_run`

Keep `session_recover` and `session_cancel` as the only remediation tools, but allow:

- `session_id` for the current single-target path
- `session_ids` for batch targeting
- `dry_run` for non-mutating preview

Pros:

- preserves a compact tool surface
- layers naturally on top of `sessions_list`
- reuses the current race-safe repository writes
- lets operators preview mixed applicability before mutating state

Cons:

- response shape needs a new aggregated form for batch and preview flows
- tools must normalize visibility, planning, and state-changed outcomes per target

Chosen.

### Option 3: Keep tools single-target and rely on caller loops

Pros:

- zero schema change
- smallest implementation diff

Cons:

- no shared `dry_run` preview
- no structured mixed-result summary
- pushes repetitive control logic and error handling into every caller

Rejected.

## Chosen Approach

Extend the existing remediation tools instead of adding new ones.

### Request model

`session_recover` and `session_cancel` accept:

- `session_id: string` for backward-compatible single-target calls
- `session_ids: string[]` for batch calls
- `dry_run: boolean` for preview mode

Exactly one of `session_id` or `session_ids` must be present.

### Backward compatibility

Preserve the current response shape when all of the following are true:

- request uses `session_id`
- `dry_run` is absent or `false`

In that legacy path, successful single-target execution keeps returning the existing inspection
payload plus `recovery_action` / `cancel_action`.

Use the new aggregated response shape when:

- `session_ids` is present, or
- `dry_run = true`

This keeps existing callers stable while allowing richer operator flows for new callers.

### Aggregated response shape

Batch and preview flows return:

- `tool`
- `current_session_id`
- `dry_run`
- `requested_count`
- `result_counts`
- `results`

Each `results[]` item includes:

- `session_id`
- `result`
- `message`
- `action` when the target is applicable
- fresh inspection payload fields when the target is visible and inspectable

The main result buckets are:

- `would_apply`
- `applied`
- `skipped_not_visible`
- `skipped_not_recoverable`
- `skipped_not_cancellable`
- `skipped_state_changed`

The first two distinguish preview from actual mutation. The skipped buckets retain the causal class
without forcing the caller to parse free-form error strings.

### Visibility and race semantics

Per target:

1. normalize the requested session id
2. enforce visibility from the current session
3. inspect the session and build a recover/cancel plan
4. if `dry_run = true`, report `would_apply` without mutating state
5. otherwise apply the existing conditional finalize/transition write

If the conditional write loses a race, the result becomes `skipped_state_changed` for that target.
The batch continues; it does not abort on the first race.

### Error handling philosophy

Batch mode should only hard-fail on malformed input, not on mixed applicability.

Examples of batch hard failures:

- both `session_id` and `session_ids` are supplied
- neither target field is supplied
- `session_ids` contains no non-empty strings

Examples of per-target skipped outcomes:

- target is not visible
- target is already terminal
- target is not an async delegate child
- target is fresh rather than overdue
- target state changes before the conditional write commits

## Data Model

No schema change is required.

The slice continues to use:

- `sessions`
- `session_events`
- `session_terminal_outcomes`

The implementation is an app-layer orchestration enhancement over the existing repository APIs.

## Testing

Add focused coverage for:

- provider schema updates exposing `session_ids` and `dry_run`
- `session_recover` dry-run preview with mixed visible recoverable and non-recoverable targets
- `session_recover` batch apply with partial success and per-target result classification
- `session_cancel` dry-run preview with queued/running/applicability mixing
- `session_cancel` batch apply with queued terminalization and running cancel-request writes
- preservation of legacy single-target response behavior
