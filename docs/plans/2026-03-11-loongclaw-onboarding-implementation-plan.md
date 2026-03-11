# LoongClaw Onboarding Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rework `loongclawd onboard` into a guided, width-aware first-run flow that reaches a successful first chat with clearer safety, provider, credential, model, and handoff UX.

**Architecture:** Keep the implementation in the existing line-oriented CLI architecture instead of introducing a heavy TUI framework. Add small rendering helpers for banner selection, footer hints, stateful menus, grouped summaries, and development build labeling, then connect onboarding success directly to improved chat startup messaging.

**Tech Stack:** Rust, Clap, existing `loongclaw-app` config/provider helpers, daemon unit tests, inline app unit tests, optional lightweight terminal-width/build-metadata helpers.

---

### Task 1: Introduce Onboarding Presentation Primitives

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`
- Optional Modify: `crates/daemon/Cargo.toml`
- Optional Modify: `Cargo.toml`

**Step 1: Write the failing tests for presentation helpers**

Add daemon tests that assert:

- development label formatting produces `vX.Y.Z · branch · sha`
- release label formatting produces only `vX.Y.Z`
- banner selection chooses wide, split, and plain variants by width
- footer hints collapse by width

Suggested test names:

```rust
#[test]
fn format_build_label_for_release_build() {}

#[test]
fn format_build_label_for_dev_build() {}

#[test]
fn select_banner_variant_uses_split_logo_for_medium_width() {}

#[test]
fn footer_hints_collapse_for_narrow_width() {}
```

**Step 2: Run the daemon tests to confirm they fail**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: new tests fail because the helper functions do not exist yet.

**Step 3: Add minimal presentation helpers**

Implement small helpers in `crates/daemon/src/onboard_cli.rs` for:

- terminal width detection or width injection for testability
- build label formatting
- banner variant selection
- footer hint rendering
- plain text fallback when color or Unicode is unavailable

Prefer isolated pure functions so they are easy to unit test.

**Step 4: Re-run the daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: the new presentation tests pass.

**Step 5: Commit**

Run:

```bash
git add crates/daemon/src/onboard_cli.rs crates/daemon/src/tests/onboard_cli.rs crates/daemon/Cargo.toml Cargo.toml
git commit -m "feat: add onboarding presentation helpers"
```

Expected: one focused commit for width-aware presentation primitives.

---

### Task 2: Replace Free-Form Provider Entry with Stateful Provider Selection

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`

**Step 1: Write the failing tests for provider grouping and readiness**

Add tests that assert:

- providers with detected env vars are promoted into `Ready now`
- `Ollama` is marked as local
- supported providers are rendered in product-defined order instead of raw enum order
- provider detail rows include status labels

Suggested test names:

```rust
#[test]
fn provider_menu_promotes_ready_providers() {}

#[test]
fn provider_menu_marks_ollama_as_local() {}

#[test]
fn provider_menu_uses_product_display_order() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-daemon provider_menu -- --nocapture
```

Expected: failures because the grouping and render helpers are not implemented.

**Step 3: Implement a provider menu model**

Add internal structs or enums in `crates/daemon/src/onboard_cli.rs` for:

- provider display section
- provider readiness state
- provider menu item
- provider detail text

Update onboarding flow to use a numbered or cursor-based selection model instead of `prompt_with_default("Provider", ...)`.

**Step 4: Re-run daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: provider grouping tests pass and older parsing tests continue to pass.

**Step 5: Commit**

Run:

```bash
git add crates/daemon/src/onboard_cli.rs crates/daemon/src/tests/onboard_cli.rs
git commit -m "feat: add guided provider selection for onboarding"
```

Expected: provider selection is now structured and test-covered.

---

### Task 3: Reorder Credential and Model Flow with Recovery Paths

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`

**Step 1: Write failing tests for first-run recovery behavior**

Add tests that assert:

- credential checks happen before model selection
- missing credentials produce recoverable user-facing state
- model probe failures can fall back to default model
- skipped model probe is classified as optional, not fatal

Suggested test names:

```rust
#[tokio::test]
async fn preflight_treats_missing_credentials_as_attention() {}

#[tokio::test]
async fn preflight_allows_default_model_after_probe_failure() {}

#[test]
fn onboarding_step_order_places_credentials_before_models() {}
```

**Step 2: Run the targeted tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: failures that show step order and preflight semantics need rework.

**Step 3: Implement the reordered flow**

Update `run_onboard_cli` and helper functions to:

- prompt for provider first
- resolve or confirm credential state second
- defer model selection until credentials are ready or a fallback route is chosen
- support model fallback when probe fails
- support a `save and finish later` exit path without misreporting success

**Step 4: Re-run daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: reordered-flow and fallback tests pass.

**Step 5: Commit**

Run:

```bash
git add crates/daemon/src/onboard_cli.rs crates/daemon/src/tests/onboard_cli.rs
git commit -m "feat: add recoverable credential and model onboarding flow"
```

Expected: one focused commit that improves first-run success rate.

---

### Task 4: Replace Flat Preflight Output with Product-Level Summary Screens

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`

**Step 1: Write failing tests for grouped preflight output**

Add tests that assert preflight items are classified into:

- `Ready`
- `Needs attention`
- `Optional`

Suggested test names:

```rust
#[test]
fn group_preflight_checks_separates_ready_attention_and_optional() {}

#[test]
fn missing_credentials_are_grouped_as_attention() {}

#[test]
fn skipped_model_probe_is_grouped_as_optional() {}
```

**Step 2: Run the daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: failures because grouped summary helpers are not implemented.

**Step 3: Implement grouped preflight rendering**

Refactor or extend:

- `OnboardCheck`
- `run_preflight_checks`
- `print_preflight_checks`

So the user-facing output summarizes readiness by group and pairs blocking items with next actions.

Add an explicit summary helper rather than overloading the current flat table formatter.

**Step 4: Re-run daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: preflight grouping tests pass and legacy tests remain green.

**Step 5: Commit**

Run:

```bash
git add crates/daemon/src/onboard_cli.rs crates/daemon/src/tests/onboard_cli.rs
git commit -m "feat: add grouped onboarding preflight summary"
```

Expected: onboarding diagnostics now read like product guidance instead of raw logs.

---

### Task 5: Make Existing Config Handling Safe and Reviewable

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`

**Step 1: Write failing tests for existing-config behavior**

Add tests that assert:

- backup is the default recommended path when config already exists
- overwrite remains explicit
- config review summary can be derived before final write

Suggested test names:

```rust
#[test]
fn existing_config_flow_prefers_backup_language() {}

#[test]
fn backup_path_uses_timestamped_suffix() {}

#[test]
fn config_summary_reports_provider_model_and_paths() {}
```

**Step 2: Run the daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: some tests fail because summary helpers or safer default wording are missing.

**Step 3: Implement safer existing-config and summary helpers**

Refactor:

- `resolve_force_write`
- `resolve_backup_path`

Add a non-destructive review summary helper for:

- provider
- model
- credential env
- config path
- memory path
- file root

**Step 4: Re-run daemon tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: config safety and summary tests pass.

**Step 5: Commit**

Run:

```bash
git add crates/daemon/src/onboard_cli.rs crates/daemon/src/tests/onboard_cli.rs
git commit -m "feat: make onboarding config writes safer"
```

Expected: existing configurations are handled explicitly and safely.

---

### Task 6: Upgrade Chat Startup Messaging and First-Run Handoff

**Files:**
- Modify: `crates/app/src/chat.rs`
- Test: `crates/app/src/chat.rs`
- Optional Modify: `README.md`

**Step 1: Write failing unit tests for chat intro helpers**

Add inline unit tests in `crates/app/src/chat.rs` for pure helpers that format:

- startup summary line
- starter prompt suggestions
- help copy

Suggested test names:

```rust
#[test]
fn format_chat_intro_includes_provider_model_and_memory_state() {}

#[test]
fn starter_prompts_include_repository_hint_when_relevant() {}

#[test]
fn help_copy_lists_primary_commands() {}
```

**Step 2: Run the app tests**

Run:

```bash
cargo test -p loongclaw-app chat -- --nocapture
```

Expected: failures because these helper functions do not exist yet.

**Step 3: Implement chat intro helpers**

Refactor `run_cli_chat` and `print_help` so chat startup:

- summarizes session, provider, model, and memory state
- includes three suggested starter prompts
- remains readable in plain text terminals

Keep the actual prompt loop unchanged except for the first-run output.

**Step 4: Re-run the app tests**

Run:

```bash
cargo test -p loongclaw-app chat -- --nocapture
```

Expected: new chat intro tests pass.

**Step 5: Commit**

Run:

```bash
git add crates/app/src/chat.rs README.md
git commit -m "feat: improve chat startup guidance"
```

Expected: onboarding success can hand off into a more informative chat surface.

---

### Task 7: Update CLI Documentation to Recommend Onboarding

**Files:**
- Modify: `README.md`
- Optional Modify: `docs/product-specs/index.md`

**Step 1: Write the minimal docs changes**

Update the quick-start sequence so first-time users are guided toward:

```text
loongclawd onboard
```

before manually running `chat`.

**Step 2: Review docs wording**

Run:

```bash
git diff -- README.md docs/product-specs/index.md
```

Expected: wording is clear, short, and consistent with the redesigned flow.

**Step 3: Commit**

Run:

```bash
git add README.md docs/product-specs/index.md
git commit -m "docs: clarify first-run onboarding flow"
```

Expected: repo docs reflect the intended first-run entrypoint.

---

### Task 8: Full Verification Pass

**Files:**
- Modify: none expected

**Step 1: Run daemon onboarding tests**

Run:

```bash
cargo test -p loongclaw-daemon onboard_cli -- --nocapture
```

Expected: PASS

**Step 2: Run app chat tests**

Run:

```bash
cargo test -p loongclaw-app chat -- --nocapture
```

Expected: PASS

**Step 3: Run full package tests if the targeted suites passed cleanly**

Run:

```bash
cargo test -p loongclaw-daemon
cargo test -p loongclaw-app
```

Expected: PASS

**Step 4: Smoke test the CLI manually**

Run:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- onboard
```

Expected:

- banner renders without wrapping in a normal terminal
- provider selection is structured
- model fallback path works
- success page offers direct chat launch

**Step 5: Commit the final polish if needed**

Run:

```bash
git status --short
```

Expected: no unexpected edits remain after verification.

If any verification-only tweaks were required:

```bash
git add <exact files>
git commit -m "chore: polish onboarding rollout"
```

---

## Notes for Execution

- Prefer small pure helper functions to keep onboarding rendering testable.
- Avoid introducing a full TUI crate in this plan.
- Keep each commit tightly scoped to the task listed above.
- Preserve user-owned config safely; never make overwrite the silent default.
- Treat release and development build labeling as user-facing product behavior, not just debug text.

---

Plan complete and saved to `docs/plans/2026-03-11-loongclaw-onboarding-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

**Which approach?**
