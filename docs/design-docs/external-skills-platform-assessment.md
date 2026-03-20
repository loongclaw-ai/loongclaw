# External Skills Platform Assessment

## Scope and Baseline

- Audited branch: `origin/dev`
- Audited commit: `7e8cf50f` (`fix: tighten browser-only web search follow-ups`)
- Assessment date: 2026-03-20

Issue `#158` was originally framed around `alpha-test`, but upstream no longer
publishes that branch. The managed-runtime closure that motivated the issue was
promoted and then extended on `dev`, so the durable design-doc layer now needs
to describe current upstream reality rather than a historical `alpha-test`
snapshot.

The key question is no longer whether LoongClaw supports external skills at
all. The current code clearly proves that it does. The harder question is what
kind of platform LoongClaw has actually built on `dev`:

- a governed managed-install runtime only
- a governed multi-scope instruction-package platform
- or a richer metadata-heavy ecosystem surface comparable to the strongest
  reference systems

The current answer is the middle one.

## Executive Summary

LoongClaw `dev` now has a real external-skills platform, not just the
managed-install closure that existed on the old `alpha-test` line.

The current code proves all of the following:

- external skills are integrated into the discovery-first runtime, where
  `tool.search` and `tool.invoke` remain the only provider-core tools and
  external skills stay discoverable behind leases instead of widening the
  provider schema directly
- `loongclaw skills` is now a real operator-facing CLI with `list`, `info`,
  `install`, `install-bundled`, `enable-browser-preview`, `remove`, and
  `policy` flows
- the runtime maintains a managed install root and persisted install index,
  supports replacement rollback, and exposes bundled first-party skill install
  paths in addition to local directory and archive installs
- skill discovery now resolves managed, user, and project scopes and surfaces
  lower-priority duplicates as `shadowed_skills` for operator debugging
- `external_skills.invoke` still returns instruction packages rather than
  executable native actions, but those instructions are carried back into the
  conversation loop intact

That is materially stronger than the older story captured in the first draft of
PR `#160`, which was closed because it had already become outdated.

At the same time, the current platform is still narrower than Claude Code,
Codex, and OpenClaw in several important ways:

- metadata is still thin: LoongClaw currently understands only `name` and
  `description` frontmatter for discovered skills
- scope precedence is managed-first rather than repo-first, so project-local
  copies do not override managed installs
- there is no built-in registry, update, sync, or marketplace workflow in the
  LoongClaw runtime itself
- local user and project scopes are discovered from disk when the runtime is
  enabled, but they do not yet have the same operator-visible policy and
  lifecycle surface as managed downloads and installs
- skills still teach the model how to use existing tools; they do not yet
  define executable action contracts of their own

So the most accurate current position is:

- LoongClaw is no longer a migration-only or managed-install-only story
- LoongClaw now has a governed multi-scope instruction-package platform
- LoongClaw is still behind the strongest reference systems on metadata,
  ecosystem workflow, and operator ergonomics breadth

## Status Versus The 2026-03-12 Runtime-Closure Plan

The implementation plan in
`docs/plans/2026-03-12-external-skills-runtime-closure.md` is substantially
closed and has since been extended.

Closed from that plan:

- managed install root and persisted install index
- install/list/inspect/invoke/remove lifecycle tools
- managed archive install support
- `SKILL.md` instruction loading back into the conversation loop
- governed fetch policy and approval gates

Landed after that plan, and therefore missing from the original assessment
draft:

- operator-facing `loongclaw skills` CLI
- bundled managed skill installation
- browser-preview bootstrap through a first-party bundled helper skill
- managed, user, and project discovery scopes with duplicate shadow reporting
- discovery-first provider routing that keeps external skills behind
  `tool.search` and `tool.invoke`

Still open after the current `dev` audit:

- richer per-skill metadata and eligibility contracts
- broader scope model beyond managed, user, and project
- registry, update, and sync workflows
- explicit operator controls for unmanaged scopes
- any decision to move from instruction packages toward executable skill action
  contracts

## What The Current Code Actually Proves

### 1. External Skills Are Now Part Of The Discovery-First Runtime

The tool catalog no longer treats external skills as a provider-exposed static
surface.

`crates/app/src/tools/catalog.rs` defines:

- `tool.search` and `tool.invoke` as the only provider-core tools
- every `external_skills.*` action as `Discoverable`

That means LoongClaw has tightened the architecture compared with the earlier
managed-runtime draft. The platform did not simply add more directly callable
provider tools. Instead, it pushed external-skills lifecycle and invocation
behind the same discovery-first gateway used for other non-core actions.

This matters for both governance and product clarity:

- provider schema stays compact
- external-skills execution remains explicit
- future capability policy can continue to reason about one discovery path
  instead of many special cases

### 2. A Real Operator CLI Now Exists

The biggest factual gap in the closed PR `#160` is that it said LoongClaw still
lacked a first-class skills CLI. That is no longer true.

`crates/daemon/src/skills_cli.rs` and `crates/daemon/src/main.rs` now expose an
operator-facing surface for:

- `loongclaw skills list`
- `loongclaw skills info <skill-id>`
- `loongclaw skills install <path>`
- `loongclaw skills install-bundled <skill-id>`
- `loongclaw skills enable-browser-preview`
- `loongclaw skills remove <skill-id>`
- `loongclaw skills policy {get,set,reset}`

This is not a parallel implementation. The CLI wraps the same underlying
runtime lifecycle and policy tools, then renders both JSON and human-readable
operator output. It also exposes `shadowed_skills` in text mode, which is
important because multi-scope discovery is no longer invisible when duplicates
exist.

So one of the original product gaps has already been closed in code and should
not stay open in design documentation.

### 3. Managed Downloads And Installs Remain Strongly Governed

The managed path in `crates/app/src/tools/external_skills.rs` is still one of
LoongClaw's strongest differentiators.

The current managed lifecycle includes:

- explicit runtime enablement
- approval-gated fetch and policy mutation
- HTTPS-only download
- allowlist and blocklist domain policy
- redirect rejection
- byte caps on downloads
- deterministic managed install paths
- persisted `index.json` metadata
- rollback-safe replacement flows
- path-hardening around managed install inspection and removal

The security posture is now paired with an operator surface instead of staying
hidden inside core tools, which makes it more credible as a product behavior
rather than just an implementation detail.

### 4. Discovery Is Now Multi-Scope, But The Precedence Model Is Opinionated

The old draft described LoongClaw as lacking layered discovery. That is also no
longer accurate.

`crates/app/src/tools/external_skills.rs` now discovers skills from:

- managed installs
- user directories
- project directories

The current project and user probes deliberately recognize multiple ecosystem
conventions:

- `.agents/skills`
- `.codex/skills`
- `.claude/skills`
- `skills` (project scope only)

The same module and its tests also prove that:

- managed wins over user
- user wins over project
- duplicate lower-priority copies remain visible as `shadowed_skills`
- project discovery anchors to the config/file-root context instead of the
  caller's unrelated current directory
- within project scope, the nearest project ancestor wins for duplicate IDs

This is real multi-scope behavior, but it is not yet the repo-first precedence
model that many coding-agent systems use. That difference should be documented,
because it affects both operator expectations and future product decisions.

### 5. Skills Remain Instruction Packages With Narrow Metadata

LoongClaw has improved operator UX and discovery, but it still keeps skill
semantics intentionally narrow.

The current `SkillFrontmatter` parser reads only:

- `name`
- `description`

There is no first-class support yet for richer metadata such as:

- user-visible invocation policy
- allowed-tool restrictions
- model selection
- environment or binary prerequisites
- config prerequisites
- dependency declarations
- hooks or nested skill loading rules

That means LoongClaw currently supports multi-scope discovery without a broader
metadata contract. This keeps the system simple, but it also means LoongClaw is
still behind the richer platform semantics now exposed publicly by Claude Code
and Codex.

### 6. Invocation Still Loads Instructions Into The Conversation Loop

LoongClaw has not changed the core semantic model for skills: they are still
instruction packages, not self-describing executable tool plugins.

The current code path still proves that:

- `external_skills.invoke` resolves the winning skill by ID
- the response returns the full instruction body
- the turn engine preserves `external_skills.invoke` payloads intact instead of
  truncating them like generic large tool payloads
- follow-up conversation assembly can promote those instructions back into the
  next model round

This is a deliberate design choice. It keeps execution auditable and avoids
turning every skill into a dynamic function-schema surface. The tradeoff is
that skills still rely on existing tools to do work; they do not add new native
action contracts on their own.

### 7. Bundled First-Party Skills Are Now A Real Product Surface

The old managed-runtime story focused on generic installs. The current platform
has moved beyond that.

The daemon CLI and external-skills runtime now support bundled first-party
skills, most visibly through the browser-preview bootstrap flow. The
`enable-browser-preview` path:

- persists the relevant runtime config
- installs the bundled helper skill
- returns operator-facing next steps and recipes

This is an important product step because it shows LoongClaw is using the
skills runtime for first-party guided workflows, not only for arbitrary
third-party imports.

## Current Architectural Position

The most accurate description of LoongClaw's current position is:

- discovery-first provider architecture
- governed managed downloads and installs
- managed, user, and project scope resolution
- instruction-package skill semantics
- operator CLI and bundled first-party enablement

That is already a meaningful platform. It is not just a thin compatibility shim
anymore.

But it is still not yet a full metadata-rich or ecosystem-rich skills operating
system. The design-doc layer should be explicit about both sides of that
statement.

## Comparison With Reference Systems

### Claude Code

Claude Code's public docs now describe a skills system with:

- personal and nested project skill locations under `.claude/skills`
- automatic discovery from nested directories and additional directories
- YAML frontmatter for invocation policy, user visibility, allowed tools, and
  model selection
- subagents that can preload skills, declare tools and permission modes, and
  persist memory at user, project, or local scope

Compared with Claude Code, LoongClaw is now similar on:

- `SKILL.md`-centered instruction packaging
- local project and user discovery
- keeping full skill bodies out of context until invocation

Compared with Claude Code, LoongClaw is still behind on:

- nested repo-scope ergonomics
- metadata richness
- tool restrictions encoded in skill metadata
- subagent integration and scoped persistent memory around skills

LoongClaw is stricter on one important axis:

- managed remote downloads are explicitly gated by runtime policy and approval

### Codex

Codex's current public skills docs describe:

- progressive disclosure from metadata to full `SKILL.md`
- repository, user, admin, and system skill locations
- repo scanning from the current directory up to the repository root
- `$skill-installer`
- config-level enable/disable
- optional `agents/openai.yaml` metadata for UI, invocation policy, and tool
  dependencies

Compared with Codex, LoongClaw is now similar on:

- progressive-disclosure skill loading
- instruction-package skills with optional scripts/resources
- project and user skill discovery

Compared with Codex, LoongClaw is still behind on:

- scope breadth and admin/system distribution
- richer optional metadata
- built-in per-skill enable/disable
- packaged dependency declarations
- installer and distribution maturity

LoongClaw remains stronger on:

- managed download governance and approval requirements for remote fetch

### OpenClaw

OpenClaw's current public README describes:

- bundled, managed, and workspace skills
- install gating plus UI
- workspace skill paths
- ClawHub as a minimal registry that can search and pull skills automatically

Compared with OpenClaw, LoongClaw now matches more than it used to:

- managed install lifecycle
- bundled first-party skill flows
- a real operator CLI

But LoongClaw is still behind OpenClaw on:

- control-plane UX breadth
- registry and update story
- workspace-first product ergonomics
- ecosystem-facing lifecycle polish

### Nanobot

Nanobot's public README now shows:

- a dedicated `skills.py` loader in the agent layer
- bundled skills in the repository
- ClawHub-backed public skill search/install
- workspace-isolated multi-instance packaging

Compared with Nanobot, LoongClaw is stronger on the managed path in several
ways:

- persisted managed install index
- replacement rollback behavior
- explicit domain-policy and approval gating for remote fetch
- duplicate shadow reporting across discovered scopes

Nanobot's public surface is stronger on lightweight public-skill acquisition
and instance packaging simplicity, while LoongClaw's managed story is more
explicitly governed and operator-auditable.

## Remaining Gaps Worth Tracking

### 1. Metadata Needs To Grow Beyond `name` And `description`

The current parser is intentionally small, but it is now the clearest limiting
factor. LoongClaw needs a constrained metadata contract for at least:

- implicit vs explicit invocation
- user visibility
- tool restrictions
- model hints
- environment, binary, or config prerequisites

Without that, multi-scope discovery exists, but platform semantics remain thin.

### 2. Scope Breadth And Precedence Still Need Product-Level Review

Managed, user, and project scopes now exist, but the current ordering is
managed-first rather than repo-first. That may be right for safety, but it is a
product decision with real ergonomics tradeoffs and should be treated as such.

The broader scope model is also still narrower than Codex and Claude Code.

### 3. Local Scopes Need Better Operator Governance

Managed downloads are governed in detail. Local project and user scopes are
discoverable once external skills are enabled, but they do not yet have the
same install-registry, approval, activation, or replacement lifecycle as the
managed path.

That asymmetry is acceptable for now, but it should be called out explicitly.

### 4. Registry / Update / Sync Workflows Are Still Missing

LoongClaw still lacks its own native answer for:

- remote registry discovery
- version updates
- sync flows
- trust/pinning policy around registry-sourced skills

Bundled skills and managed install cover local operation well, but the
ecosystem story is still incomplete.

### 5. Skills And Native Action Contracts Are Still Separate

The current runtime is coherent because it keeps skills as instruction packages.
If LoongClaw ever wants skills to become executable action surfaces, that
should be a separate explicit architecture decision, not an accidental drift in
the current design-doc language.

## Recommended Positioning

The most defensible current product statement is:

LoongClaw `dev` now supports a governed, discovery-first external-skills
platform with managed installs, an operator CLI, bundled first-party helper
skills, and managed/user/project scope resolution. It is no longer accurate to
describe the product as a migration-only or managed-install-only skills story.
At the same time, LoongClaw still lacks the metadata depth, broader scope
surface, and registry/update workflow maturity seen in the strongest reference
systems.

That framing gives LoongClaw credit for what the code already proves while
keeping the remaining gaps precise and believable.

## Primary Evidence Files

Core runtime and discovery:

- `crates/app/src/tools/catalog.rs`
- `crates/app/src/tools/mod.rs`
- `crates/app/src/tools/external_skills.rs`

Conversation-loop integration:

- `crates/app/src/conversation/tests.rs`
- `docs/design-docs/discovery-first-tool-runtime-contract.md`

Operator surface:

- `crates/daemon/src/main.rs`
- `crates/daemon/src/skills_cli.rs`
- `crates/daemon/tests/integration/skills_cli.rs`

Historical closure baseline:

- `docs/plans/2026-03-12-external-skills-runtime-closure.md`

## Reference System Docs

- [Codex Agent Skills](https://developers.openai.com/codex/skills)
- [Claude Code Slash Commands / Skills](https://code.claude.com/docs/en/slash-commands)
- [Claude Code Subagents](https://code.claude.com/docs/en/sub-agents)
- [OpenClaw README](https://github.com/openclaw/openclaw)
- [Nanobot README](https://github.com/HKUDS/nanobot)
