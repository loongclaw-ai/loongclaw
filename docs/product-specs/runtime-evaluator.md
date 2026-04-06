# Runtime Evaluator

## User Story

As a LoongClaw operator, I want one staged evaluator surface for runtime
experiments and capability families so that promotion decisions are based on
repeatable evidence instead of ad-hoc intuition.

## Acceptance Criteria

- [ ] LoongClaw exposes a `runtime-evaluator` command family with `run`,
      `show`, and `compare` subcommands.
- [ ] `runtime-evaluator run` accepts one explicit source reference:
      a finished `runtime-experiment` run or one indexed `runtime-capability`
      family.
- [ ] Every evaluator run records one explicit stage:
      `smoke`, `canary`, or `full`.
- [ ] Every evaluator run records:
      baseline reference,
      candidate reference,
      suite id,
      metrics,
      warnings,
      operator notes,
      and one final decision from `keep`, `discard`, or `retry`.
- [ ] `runtime-evaluator show` round-trips the persisted evaluator artifact as
      JSON and renders the stage, decision, and decision-critical evidence
      first in text output.
- [ ] `runtime-evaluator compare` summarizes multiple evaluator runs against the
      same candidate or capability family without mutating runtime state.
- [ ] Product docs describe `runtime-evaluator` as the operator-facing evidence
      layer above `runtime-experiment` and `runtime-capability`, not as an
      autonomous optimizer.
- [ ] Evaluator artifacts remain auditable and deterministic enough to support
      later promotion policy without requiring hidden heuristics.

## Out of Scope

- Automatically editing code, prompts, skills, or config
- Automatically promoting a candidate into live runtime state
- Online learning or background policy training
- Hidden scorer heuristics that cannot be inspected from stored artifacts
- Long-running daemonized evaluator services in the first iteration
