# Bundled Skill Pack Registry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make bundled skill packs a first-class app-layer registry that powers onboarding, `skills install-bundled`, and `skills info` consistently.

**Architecture:** Extend `bundled_skills.rs` with pack metadata and helper queries, route onboarding to consume app-layer preinstall targets, and teach daemon `skills_cli` how to install and inspect pack ids. Keep individual bundled skills as the installable primitives while annotating skill-level operator output with pack memberships.

**Tech Stack:** Rust, bundled asset registry in `crates/app`, daemon CLI rendering, external-skills operator payloads.

---

### Task 1: Add failing tests for the pack registry contract

**Files:**
- Modify: `crates/app/src/tools/bundled_skills.rs`
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/app/src/tools/bundled_skills.rs`
- Test: `crates/daemon/src/onboard_cli.rs`

**Step 1: Write the failing test**

Add tests that prove:

- bundled pack ids can be looked up centrally
- skill membership can be derived from the pack registry
- onboarding pack visibility is derived from that registry

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app bundled` and `cargo test -p loongclaw-daemon preinstalled_skills_screen_only_surfaces_the_onboarding_subset -- --nocapture`

Expected: FAIL because onboarding and bundled skills still derive pack state from separate constants.

**Step 3: Write minimal implementation**

- add `BundledSkillPack`
- add lookup and membership helpers
- route onboarding choice construction through the new helpers

**Step 4: Run test to verify it passes**

Run the same test commands.

Expected: PASS

### Task 2: Add failing daemon tests for pack-aware CLI install and inspect

**Files:**
- Modify: `crates/daemon/tests/integration/skills_cli.rs`
- Modify: `crates/daemon/src/skills_cli.rs`

**Step 1: Write the failing test**

Add tests that prove:

- `skills install-bundled anthropic-office` installs all pack members
- `skills info anthropic-office` returns a pack-level payload with member ids
- skill-level `skills info` exposes pack membership for bundled members

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon skills_cli -- --nocapture`

Expected: FAIL because the daemon currently treats bundled ids as skill ids only.

**Step 3: Write minimal implementation**

- resolve pack ids in `skills install-bundled`
- resolve pack ids in `skills info`
- extend text rendering for pack-level info and skill-level memberships

**Step 4: Run test to verify it passes**

Run the same test command.

Expected: PASS

### Task 3: Annotate operator-facing skill payloads with pack membership

**Files:**
- Modify: `crates/app/src/tools/external_skills.rs`
- Modify: `crates/daemon/src/skills_cli.rs`
- Test: `crates/app/src/tools/external_skills.rs`

**Step 1: Write the failing test**

Add operator-surface tests proving bundled skills such as `docx` or
`minimax-docx` expose pack membership metadata.

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app external_skills -- --nocapture`

Expected: FAIL because operator payloads currently know nothing about pack
membership.

**Step 3: Write minimal implementation**

- annotate operator list/inspect serialization with pack membership objects
- keep the model-facing surface unchanged
- teach CLI summary rendering to show pack relationships compactly

**Step 4: Run test to verify it passes**

Run the same command.

Expected: PASS

### Task 4: Final verification

**Files:**
- Modify only what previous tasks require

**Step 1: Formatting**

Run: `cargo fmt --all -- --check`

Expected: PASS

**Step 2: App verification**

Run: `cargo test -p loongclaw-app install_from_bundled_skill -- --nocapture`

Expected: PASS

**Step 3: Daemon verification**

Run: `cargo test -p loongclaw-daemon onboard -- --nocapture`

Expected: PASS

**Step 4: Lint**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: PASS
