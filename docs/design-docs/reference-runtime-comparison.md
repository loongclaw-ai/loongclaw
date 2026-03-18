# Reference Runtime Comparison

Date: 2026-03-18
Status: Active

## Summary

LoongClaw's `alpha-test` branch is no longer missing architecture. It is missing
selective productization and app-layer orchestration patterns that other agent
projects have already validated.

This document compares four repositories:

- `loongclaw-ai/loongclaw` `alpha-test@337d9036`
- `openclaw/openclaw` `main` (fetched 2026-03-18)
- `NousResearch/hermes-agent` `main` (fetched 2026-03-18)
- `langchain-ai/deepagents` `main` (fetched 2026-03-18)

The goal is not surface-level parity. The goal is to identify what LoongClaw
should protect, what it should adopt directly, what it should adapt through its
own kernel-governed seams, and what it should avoid copying.

The recommended direction is:

- preserve LoongClaw's kernel-first and control-plane-first architecture
- adopt a middleware-style app orchestration layer above the current turn
  runtime instead of expanding ad hoc branch logic
- add a session-aware automation control plane before broad UI or channel
  expansion
- evolve memory from a registry-only seam into derivation and retrieval
  adapters while keeping canonical history LoongClaw-owned

## Why This Comparison Exists

LoongClaw already does several hard things correctly:

- strict multi-crate DAG and layer boundaries
- policy-gated tools and kernel-governed execution
- ACP modeled as a separate control plane
- truthful runtime-visible tool advertising
- snapshot/restore/experiment primitives for operator review

What it does not yet have is the same maturity in:

- long-running assistant control surfaces
- session-aware automation
- context compaction fidelity
- structured subagent ergonomics
- app-layer composition patterns that prevent future orchestration sprawl

These gaps are now more important than adding one more provider or one more
channel.

## Non-Goals

- chase OpenClaw feature parity channel-by-channel
- import Hermes's monolithic agent loop as a new core
- weaken LoongClaw's policy or approval posture to match "trust the model"
  systems
- adopt hardcoded heuristics where a typed registry, policy object, or adapter
  seam is more durable
- commit LoongClaw to a hosted memory or control-plane dependency

## Method

The comparison used repository READMEs, architecture docs, and selected source
files that define each system's real extension seams:

- OpenClaw:
  - Gateway request/control-plane entrypoints
  - cron scheduler docs
  - plugin capability and discovery model
  - dangerous tool policy lists
- Hermes Agent:
  - top-level architecture guide
  - context compressor implementation
  - tool registry and cron scheduler
  - CLI and gateway product surface
- DeepAgents:
  - `create_deep_agent(...)`
  - middleware and backend protocol layers
  - subagent middleware
  - summarization middleware
  - CLI agent assembly

This is an engineering comparison, not a marketing comparison. The emphasis is
where each project placed the seam and how much long-term maintenance debt that
seam implies.

## Current LoongClaw Position

LoongClaw's current `alpha-test` baseline has three especially strong moves:

1. The kernel remains small and explicit about the L0-L9 layer model.
2. The app layer already exposes registries for context engines, memory
   systems, ACP backends, and tool visibility.
3. Operator-facing runtime introspection already exists for context, memory, and
   ACP selection.

This means LoongClaw does not need a rewrite. It needs selective app-layer
evolution above already-correct kernel boundaries.

## Comparative Snapshot

| Dimension | OpenClaw | Hermes Agent | DeepAgents | LoongClaw | Recommendation |
| --- | --- | --- | --- | --- | --- |
| Core architecture | Large integrated gateway/runtime product | Monolithic agent loop with supporting subsystems | Graph + middleware + backend composition | Strict layered kernel + app/control-plane seams | Preserve LoongClaw core model |
| Control plane | Strongest: gateway centralizes sessions, channels, cron, skills, UI | Strong: gateway plus scheduled tasks | Light CLI/runtime management | ACP and daemon introspection are strong, but broader automation is still thin | Learn from OpenClaw control-plane productization |
| Context management | Session pruning and runtime compaction exist | Strong compaction fidelity and session continuity | Strong middleware-based summarization/offload | Context-engine seam exists, but richer compaction behavior is still immature | Learn from Hermes and DeepAgents compaction patterns |
| Memory model | Product-centric session memory, less explicit layering | Strong user/session memory and recall loop | AGENTS.md memory plus backend persistence | Correct long-term seam, but derivation/retrieval stages are still sparse | Keep LoongClaw canonical authority; adapt memory enrichment ideas |
| Tool governance | Explicit dangerous-tool policies by surface | Approval and backend isolation, but less formally layered | HITL middleware; security delegated to tools/sandbox | Strongest governance layering and truthful runtime tool view | Preserve LoongClaw model; do not weaken it |
| Subagents | Multi-agent routing and cross-session tools | Delegation plus batch/RL workflows | Cleanest subagent abstraction and isolated context model | Delegate tools exist, but ergonomics are still low-level | Learn primarily from DeepAgents here |
| Plugins / extensions | Capability ownership and plugin SDK are mature | Skills and toolsets are mature; runtime less typed | Tools and middleware more than plugin ownership | Kernel/plugin IR is strong; product-facing extension lifecycle still early | Learn OpenClaw plugin ownership model, keep LoongClaw governance |
| Product surfaces | Most mature multi-surface assistant product | Very mature CLI + gateway + learning loop | Strong coding-agent CLI | CLI-first MVP with truthful constraints | Expand conservatively after automation foundation |

## What LoongClaw Already Does Better

The most important result from this comparison is what **not** to undo.

### 1. Boundary discipline

LoongClaw's strict DAG, kernel contract surface, and policy-first architecture
are cleaner than OpenClaw and Hermes. Those projects are very productive, but
they do not provide the same level of boundary clarity between core authority,
product behavior, and extension logic.

### 2. Explicit control-plane separation for ACP

LoongClaw's decision to model ACP as a separate control plane instead of
smuggling it into provider turns is the right long-term move. Neither Hermes
nor DeepAgents gives a better replacement here.

### 3. Truthful runtime surfaces

LoongClaw's runtime tool visibility model is already better than many agent
systems that advertise a capability because it exists in code even when it is
disabled in the active runtime. This should become a general product rule, not a
one-off tool rule.

### 4. Operator review primitives

`runtime-snapshot`, `runtime-restore`, and `runtime-experiment compare` point
toward a governed optimization loop. That is a stronger base for future
learning/evolution work than letting the assistant silently mutate itself.

## Reference Learnings By Project

### OpenClaw: learn the control plane, not the sprawl

OpenClaw's most valuable idea is not "support every surface". Its most valuable
idea is that the assistant is the product and the gateway is the control plane.

What LoongClaw should learn:

- session-aware automation is a first-class product capability
- a scheduler should understand session targets and delivery targets, not just
  timestamps
- plugin ownership should map to product/company boundaries while capabilities
  stay core contracts
- channels, UI, and tools should all read from one control-plane authority

What LoongClaw should not copy:

- a broad in-process runtime without equivalent governance hardening
- feature spread before a stronger automation/session layer exists
- plugin execution models that loosen the current kernel-first trust boundary

### Hermes Agent: learn the continuity loop, not the monolith

Hermes is strongest where it treats the assistant as an always-on, learning,
cross-session operator surface rather than a one-shot agent loop.

What LoongClaw should learn:

- context compression must preserve tool-call/tool-result integrity
- scheduled work should be modeled as first-class agent tasks
- user/session continuity matters more than one more raw tool
- future improvement loops should combine memory, search, and explicit operator
  review

What LoongClaw should not copy:

- a single central loop that becomes the default place for every new behavior
- memory growth that bypasses explicit policy and canonical authority
- research-only complexity until the product control plane is ready for it

### DeepAgents: learn the composition model

DeepAgents provides the cleanest answer to a question LoongClaw is about to hit:
how do new app-layer behaviors get added without turning the conversation
runtime into a large, fragile branch tree?

What LoongClaw should learn:

- middleware ordering is a stronger seam than scattered "if feature enabled"
  checks
- backend protocols are a durable place to unify filesystem, local shell,
  sandbox, and future offload behavior
- subagents should have typed role/spec metadata, not only a raw task string
- HITL belongs in an orchestration layer above tool execution, not as
  duplicated local conditionals

What LoongClaw should not copy:

- the "trust the model" security posture
- pushing safety down only into sandbox/backends and out of top-level policy
- adopting LangGraph-style concepts mechanically if the current Rust seams
  already express the needed contract

## Adopt / Adapt / Avoid

### Adopt directly

- OpenClaw's notion of session-aware automation and delivery modes
- Hermes's tool-pair-safe context compaction discipline
- DeepAgents's middleware/backends/subagent vocabulary

### Adapt through LoongClaw seams

- gateway-style control-plane scheduling should become a daemon/ACP/session
  feature, not a separate assistant runtime
- memory enrichment should plug into LoongClaw-owned canonical history and the
  existing context-engine lifecycle
- subagents should grow out of `delegate` and `delegate_async`, not bypass them

### Avoid

- surface parity as a roadmap driver
- hardcoded feature routing that bypasses registries/policy
- hosted or product-specific lock-in for memory/control-plane behavior
- adding a UI before the automation/session layer is strong enough to support it

## Approaches Considered

### Approach A: Surface parity first

Expand channels, UI, companion runtimes, and product polish until LoongClaw
looks closer to OpenClaw.

Pros:

- easy to explain externally
- visible progress quickly

Cons:

- creates product sprawl before the orchestration base is ready
- risks widening the assistant surface faster than the current policy and
  runtime seams can comfortably support
- copies the result of other projects without copying the enabling structure

### Approach B: Intelligence parity first

Focus next on memory, self-improvement, skill growth, and long-term learning
loops in the style of Hermes.

Pros:

- attractive long-term differentiator
- aligns with current experiment/snapshot primitives

Cons:

- the memory seam is not mature enough yet
- risks adding "smartness" without enough operator-facing control
- raises governance complexity before the automation/session layer is finished

### Approach C: Orchestration seam first

Strengthen the app-layer orchestration model, then use that seam to add
automation and richer memory behavior in small governed slices.

Pros:

- smallest long-term debt
- best fit for LoongClaw's current architecture
- unlocks future automation, memory, and multi-surface work without a rewrite
- preserves kernel-first principles

Cons:

- less flashy than channel/UI expansion
- requires restraint: the first slice is architectural enablement, not visible
  product breadth

## Decision

Choose Approach C.

The next LoongClaw slice should not be "more surfaces" or "more autonomy". It
should be a better **orchestration seam** that allows those things to be added
without ad hoc growth.

## Recommended Roadmap

### Phase 1: Conversation turn middleware foundation

Introduce an ordered app-layer turn middleware stack above the current
conversation runtime. This stack should own cross-cutting behaviors such as:

- local context injection
- skills / prompt addenda
- context compaction triggers
- follow-up payload reduction
- HITL approval prompts
- subagent spawn/finish hooks

This should reuse, not replace, the current `ConversationContextEngine` seam.
The context engine remains the authority for prompt assembly and context
projection. Middleware becomes the place where pre/post-turn behavior is
composed.

### Phase 2: Session-aware automation control plane

After the middleware seam exists, add a scheduler/automation model that routes
through the same session, address, memory, and ACP binding concepts already used
by interactive turns.

The key design rule:

automation should target **sessions and delivery policies**, not raw shell
commands.

### Phase 3: Memory derivation and retrieval adapters

Once middleware and automation are stable, enrich the current builtin memory
system by giving real meaning to derivation and retrieval stages while keeping
LoongClaw as the canonical history authority.

This is where Hermes's continuity ideas and DeepAgents's history offload model
become valuable.

## Immediate Next Slice

The next concrete slice should be **Phase 1 only**:

> introduce a conversation turn middleware foundation without changing the
> kernel trust boundary or creating a second conversation runtime.

Why this slice first:

- it is the cleanest bridge between current LoongClaw seams and external best
  practices
- it lowers future automation and memory complexity
- it avoids hardcoded routing growth in `turn_coordinator`, `turn_loop`, and
  neighboring app-layer orchestration code

## Success Criteria

This comparison is only useful if it changes future decisions. The document is
successful when it leads to:

- one explicit next implementation slice instead of three competing directions
- no weakening of kernel-first governance
- no roadmap pressure to chase feature parity without orchestration readiness
- reusable app-layer seams that reduce future branch growth

## Follow-Through

The implementation plan paired with this document should cover the Phase 1 turn
middleware foundation only. Future issues should branch from that foundation
instead of bypassing it with direct feature additions.
