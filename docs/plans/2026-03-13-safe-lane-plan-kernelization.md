# Safe-Lane Plan Loop Kernelization Implementation Plan

Goal: refactor the safe-lane plan execution loop into an explicit behavior-preserving kernel with
typed state, round outcome, and next-step decision boundaries.

Architecture: keep `ConversationTurnCoordinator` as the public entrypoint, but move the implicit
safe-lane round/replan/terminal decision logic into explicit internal helpers around
`execute_turn_with_safe_lane_plan(...)`.

Tech Stack: Rust, async conversation runtime traits, plan executor / verifier runtime, cargo test

## Task 1: Lock the safe-lane kernelization target in docs

Files:
- create `docs/plans/2026-03-13-safe-lane-plan-kernelization-design.md`
- create `docs/plans/2026-03-13-safe-lane-plan-kernelization.md`

Checks:
- `rg -n "execute_turn_with_safe_lane_plan|SafeLaneFailureRoute::from_failure|with_backpressure_guard|with_session_governor_override|terminal_turn_result_from_plan_failure_with_route" crates/app/src/conversation/turn_coordinator.rs`
- `ls docs/plans/2026-03-13-safe-lane-plan-kernelization-design.md docs/plans/2026-03-13-safe-lane-plan-kernelization.md`

## Task 2: Add failing kernel-level tests

File:
- modify `crates/app/src/conversation/turn_coordinator.rs`

Add focused tests for new pure helpers that do not exist yet:

1. verify failure with retryable signal and remaining budget chooses replan
2. verify failure under governor no-replan terminalizes with governor-specific failure code
3. plan failure replan preserves failed-subgraph restart cursor
4. terminal plan failure under backpressure / governor keeps current failure-code mapping

Check:
- run the new targeted test and confirm RED

## Task 3: Introduce typed internal safe-lane kernel structures

File:
- modify `crates/app/src/conversation/turn_coordinator.rs`

Add minimal internal types for:

1. safe-lane loop state
2. round execution outcome
3. next-step decision
4. replan update

Keep them local to `turn_coordinator.rs`.

## Task 4: Refactor `execute_turn_with_safe_lane_plan(...)`

File:
- modify `crates/app/src/conversation/turn_coordinator.rs`

Use the kernel helpers to:

1. initialize session state
2. evaluate one round
3. derive next-step decision
4. apply state mutation and event emission

Preserve:

1. current event names and payload meanings
2. current route-reason semantics
3. current terminal failure-code mapping
4. current replan cursor behavior
5. current route provenance fields and labels

## Task 5: Re-run targeted safe-lane coverage

Checks:
- `cargo test -p loongclaw-app turn_coordinator::tests::safe_lane_ -- --test-threads=1`
- `cargo test -p loongclaw-app handle_turn_with_runtime_safe_lane -- --test-threads=1`

Add one high-level regression test only if the kernelized path exposes an uncovered branch.

## Task 6: Run full verification

Checks:
- `cargo test -p loongclaw-app turn_coordinator -- --test-threads=1`
- `cargo test -p loongclaw-app -- --test-threads=1`
- `cargo clippy -p loongclaw-app --all-targets --all-features -- -D warnings`

Final diff review:
- `git diff -- crates/app/src/conversation/turn_coordinator.rs docs/plans/2026-03-13-safe-lane-plan-kernelization-design.md docs/plans/2026-03-13-safe-lane-plan-kernelization.md`
