# Preinstalled Skills Onboarding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship curated bundled skills that can be selected during `loongclaw onboard` and are installed into the managed external-skills runtime before onboarding finishes.

**Architecture:** Extend bundled-skill packaging from `SKILL.md`-only assets to vendored skill directories, add onboarding metadata for curated bundled skills, and wire onboarding to persist external-skills config plus install the selected bundled skills after config write. Keep the browser-preview fast path working on top of the same bundled installation mechanism.

**Tech Stack:** Rust, clap/dialoguer onboarding flow, LoongClaw external-skills runtime, repository-vendored skill assets.

---

### Task 1: Add failing tests for bundled multi-file skill installs

**Files:**
- Modify: `crates/app/src/tools/external_skills.rs`
- Modify: `crates/app/src/tools/bundled_skills.rs`
- Test: `crates/app/src/tools/external_skills.rs`

**Step 1: Write the failing test**

Add an app test that installs a bundled skill expected to contain at least one
extra packaged file beyond `SKILL.md`, then asserts the managed install copy
includes both `SKILL.md` and the extra file.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app install_from_bundled_skill -- --nocapture`

Expected: FAIL because bundled installs currently only write `SKILL.md`.

**Step 3: Write minimal implementation**

- change bundled skill metadata to point at a source directory
- copy the bundled directory into the incoming managed install root
- keep the existing `source_kind=bundled` contract stable

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app install_from_bundled_skill -- --nocapture`

Expected: PASS

### Task 2: Define curated bundled skill inventory and remove the stray artifact

**Files:**
- Delete: `skills/update-harness.skill`
- Create or Modify: curated bundled skill directories under `skills/`
- Modify: `crates/app/src/tools/bundled_skills.rs`
- Test: `crates/app/src/tools/bundled_skills.rs` or existing app tests that cover bundled inventory

**Step 1: Write the failing test**

Add tests that assert the bundled inventory contains the curated skill ids
required for onboarding and no longer references the stray `update-harness`
artifact.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app bundled -- --nocapture`

Expected: FAIL because only `browser-companion-preview` is bundled today.

**Step 3: Write minimal implementation**

- vendor the curated bundled skill directories under `skills/`
- register the curated bundled skill ids and onboarding metadata
- delete `skills/update-harness.skill`

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app bundled -- --nocapture`

Expected: PASS

### Task 3: Add failing onboarding tests for preinstalled skill selection

**Files:**
- Modify: `crates/daemon/tests/integration/onboard_cli.rs`
- Modify: `crates/daemon/src/onboard_cli.rs`
- Modify: `crates/daemon/src/onboard_finalize.rs`

**Step 1: Write the failing tests**

Add daemon tests that prove:

- onboarding can resolve a selected bundled skill list into config/runtime state
- selecting bundled skills enables `external_skills.enabled`
- selecting bundled skills enables `external_skills.auto_expose_installed`
- onboarding installs the selected bundled skills into the managed install root

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon onboard bundled -- --nocapture`

Expected: FAIL because onboarding does not currently model or install bundled
skills.

**Step 3: Write minimal implementation**

- add onboarding state for selected bundled skills
- add a new onboarding step for curated bundled skill selection
- persist an explicit managed install root derived from the config output path
- install selected bundled skills after config write and before success output

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon onboard bundled -- --nocapture`

Expected: PASS

### Task 4: Keep browser preview and success summary behavior coherent

**Files:**
- Modify: `crates/daemon/src/browser_preview.rs`
- Modify: `crates/daemon/src/onboard_finalize.rs`
- Modify: `crates/daemon/tests/integration/onboard_cli.rs`

**Step 1: Write the failing test**

Add or update tests that prove:

- browser preview still uses the shared bundled install lane
- onboarding success output remains coherent when bundled skills were selected

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon browser_preview onboarding_success_summary -- --nocapture`

Expected: FAIL if the new bundled install model breaks old assumptions.

**Step 3: Write minimal implementation**

- keep `skills enable-browser-preview` routed through the same bundled install
  path
- add only the smallest success-summary surface needed to confirm installed
  bundled skills

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon browser_preview onboarding_success_summary -- --nocapture`

Expected: PASS

### Task 5: Final targeted verification

**Files:**
- Modify only what the previous tasks require

**Step 1: Run app verification**

Run: `cargo test -p loongclaw-app bundled external_skills -- --nocapture`

Expected: PASS

**Step 2: Run daemon verification**

Run: `cargo test -p loongclaw-daemon onboard browser_preview -- --nocapture`

Expected: PASS

**Step 3: Run focused lint or formatting if needed**

Run: `cargo fmt --all -- --check`

Expected: PASS

**Step 4: Commit**

```bash
git add skills docs/plans crates/app/src/tools crates/daemon/src crates/daemon/tests/integration
git commit -m "feat: bundle preinstalled skills into onboarding"
```
