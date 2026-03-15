# External Skills Platform Assessment

## Scope and Baseline

- Audited branch: `origin/alpha-test`
- Audited commit: `a251bf6` (`fix(app): preserve safe-lane session tool execution without kernel`)
- Assessment date: 2026-03-15

This document reassesses LoongClaw's external-skills support after the
managed-runtime loop landed on `alpha-test`.

The central question is no longer "does LoongClaw support external skills at
all?" The answer is now clearly yes. The harder question is what kind of skill
system LoongClaw has actually built:

- a migration-only shim
- a managed instruction-package runtime
- or a full multi-scope skills platform comparable to Claude Code, Codex, or
  OpenClaw

The code now proves that LoongClaw has moved decisively into the second
category.

## Executive Summary

LoongClaw `alpha-test` now has a real managed external-skills runtime:

- runtime tool catalog includes `external_skills.fetch`, `install`, `list`,
  `inspect`, `invoke`, `remove`, and `policy`
- downloads are guarded by explicit enablement, approval, HTTPS-only fetch,
  allowlist/blocklist policy, and redirect rejection
- installs write a managed on-disk index and normalize skill roots under a
  deterministic install directory
- installed skills can be surfaced to the model through the capability snapshot
- `external_skills.invoke` feeds `SKILL.md` instructions back into the
  conversation loop as system-context guidance
- migration still detects Codex / Claude / generic skills artifacts and writes
  an audit manifest, but intentionally does not auto-install them

That is materially stronger than the older "download-only plus migration-note"
state.

The remaining gap is different now:

- LoongClaw still lacks a first-class operator/product surface for skills
- it does not yet offer multi-scope discovery and precedence
- it does not yet expose per-skill config/env gating like OpenClaw
- it does not yet provide a registry, sync/update workflow, or ecosystem
  marketplace path
- skills remain instruction packages, not executable action surfaces or dynamic
  plugin tools

So the right current rating is:

- `3.5 / 5` as a managed instruction-package external-skills runtime
- `2 / 5` as a broader productized skills platform

## Status Versus The 2026-03-12 Runtime-Closure Design

The design captured in
`docs/plans/2026-03-12-external-skills-runtime-closure-design.md` is now
substantially implemented on `alpha-test`.

Implemented from that design:

- managed install root and persisted index
- install/list/inspect/invoke/remove lifecycle tools
- capability snapshot disclosure for installed skills
- conversation-loop promotion of invoked skill instructions
- guarded fetch policy and approval controls

Still not closed from a broader product perspective:

- operator-facing `loongclaw skills` CLI
- multi-scope discovery and precedence
- rich per-skill metadata and eligibility filters
- migration-to-install bridge
- registry / update / ecosystem workflows

## What The Code Actually Proves

### 1. LoongClaw Now Exposes A Real External-Skills Runtime Surface

`crates/app/src/tools/catalog.rs` registers all seven external-skills tools as
runtime-available core tools:

- `external_skills.fetch`
- `external_skills.inspect`
- `external_skills.install`
- `external_skills.invoke`
- `external_skills.list`
- `external_skills.policy`
- `external_skills.remove`

`crates/app/src/tools/mod.rs` then routes those names through the normal core
tool dispatcher rather than leaving them as planned or stubbed entries.

This matters because the feature is no longer a documentation claim or a
sidecar experiment. The main runtime advertises and dispatches these tools.

### 2. The Runtime Is Policy-Governed, Not Just Convenience-Oriented

`crates/app/src/tools/external_skills.rs` implements a proper policy layer:

- runtime enable/disable gating
- approval-gated policy mutation
- approval-gated downloads
- HTTPS-only fetch
- allowlist/blocklist domain enforcement
- redirect rejection
- max download byte caps

That safety posture is stricter than many "just download a prompt pack" skill
systems and is one of LoongClaw's strongest differentiators.

The design intent is also reflected in the default config:

- `external_skills.enabled = false`
- `require_download_approval = true`
- `auto_expose_installed = true`

This means LoongClaw treats external skills as a governed extension surface,
not as ambient prompt state.

### 3. Installation Is Managed And Indexed

The install path is no longer ad hoc.

`external_skills.install`:

- resolves the input path through the safe file-root policy
- rejects symlink sources
- accepts either a directory or a local `.tgz` / `.tar.gz`
- extracts archives into staging
- locates exactly one installable skill root containing `SKILL.md`
- derives or validates a normalized `skill_id`
- copies the skill into a managed install root
- writes a persisted `index.json`
- records digest, source kind/path, install path, install time, and active state
- uses backup/rollback logic when replacing an existing install

This is a meaningful product step up from simple "put files somewhere and hope
the model sees them".

### 4. Installed Skills Are Discoverable By The Model

`crates/app/src/tools/mod.rs` now extends the capability snapshot with an
`[available_external_skills]` block when external-skills tooling is present and
auto-exposure is enabled.

`crates/app/src/tools/external_skills.rs` builds those lines from the managed
install index, filtering to active entries.

This gives the model deterministic awareness that installed skills exist without
loading every `SKILL.md` body into context up front.

That is much closer to Codex and Nanobot's progressive-disclosure model than to
a monolithic prompt append.

### 5. Invocation Is Wired Into The Conversation Loop

The most important runtime closure is not installation; it is the loop between
"the model sees a skill" and "the model can actually use it".

LoongClaw now has that closure:

- `external_skills.invoke` returns the resolved `SKILL.md` instructions
- `turn_engine` preserves a large enough payload summary budget for that tool
- `turn_shared` parses successful `external_skills.invoke` results and extracts
  the untruncated instruction body
- the follow-up message builder promotes those instructions into system context
  for subsequent turns

This is the key proof that skills are not just stored metadata. They can change
the next model round in a controlled and explicit way.

### 6. Migration Compatibility Is Still Separate From Runtime Installation

Migration support remains strong, but its scope is intentionally narrower than
installation:

- detect `SKILLS.md`
- detect `skills-lock.json`
- detect `.codex/skills`
- detect `.claude/skills`
- detect `skills/`
- collect declared/locked/resolved skill ids
- persist an external-skills manifest during apply
- append a profile-note addendum once

The migration warning is explicit: imported skills are not auto-installed into
runtime. Operators must still use `fetch -> install -> list -> invoke` to make
them available in chat.

That split is now a design choice, not an unfinished accident.

## Current Architectural Position

LoongClaw's current external-skills model is:

- static runtime tool catalog
- dynamic managed skill inventory
- explicit invocation boundary
- instruction-package execution model

This is important because it defines both LoongClaw's strengths and its limits.

### Strengths Of This Model

- fits the existing static tool architecture cleanly
- keeps auditability and policy enforcement straightforward
- avoids dynamic per-skill provider schema generation
- avoids silently mounting arbitrary prompt content
- makes installation and invocation explicit operator-visible state

### Limits Of This Model

- a skill is not its own callable action surface
- there is no per-skill executable contract beyond `SKILL.md`
- there is no first-class layered discovery model
- there is no built-in operator CLI for inspecting/managing skills outside the
  agent/tool loop

In other words, LoongClaw has productized "managed instruction packages", not a
full "skills operating system".

## Comparison With Reference Systems

### Claude Code

Claude Code currently provides:

- multiple scopes with clear precedence: enterprise, personal, project, plugin
- automatic nested directory discovery
- live change detection in additional directories
- YAML frontmatter with invocation control, allowed-tools, model, forked-agent
  execution, hooks, and argument substitution
- plugin marketplace and managed settings integration

Compared with Claude Code, LoongClaw is behind on:

- scope model and precedence
- nested discovery and hot reload
- rich frontmatter semantics
- plugin marketplace / organizational distribution
- slash-command style user-facing ergonomics

Compared with Claude Code, LoongClaw is ahead or stricter on:

- explicit runtime safety posture for downloaded third-party skills
- approval-gated domain policy at the tool layer

### Codex

Codex currently provides:

- repository, user, admin, and system skill locations
- progressive disclosure by loading metadata first and `SKILL.md` on demand
- built-in `$skill-installer`
- config-level enable/disable of specific skills
- optional `agents/openai.yaml` metadata for UI, dependency, and invocation
  policy

Compared with Codex, LoongClaw is now similar on:

- progressive-disclosure style invocation model
- treating skills as instruction packages rather than dynamic function tools

Compared with Codex, LoongClaw is behind on:

- multi-scope discovery
- first-class installer UX
- per-skill config toggles
- optional metadata contract
- system/admin distribution model

### OpenClaw

OpenClaw remains the strongest direct comparison because it treats skills as a
full product surface rather than only an agent convenience:

- bundled, managed, workspace, and extra-dir scopes
- documented precedence rules
- per-skill gating using metadata and environment/config/binary requirements
- watcher-based refresh
- `openclaw skills` operator CLI
- ClawHub install/update/sync ecosystem
- plugin-shipped skills and shared multi-agent semantics

Compared with OpenClaw, LoongClaw is behind on almost every operator-facing and
ecosystem-facing dimension:

- discovery scopes
- precedence
- eligibility checks
- per-skill env/config overrides
- operator CLI
- registry / sync / update lifecycle
- plugin packaging

LoongClaw is closer to OpenClaw only on one narrow axis:

- explicit install/index/invoke lifecycle now exists in core runtime instead of
  being purely conceptual

### Nanobot

Nanobot currently appears to use:

- bundled skills under `nanobot/skills/`
- workspace skills under `workspace/skills/`
- `SkillsLoader`-driven discovery
- progressive disclosure
- skill summaries always visible to the model
- requirement gating by bins/env

DeepWiki and public repository evidence suggest Nanobot still leans more
heavily on filesystem discovery plus prompt-loading than on a managed install
registry. Public issues also show skill availability can still be sensitive to
environment gating and whether the model actually reads the skill file.

Compared with Nanobot, LoongClaw is now stronger on:

- explicit managed install index
- explicit install/remove lifecycle
- explicit download governance

Compared with Nanobot, LoongClaw is still behind on:

- built-in workspace-scoped discovery model
- requirement-gated eligibility metadata
- default packaged skill ecosystem

## Remaining Gaps

### 1. No First-Class Operator CLI

There is still no `loongclaw skills ...` command family.

`crates/daemon/src/main.rs` exposes onboarding and import flows, but not a
dedicated skills management surface. Today the managed lifecycle exists only as
agent-callable tools and as import-side mapping support.

That makes the feature harder to operate, debug, and teach.

### 2. No Layered Discovery Or Precedence Model

The current runtime model revolves around one managed install root, not a full
scope hierarchy.

LoongClaw does not yet have first-class equivalents of:

- repo skills
- user skills
- admin/system skills
- extra skill directories
- workspace overrides
- nested discovery

This is the biggest product gap versus Claude Code, Codex, and OpenClaw.

### 3. No Per-Skill Eligibility Metadata

OpenClaw and Nanobot both treat skill availability as conditional on environment
and config requirements. LoongClaw's current managed runtime does not interpret
frontmatter for:

- required binaries
- required environment variables
- config prerequisites
- OS gating
- user-invocable vs model-invocable policy
- per-skill tool restrictions

Today, availability is basically:

- runtime policy enabled
- skill installed
- skill active

That is simpler, but much less expressive.

### 4. Skills Are Not Executable Action Contracts

`external_skills.invoke` only loads instructions. It does not dispatch
skill-defined actions.

This is visible in two ways:

- provider tool schema for `external_skills.invoke` only accepts `skill_id`
- provider parsing tests still allow arbitrary extra arguments like
  `action=get_states`, but the runtime ignores those fields

So LoongClaw skills can teach the model how to use existing tools, but they do
not yet become action-addressable runtime extensions.

That is a valid design choice, but it should be stated clearly.

### 5. Migration Still Stops Short Of Installation

LoongClaw can detect and audit external skills from other ecosystems, but the
import path still ends at:

- profile-note addendum
- external-skills audit manifest

There is no direct bridge from a migrated skill artifact to a managed install
entry.

This keeps migration safe, but it leaves user work unfinished after import.

### 6. Kernel Plugin Pipeline Is Still Parallel, Not Unified

The `kernel` + `spec` path still contains a richer plugin scan / translate /
activate / bootstrap pipeline, but that machinery is not the same thing as the
external-skills runtime.

That means LoongClaw currently has two extension stories:

- managed external skill instruction packages in app runtime
- plugin scanning and bootstrap in spec/kernel flows

Those stories are conceptually adjacent, but not yet unified into one extension
platform.

## Recommended Product Direction

LoongClaw should not immediately jump to dynamic per-skill native tools.

The better next move is to finish the product surface around the architecture it
already chose.

### Phase 1: Productize The Existing Managed Runtime

Build a first-class operator surface around the runtime that already exists:

- add `loongclaw skills list`
- add `loongclaw skills info <skill-id>`
- add `loongclaw skills install <path>`
- add `loongclaw skills remove <skill-id>`
- add `loongclaw skills policy`

This should be a thin daemon/CLI wrapper over the existing core tools, not a
parallel implementation.

### Phase 2: Add Layered Discovery And Precedence

Introduce explicit skill scopes:

- project/repo
- user
- managed/system
- managed install root

Then define deterministic precedence rules and snapshot rendering.

This is the highest-leverage step if the goal is to compete with Codex,
Claude Code, or OpenClaw ergonomics.

### Phase 3: Add Per-Skill Metadata And Eligibility

Support a constrained subset of skill metadata:

- implicit-invocation allow/deny
- user-invocable flag
- required bins/env/config
- optional display name/homepage

This would let LoongClaw keep the managed-runtime architecture while gaining a
much more expressive discovery layer.

### Phase 4: Close The Migration-To-Install Gap

After the operator CLI and layered discovery exist, add an explicit opt-in
bridge from imported skill artifacts to managed installs.

The key word is opt-in. Auto-install should remain off by default.

### Phase 5: Decide Whether Skills And Plugins Should Converge

Only after the above steps should LoongClaw decide whether some skills should
graduate into richer runtime extensions backed by kernel plugin/bootstrap
machinery.

That future is plausible, but it is a separate architectural decision from the
current managed instruction-package system.

## Recommended Positioning

The most accurate current product statement is:

> LoongClaw `alpha-test` now supports governed external-skill download,
> installation, discovery, inspection, invocation, and removal as managed
> instruction packages, but it does not yet provide the full multi-scope,
> metadata-rich, operator-first skills platform seen in Claude Code, Codex, or
> OpenClaw.

That framing is important because it is both stronger and more defensible than
the earlier "partial skills support" story.

It gives LoongClaw credit for what is already real without pretending the
ecosystem and product-management layers are already complete.

## Primary Evidence Files

Core runtime and config:

- `crates/app/src/tools/catalog.rs`
- `crates/app/src/tools/mod.rs`
- `crates/app/src/tools/external_skills.rs`
- `crates/app/src/tools/runtime_config.rs`
- `crates/app/src/config/tools_memory.rs`
- `crates/app/src/context.rs`

Conversation/runtime closure:

- `crates/app/src/conversation/turn_engine.rs`
- `crates/app/src/conversation/turn_shared.rs`
- `crates/app/src/conversation/turn_loop.rs`

Migration/import path:

- `crates/app/src/migration/mod.rs`
- `crates/app/src/migration/orchestrator.rs`
- `crates/daemon/src/import_claw_cli.rs`

Parallel extension substrate still not unified with external skills:

- `crates/kernel/src/plugin.rs`
- `crates/kernel/src/plugin_ir.rs`
- `crates/kernel/src/bootstrap.rs`
- `crates/spec/src/spec_execution.rs`
