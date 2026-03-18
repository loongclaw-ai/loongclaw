# Hybrid Turn Kernel Convergence Design

Date: 2026-03-13
Branch: `feat/turn-loop-kernelization-20260313`
Scope: research-calibrated convergence path after fast-lane and safe-lane kernelization
Status: working design note

## Why this note exists

Fast-lane turn-loop kernelization and safe-lane plan-loop kernelization are now in place, but the
outer hybrid turn path still determines whether the architecture can evolve into a stronger runtime
without reintroducing implicit state transitions.

The next problem is no longer "can the loop run?". The next problem is "where is the shared turn
contract between lanes, reply finalization, retries, and persisted state?"

## External calibration

This note is based on the following primary-source references:

1. LangGraph overview: https://docs.langchain.com/oss/python/langgraph/overview
2. LangGraph Graph API overview: https://docs.langchain.com/oss/javascript/langgraph/graph-api
3. LangGraph persistence: https://docs.langchain.com/oss/python/langgraph/persistence
4. Semantic Kernel Agent Architecture: https://learn.microsoft.com/en-us/semantic-kernel/frameworks/agent/agent-architecture
5. Temporal Continue-As-New: https://docs.temporal.io/develop/typescript/continue-as-new

## What the references agree on

### 1. Execution needs an explicit state model

LangGraph treats execution as graph/state transitions instead of ad hoc loop mutation.
Semantic Kernel separates agents, threads, orchestration patterns, and plugins.
Temporal treats workflow state and activity execution as separate concerns.

Implication for loongclaw:
Current lane internals are getting healthier, but the hybrid turn boundary still needs a more
explicit shared state/decision contract.

### 2. Retry boundaries must be explicit and auditable

LangGraph exposes retry policy at runtime boundaries.
Temporal emphasizes retry classes, idempotent activities, and terminal vs retryable failures.

Implication for loongclaw:
`safe_lane` already has route/backpressure/governor logic, but the architecture should keep retry
and finalization decisions as explicit typed transitions instead of repeated inline match logic.

### 3. Reply finalization is a real kernel boundary

Across these systems, the runtime separates execution output from the commit/update step that turns
execution results into durable state, user-visible output, or the next control transition.

Implication for loongclaw:
The provider/lane execution result should not directly inline the reply-finalization policy in each
caller. A shared reply-finalization kernel is the correct convergence seam.

### 4. Structured contracts are safer than repeated local conventions

LangGraph relies on shared state schema and control-flow primitives.
Semantic Kernel relies on explicit orchestration patterns and thread/message contracts.
Temporal relies on deterministic workflow/activity contracts.

Implication for loongclaw:
Duplicated follow-up semantics, external-skill activation parsing, and raw-fallback behavior should
move into shared helpers or shared decision types before more policy is added.

### 5. Long-running execution needs a compact checkpoint seam

LangGraph persistence is thread-scoped and records checkpoints at explicit super-step boundaries.
Semantic Kernel keeps thread state separate from orchestration patterns and plugin execution.
Temporal uses Continue-As-New as a deliberate carry-forward boundary when a workflow grows too long.

Implication for loongclaw:
The eventual durable turn checkpoint boundary should snapshot a compact typed turn state, not replay
opaque emitted events until control flow becomes reconstructable by accident.

## Current loongclaw position

### What is already good

1. `ConversationTurnLoop` now has explicit per-turn session state, round evaluation, and round
   decision types.
2. `execute_turn_with_safe_lane_plan(...)` now has explicit safe-lane loop state, round execution,
   and failure decision helpers.
3. Safe-lane replan cursor derivation already depends on round outputs instead of executor internals.

### What still needs convergence

1. `ConversationTurnCoordinator` still owns a large outer hybrid turn path.
2. Reply finalization policy is shared conceptually across lanes but was historically implemented as
   repeated local logic.
3. External-skill activation and completion-with-raw-fallback semantics can drift if duplicated.
4. Durable per-turn checkpointing across lane boundaries is still event-centric, not kernel-centric.

## Immediate convergence target

The next narrow, high-value step is not a giant shared orchestrator. It is a smaller and more exact
boundary:

1. shared reply-finalization decision vocabulary
2. shared external-skill follow-up activation rules
3. shared completion-with-raw-fallback helper

This keeps naming honest and keeps the architecture aligned with the real control seam.

## What this branch now implements toward that target

1. `turn_shared.rs` owns external-skill activation parsing and rejects truncated invoke payloads.
2. `turn_shared.rs` owns completion-with-raw-fallback behavior.
3. `ConversationTurnCoordinator` now derives an explicit `TurnReplyDecision` before persistence and
   after-turn side effects.
4. `ConversationTurnCoordinator` now uses an explicit provider-turn session state and a single
   provider-turn commit/finalization boundary for success and inline-provider-error paths.
5. `ConversationTurnCoordinator` now derives an explicit provider-request decision and an explicit
   provider-lane-execution result before reply finalization.
6. `ConversationTurnCoordinator` now derives a `ProviderTurnPreparation` and
   `ProviderTurnLanePlan` so lane selection, raw-output mode, and tool-step limits enter the outer
   provider path as a typed preparation stage instead of anonymous locals.
7. Fast-lane and safe-lane now share a narrow budget vocabulary:
   `TurnRoundBudget`, `SafeLaneReplanBudget`, and `EscalatingAttemptBudget`.
   This keeps round-followup, replan exhaustion, and attempt-escalation logic out of anonymous
   integer comparisons.
8. Safe-lane failure routing now carries explicit route provenance
   (`base_routing`, `backpressure_guard`, `session_governor`) so verify/plan finalization and
   runtime audit paths do not need to infer override origin from `reason` strings alone.
9. Safe-lane route reasons now share a typed vocabulary
   (`SafeLaneFailureRouteReason`) across base routing, backpressure terminalization, governor
   override, verify terminalization, checkpoint provenance, and event emission. The runtime still
   emits stable snake_case strings outward, but the kernel no longer branches on ad hoc literals.
10. Safe-lane failure codes now also have an explicit classifier boundary for durable consumers.
    Health derivation, governor-history recovery, and other event-driven readers can distinguish
    known verify/backpressure/governor failure families without relying on substring or prefix
    heuristics over persisted strings. Generation paths are converging on the same vocabulary, so
    write-side terminalization and read-side durability logic no longer drift independently.
11. The provider-path turn boundary now has a compact typed checkpoint seam spanning preparation,
   provider request outcome, lane execution summary, reply decision summary, and finalization
   summary. This is still in-memory/internal only, but it turns the outer turn path into a
   snapshot-friendly kernel without committing to event-schema churn.
12. Session-history reads are converging on a shared assistant-window seam that is explicitly bound
    to the active memory configuration. Checkpoint summaries, safe-lane summaries, and
    governor-history recovery now share the same kernel-first, durable-fallback read boundary
    instead of silently drifting across duplicate window-loading helpers.
13. Safe-lane analytics now has an internal typed-rollup seam on top of the durable event summary.
    Persisted payloads and outward summary fields remain stable string-based schema for backward
    compatibility, but health/governor readers can now consume typed final failure codes, typed
    route reasons, typed backpressure counters, and typed final-status rollups without rebuilding
    classifiers ad hoc at each read site.
14. The outer provider-path turn now also converges on a narrow resolved-outcome seam before
    persistence/finalization. Provider request handling, lane execution, reply resolution, and
    finalization mode selection still happen in stages, but they now collapse into one typed
    resolved turn contract before checkpoints and post-turn durability are applied.
15. Provider-path and turn-loop reply finalization now share a smaller tool-driven reply seam.
    Raw-reply derivation, tool-result/tool-failure followup payload extraction, and non-tool
    fallthrough semantics no longer live in two drifting local evaluators; they are derived from
    one shared helper in `turn_shared`.
16. Provider-path and turn-loop followup tail construction now also converges on a shared helper
    seam in `turn_shared`. Assistant preface carry-forward, external-skill invoke promotion,
    loop-warning insertion, truncation-hint prompting, and tool_result/tool_failure assistant
    payload shapes no longer drift across duplicated local builders. The remaining intentional
    difference is budget policy: the turn loop maps payloads through bounded truncation, while the
    provider-path coordinator keeps unbounded followup payload text.
17. Turn-loop hard-stop guard followup construction now also sits on the shared kernel side.
    The guard marker, user prompt shape, and optional carry-forward of the latest tool payload are
    no longer private `turn_loop` string assembly rules. The only turn-loop-local behavior left on
    that path is the truncation budget mapper applied to the carried tool payload before the shared
    guard tail is emitted.
18. The remaining thin local followup payload wrappers have now been removed as well. Both
    `turn_loop` and the provider-path coordinator consume `ToolDrivenFollowupPayload` directly, and
    checkpoint followup-kind derivation now depends on a shared typed kind helper rather than local
    `ToolResult`/`ToolFailure` mirror enums drifting separately in each runtime path.
19. The repeated pre-decision transition logic now also converges on a shared base decision seam.
    Non-tool direct finalization, raw-tool-output short-circuiting, and followup-candidate
    derivation (`raw_reply` plus shared followup payload) are no longer duplicated across
    `turn_loop` and the provider-path coordinator. Local paths now only apply their own
    post-decision policy: round budget / loop hard-stop shaping on one side, direct
    completion-pass resolution on the other.
20. Provider-path reply checkpoints now also reuse the shared followup kind type directly.
    The durable `followup_kind` field no longer depends on a coordinator-local mirror enum with the
    same `tool_result` / `tool_failure` vocabulary. Shared payload kind, reply decision, and
    checkpoint summary are now aligned on one serialized type boundary.
21. The provider-path reply decision shell has also been removed. The coordinator now consumes
    `ToolDrivenReplyBaseDecision` directly for decision, checkpoint summary derivation, and
    completion-pass resolution instead of wrapping that same shared shape in a local
    `TurnReplyDecision` enum first.
22. Tool-driven followup dispatch now also converges on a shared payload router in `turn_shared`.
    Provider-path and turn-loop callers no longer keep separate result-vs-failure match shells just
    to select the same shared tail builders. The remaining intentional divergence is still local:
    provider-path prepends base messages unchanged, while the turn loop applies followup-payload
    truncation and keeps the loop-guard branch private.
23. Durable turn-checkpoint repair now also consumes a shared typed recovery plan instead of only
    a flat recovery action. Summary surfaces and the repair executor converge on the same kernel
    contract for: whether repair is needed, whether repair is manual-only, which tail steps remain
    runnable, and which existing per-step statuses should be carried forward before rerunning the
    tail. Runtime-only downgrade reasons such as visible-tail mismatch remain local, but
    summary-derived repair semantics no longer drift across analytics, CLI reporting, and the
    repair executor.
24. Tail-repair outcomes now also expose a typed reason vocabulary instead of returning raw
    string literals from the coordinator path. Shared summary-derived manual reasons and
    runtime-only downgrade reasons now sit behind one enum boundary, so CLI reporting, tests, and
    future harness code do not need to rely on ad hoc string comparisons when evaluating repair
    results.
25. Repair execution now runs against a typed resume input instead of ad hoc `build_context`
    output. That resume seam restores the original finalization envelope by rebuilding context with
    the provider-path system layer included and by preferring checkpoint-carried estimated-token
    metadata when deciding whether compaction should rerun. The repair path therefore converges on
    the same finalization inputs normal provider turns used, instead of drifting based on whatever
    a later context reassembly happens to estimate.
26. The repair resume seam now also refuses context-shape drift when the checkpoint still carries
    a preparation snapshot. If the rebuilt visible envelope no longer matches the original
    `context_message_count` boundary, repair downgrades to manual inspection instead of replaying
    tail hooks against a widened or narrowed history window. This keeps constrained tail recovery
    aligned with the original provider turn instead of silently absorbing later summary/profile
    reshaping.
27. Repair preparation intake is now typed as well. The repair path no longer cherry-picks raw
    JSON fields for `estimated_tokens` and `context_message_count`; it parses a minimal typed
    preparation view and treats malformed checkpoint preparation payloads as manual-only recovery.
    That keeps corrupted durable state from silently degrading into best-effort tail replay.
28. Repair admissibility now also rejects content drift, not only envelope-shape drift. New
    checkpoints persist a preparation-context fingerprint over the original pre-assistant
    finalization envelope, and repair compares that fingerprint against the rebuilt visible
    pre-assistant envelope before rerunning any tail hooks. If the count still matches but the
    actual context content has drifted, repair downgrades to manual inspection instead of
    replaying compaction or after-turn hooks against a different conversation state.
29. Runtime-only repair downgrades are now observable without executing repair. The coordinator
    exposes a narrow runtime-gate probe that rebuilds the typed resume input, checks the same
    admissibility rules, and reports a manual-only downgrade reason when current session context
    has drifted beyond what durable summary state can prove. CLI startup health can therefore show
    fingerprint/context mismatches before any tail hook rerun is attempted, without turning the
    repair path into a replay-first "try it and see" flow.
30. Runtime admissibility is now routed through one local typed seam instead of being re-derived
    separately by repair execution and runtime probe paths. The coordinator now evaluates one
    narrow eligibility result that distinguishes summary-derived terminal states, runtime-only
    manual downgrades, and truly runnable tail repair inputs. The same seam also lets
    `/turn_checkpoint_summary` surface a probe line for the same requested window, so summary and
    runtime probe no longer disagree just because they sampled different history ranges.
31. The runtime-gate seam is now also covered as an explicit four-state matrix instead of only a
    single positive probe example. Tests now prove that probe output is suppressed for
    `NotNeeded`, summary-derived `InspectManually`, and fully runnable repair states, while the
    runtime-only manual downgrade remains the only case that emits a probe line. This keeps
    observability tight: the CLI only warns when runtime context drift adds new information beyond
    durable summary state, not for already-known or safely recoverable conditions.
32. Provider-turn request normalization now also converges on one shared typed seam. Fast-lane
    `turn_loop` and the provider-path coordinator no longer keep separate local mappings from
    `CliResult<ProviderTurn>` plus `ProviderErrorMode` into continue / inline-provider-error /
    propagate-error branches. The shared seam ends exactly there: downstream persistence,
    checkpoint shaping, and completion/finalization side effects still stay local to each path.
    This is the intended convergence boundary because the three-way request outcome is genuinely
    shared semantics, while the consequences of that outcome are not.
33. Reply persistence class is now also shared, but only at the exact point where semantics are
    truly identical. Fast-lane finalization, provider-path finalization, and ACP/raw fallbacks now
    all route normal replies versus inline provider-error replies through one typed
    `ReplyPersistenceMode` vocabulary and one persistence selector. The convergence still stops
    before checkpoint emission, after-turn execution, and context compaction, because those later
    lifecycle stages remain materially different between the fast path and the provider-path
    coordinator.
34. Reply resolution mode is now also shared at the base-decision layer. The direct-versus-
    completion-pass distinction no longer lives only as coordinator-local checkpoint naming;
    `ToolDrivenReplyBaseDecision` now exposes one shared `ReplyResolutionMode`, and the
    coordinator checkpoint simply records that mode. This keeps the shared seam aligned with the
    already-shared base decision instead of re-encoding the same branch locally under a second
    enum.
35. Budget decisions are now slightly more typed and less boolean-driven. Fast-lane
    `TurnRoundBudget` now exposes an explicit follow-up decision instead of forcing callers to
    branch on raw index math, while safe-lane replan budget and backpressure budget now both
    expose a shared continuation decision that either allows progress or returns a typed terminal
    reason. This keeps budget terminalization closer to the same decision-first style already used
    for request normalization, reply resolution, and reply persistence.
36. Safe-lane terminal route profiling is now also more local and less stringly-derived. The
    route itself now owns its verify summary label and terminal code mappings for verify and plan
    failure paths, instead of forcing multiple free functions to re-match the same
    `source/reason` tuple. This keeps terminalization semantics attached to the typed route object
    that already carries provenance.
37. Safe-lane route transition shaping is now also localized onto the route type instead of being
    spread across separate coordinator helpers. Base failure classification, backpressure
    terminalization, session-governor override, and event-facing route labels now all live on
    `SafeLaneFailureRoute` itself, while the coordinator only composes those typed transitions.
    This preserves the existing event schema and lifecycle boundaries without re-inflating a new
    runtime-level abstraction.
38. Durable checkpoint observability now also consumes the typed safe-lane terminal route that was
    already being persisted in lane snapshots. Turn-checkpoint analytics and chat summary/health
    output now surface that stored route decision/reason/source directly instead of dropping the
    provenance after persistence. This improves recovery-time diagnosis without changing the
    checkpoint schema or turning repair into a replay-first reconstruction path.
39. The durable checkpoint plane now also reuses the shared route-decision / route-source
    vocabulary instead of maintaining a second raw-string-only representation in analytics. This
    keeps runtime routing, persisted lane snapshots, and operator-facing checkpoint summaries on
    one narrow typed contract while still leaving execution lifecycle control local to the
    coordinator.
40. Checkpoint repair and startup-health reporting now also preserve override-driven terminal route
    provenance when recovery downgrades to manual inspection. The repair action still stays
    local to the checkpoint tail state, but the manual reason can now distinguish safe-lane
    backpressure and session-governor terminals from generic state inspection. This keeps durable
    recovery hints typed and specific without conflating route provenance with replay-oriented
    reconstruction logic.
41. Route-aware checkpoint recovery hints are now explicitly terminal-only, and checkpoint route
    label projection is centralized on the event summary instead of being re-expanded in each chat
    formatter. This avoids accidental reuse of non-terminal route snapshots for manual-repair
    diagnosis and trims another small repetition seam without widening the public architecture.
42. Durable repair hint classification now also validates override-pair consistency instead of
    trusting persisted route source alone. Backpressure-specific and session-governor-specific
    manual reasons are emitted only when the persisted terminal route carries a source/reason pair
    that is internally coherent, so malformed or mixed snapshots degrade back to generic manual
    inspection instead of overstating what the runtime previously decided.
43. Runtime tail-repair probes now also expose their diagnostic source explicitly instead of
    flattening everything into action/reason only. This keeps operator-facing probe output aligned
    with the already-typed internal eligibility source and prevents runtime-gate findings from
    looking indistinguishable from summary-derived manual repair conclusions.
44. Tail-repair outcomes now also preserve summary-versus-runtime source on the terminal operator
    surface instead of discarding that distinction after probe time. This keeps `turn_checkpoint_
    repair` aligned with probe diagnostics and makes manual-required outcomes auditable without
    inferring provenance from reason strings alone.
45. Turn-checkpoint operator diagnostics now also converge on a narrow local render seam instead
    of re-projecting summary, repair, and probe labels separately in each formatter. Startup
    health, summary output, runtime probe, and repair output still keep their existing outward
    roles, but they now share the same typed label projection for recovery action/reason/source
    and for summary-derived state labels. That keeps future checkpoint observability expansion
    from drifting across multiple string assembly sites while avoiding any new runtime-wide
    "orchestrator" abstraction.
46. Turn-checkpoint diagnostics are now also assembled from a coordinator-owned typed report
    instead of making the chat layer pair summary loading with a separate runtime-probe call.
    Summary-derived recovery assessment and optional runtime-gate findings still remain narrow and
    read-only, but they now cross the coordinator/chat boundary as one typed diagnostic contract.
    That reduces another summary-versus-runtime drift seam without widening repair execution into a
    general replay subsystem.
47. Turn-checkpoint diagnostics now also consume a single session-history snapshot instead of
    reloading the memory window once for summary state and again for runtime-gate eligibility.
    Summary recovery assessment and latest checkpoint payload are derived from the same assistant
    window snapshot, so operator diagnostics no longer admit cross-read drift when kernel-routed
    memory changes between calls. This keeps the seam local to session history and coordinator
    logic rather than introducing a broader caching or replay layer.
48. The turn-checkpoint session-history seam now also folds assistant checkpoint events into
    summary state and latest-checkpoint payload in one parse pass instead of first building
    summary state and then reparsing the same snapshot to recover the latest checkpoint body.
    This keeps the kernelized diagnostics path narrow and deterministic while reducing repeated
    JSON decoding inside the same history snapshot boundary.
49. Safe-lane governor history now also consumes a single safe-lane history projection instead of
    parsing assistant event content once for governor failure samples and then reparsing the same
    history again for safe-lane summary rollups. The summary surface and the governor sample
    surface stay separate, but they now share one local fold seam, which removes another small
    consistency and overhead leak without introducing a broader runtime cache layer.
50. Provider-path and turn-loop reply entry now also share an explicit `ToolDrivenReplyPhase`
    contract instead of leaving the coordinator with a private `TurnReplyEvaluation ->
    decide_turn_reply_action` shell while the turn loop talked directly to `ToolDrivenReplyKernel`.
    The shared phase stays narrow on purpose: it only owns the reply-entry projection from
    assistant preface, tool-intent presence, raw-output mode, and turn result into one
    typed base decision plus resolution metadata. Checkpoint persistence, round-budget policy,
    and completion-pass execution remain local to their callers.
51. `ToolDrivenReplyPhase` now also carries the projected raw-reply view used by the turn loop's
    round-limit fallback path. The fast path no longer asks one helper for `raw_reply` and another
    helper for the base decision over the same turn result. That removes a small but real drift and
    recomputation seam while keeping the phase boundary local to reply-entry projection rather than
    widening it into a broader lane or persistence abstraction.
52. Provider-path resolved outcomes now also own their final `TurnCheckpointSnapshot` at resolve
    time instead of reconstructing that snapshot later during apply/finalization from
    `preparation + user_input + scattered resolved fields`. This makes the resolved outcome seam
    more self-contained and snapshot-friendly: request classification, lane summary, reply summary,
    and finalization summary now cross the resolve/apply boundary as one compact object rather than
    being re-derived from parallel inputs.
53. Propagated provider errors now also participate in the durable checkpoint seam instead of
    silently skipping it. A `return_error` outcome now persists a single `finalized` checkpoint
    event with skipped finalization progress, and analytics no longer equates "checkpoint durable"
    with "reply durable" for that path. This closes an auditability hole without pretending that a
    provider error produced a persisted assistant reply.
54. The provider-path `Continue` branch now also crosses a narrower local phase seam before reply
    resolution. Request shaping, lane execution summary, and tool-driven reply projection are
    carried together as one `ProviderTurnContinuePhase` instead of being re-derived as parallel
    locals inside `resolve_provider_turn(...)`. The seam stays intentionally coordinator-local:
    it reduces hybrid-turn glue without widening a new cross-runtime abstraction.
55. Provider-path reply finalization now also enters the tail through a compact local phase input
    instead of reassembling `reply + after_turn_messages + estimated_tokens` as unrelated locals at
    the call site. `ProviderTurnReplyTailPhase` keeps the finalization boundary explicit without
    inflating resolved outcomes with full session payloads, so the durable tail remains typed and
    local rather than drifting back into ad hoc apply-side plumbing.
56. Resolved provider outcomes now also cross into apply/finalization through a typed terminal
    phase instead of making `apply_resolved_provider_turn(...)` re-branch over raw resolved data.
    Persisted replies and propagated provider errors still terminate differently, but that
    difference now sits behind one local `ProviderTurnTerminalPhase` seam. This keeps
    resolve/apply/tail ownership explicit without turning provider-path finalization into a new
    runtime-wide orchestration layer.
57. Operator-facing turn-checkpoint diagnostics now also distinguish reply durability from
    checkpoint durability explicitly. Sessions that only persisted a terminal checkpoint (for
    example propagated provider errors) no longer show up as a vague `durable=0` state without
    context; chat summary and startup health now surface `checkpoint_durable=1` with
    `durability=checkpoint_only`, which keeps the internal checkpoint semantics auditable at the
    same precision the kernel now maintains internally.
58. `checkpoint_durable` now also lives on `TurnCheckpointEventSummary` itself instead of being
    re-inferred inside chat from `session_state != not_durable`. That keeps operator surfaces
    honest about where durability semantics come from: analytics owns the fact that a checkpoint
    event was durably observed, and chat just renders the typed summary. This is a small change,
    but it closes one more inference seam between the checkpoint kernel and its operator-facing
    diagnostics.

This is still behavior-preserving for normal paths, but it hardens one previously duplicated unsafe
edge: truncated external-skill invoke payloads no longer activate managed skill context in the
coordinator path.

## Next phases after this note

### Phase 1: shared hybrid turn transition contract

Introduce a narrow typed contract for the outer hybrid turn path:

1. provider turn obtained
2. lane execution completed
3. reply finalization decision derived
4. persistence/after-turn/context-compaction applied

### Phase 2: typed retry budget kernel

Unify the notion of round budget, retry budget, and terminalization budget so fast-lane and
safe-lane stop encoding retry ceilings in unrelated local primitives.

This phase should stay narrow:

1. fast-lane follow-up continuation should depend on a typed round budget, not direct index math
2. safe-lane replan exhaustion should depend on a typed retry budget, not raw `round/max_rounds`
3. safe-lane node-attempt growth should depend on a typed escalating attempt budget, not manual
   increment/min logic
4. safe-lane failure routing should carry typed provenance for terminalization overrides so verify
   and plan failure finalization can consume a route object instead of stringly-typed reason
   plumbing
5. total-attempt backpressure and terminalization can remain separate until a later slice proves a
   stronger merged budget contract is actually needed

### Phase 3: durable turn checkpoint boundary

Persist a compact typed turn snapshot around lane execution and finalization so interrupted or
partially failed turns can be resumed or audited without reconstructing everything from emitted
assistant events. The right model is closer to LangGraph thread checkpoints / super-step snapshots
and Temporal carry-forward execution state than to a flat append-only event replay.
That checkpoint should preserve typed route provenance when terminalization is override-driven,
because backpressure/governor exits are runtime-state facts, not just presentation strings.

Current convergence slice:

1. `TurnCheckpointSnapshot` exists as a narrow provider-path seam and is now persisted through a
   versioned `turn_checkpoint` conversation event.
2. That durable payload is stage-aware (`post_persist`, `finalized`, `finalization_failed`) so the
   runtime can distinguish reply durability from later finalization progress.
3. Safe-lane terminal route provenance is carried into the outer lane execution summary instead of
   being re-inferred from terminal error codes.
4. The commit/finalization boundary consumes typed finalization summary rather than a raw
   persistence mode argument and now records post-turn progress explicitly.
5. Context compaction durability is explicit at the checkpoint layer:
   `skipped`, `completed`, `failed_open`, and fail-closed `failed` are no longer implicit runtime
   side effects.
6. Replay/time-travel concerns remain separate from base turn-state reconstruction. The checkpoint
   is a compact kernel contract, not a second full-fidelity assistant-reply store.

## Non-goals

1. No giant "orchestrator" abstraction.
2. No premature shared code between fast-lane and safe-lane execution engines.
3. No policy retuning mixed into basic kernel convergence work.
4. No event-schema churn unless a later phase proves it is necessary.

## Acceptance signal for the current stage

The architecture is moving in the right direction when:

1. duplicated turn-finalization semantics are removed or minimized
2. retry/finalization branches become typed and unit-testable
3. external-skill activation safety is consistent across lanes
4. new policy work can be implemented by editing typed decision helpers rather than expanding large
   monolithic turn handlers
