# Preinstalled Skills Onboarding Design

## Goal

Let LoongClaw ship a curated first-party bundled skill set that operators can
select directly during `loongclaw onboard`, with the selected skills installed
into the managed external-skills runtime before onboarding completes.

## Problem

Today the repository only bundles one helper skill,
`browser-companion-preview`, and `onboard` does not let operators choose or
install bundled skills. There is also a stray `skills/update-harness.skill`
artifact in the repository root that is not part of the product surface.

The requested bundled skills include upstream packages that are larger than a
single `SKILL.md` file. Some depend on additional references or scripts, so the
current bundled-skill model, which writes only one in-memory `SKILL.md` into the
managed runtime, is too narrow.

## Decision

Implement bundled skills as packaged directories, not just embedded markdown
strings.

This design keeps onboarding deterministic and offline-friendly:

- LoongClaw vendors the curated skill content into the repository under
  `skills/`.
- `external_skills.install` can install a bundled skill by copying the bundled
  skill directory into the managed runtime.
- `onboard` exposes a new optional "preinstalled skills" step that lets the
  operator select zero or more bundled skills.
- When the operator selects at least one bundled skill, onboarding enables the
  external-skills runtime, enables installed-skill auto exposure, sets a stable
  managed install root next to the config file, writes the config, then installs
  the selected bundled skills into that managed runtime before success is
  reported.

## Why This Approach

### Recommended: vendor bundled skill directories and install them locally

Pros:

- no network dependency during onboarding
- exact repository-controlled skill content and reviewability
- supports multi-file skills without inventing fetch-time policy exceptions
- matches the user's expectation of "preinstalled"

Cons:

- repository grows by the vendored skill content
- bundled skill inventory needs explicit maintenance

### Rejected: fetch remote skills during onboarding

This would make onboarding depend on runtime network access, remote stability,
and upstream packaging format. It would also blur the line between "bundled" and
"downloaded later", which is exactly the distinction the operator asked to make.

### Rejected: keep bundled skills as `SKILL.md`-only assets

This does not scale to the requested upstream skills if they rely on
`references/`, `scripts/`, or other packaged files.

## Scope

In scope:

- remove the stray `skills/update-harness.skill` artifact
- expand bundled-skill packaging to support whole skill directories
- add a curated bundled-skill catalog for onboarding
- add onboarding UI and non-interactive support for selecting bundled skills
- install selected bundled skills during onboarding completion
- add tests for bundled installation and onboarding persistence

Out of scope:

- automatic syncing from upstream repositories
- marketplace trust or signature verification changes
- generic multi-select widgets for all onboarding screens

## Product Behavior

Interactive onboarding:

- shows an optional preinstalled-skills screen after core setup choices
- offers curated bundled skills with concise descriptions
- allows skipping skill installation entirely
- writes the config and installs selected skills before success output

Non-interactive onboarding:

- should preserve current behavior unless an explicit CLI option is provided for
  preinstalled bundled skills
- if no bundled-skill option is passed, no bundled skills are installed

Success summary:

- should keep the existing next-action structure
- may include a compact saved-setup line that confirms bundled skills were
  installed

## Data Model

Bundled skill metadata should include:

- stable skill id
- bundled source path
- bundled repository directory
- display label / summary for onboarding
- onboarding default recommendation flag

The bundled install path should copy the entire source directory into the
managed install root, preserving `SKILL.md` plus any additional packaged files.

## Failure Handling

If bundled skill installation fails after config write:

- onboarding should fail the overall flow
- the config write should roll back when possible
- any partially installed managed skill directory should be removed by the
  existing external-skills install rollback path or by onboarding cleanup

## Verification

Required evidence:

- app-level tests proving bundled directory installs copy more than `SKILL.md`
- daemon tests proving onboarding persists external-skills runtime settings when
  bundled skills are selected
- daemon tests proving onboarding installs the selected bundled skills into the
  managed install root
- targeted onboarding baseline remains green
