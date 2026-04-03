# Bundled Skill Pack Registry Design

## Goal

Promote bundled skill packs from onboarding-only aliases into a first-class app
registry that can power onboarding, manual installation, and CLI discovery from
one source of truth.

## Problem

The current bundled skill model has three disconnected layers:

- bundled skill assets live in `crates/app/src/tools/bundled_skills.rs`
- onboarding pack aliases live in `crates/daemon/src/onboard_cli.rs`
- `skills list/info` only understands individual skills, not pack membership

This works for a few packs, but it will drift as more bundled families are
added. The onboarding list, install behavior, and CLI visibility can diverge.

## Decision

Create a formal bundled pack registry in the app layer.

The registry should describe:

- pack id
- pack label
- pack summary
- member bundled skill ids
- whether the pack is onboarding-visible
- whether the pack is recommended during onboarding

The daemon layer should consume that registry instead of re-declaring pack
membership. `skills install-bundled` and `skills info` should also understand
pack ids, not just skill ids.

## Scope

In scope:

- first-class bundled pack metadata in `crates/app`
- onboarding preinstall choices derived from app-layer registry
- CLI `skills install-bundled <pack-id>` support
- CLI `skills info <pack-id>` support
- pack membership visibility for skill-level operator inspection and summary

Out of scope:

- exposing packs through the model-facing `external_skills.list` surface
- changing the installed external skill index schema
- remote/downloaded pack formats

## Design

### 1. App-layer bundled registry

Keep bundled skills and packs in the same module so the relationships stay
close to the packaged assets. A bundled skill remains the installable primitive.
A bundled pack is metadata over a stable set of bundled skill ids.

### 2. Onboarding consumption

Onboarding should derive its preinstall choice list from app-layer metadata.
Single bundled skills that remain first-run friendly can still be exposed
individually, but pack-level entries should come from the same registry rather
than daemon-local arrays.

### 3. CLI pack support

`skills install-bundled` should accept either a bundled skill id or a bundled
pack id. For a pack, the daemon can iterate over member skill ids and install
them into the managed runtime. `skills info` should resolve a pack id into a
pack-specific payload that lists members and onboarding visibility.

### 4. Skill-level pack membership

Operator-facing skill list and inspect payloads should annotate each bundled
skill with the packs it belongs to. This preserves discoverability without
requiring packs to masquerade as actual installed skills.

## Validation

Required evidence:

- app tests for pack registry membership and pack lookup
- daemon tests for onboarding visibility driven by the registry
- daemon tests for `skills install-bundled <pack-id>` and `skills info <pack-id>`
- existing bundled install and onboarding regression tests stay green
