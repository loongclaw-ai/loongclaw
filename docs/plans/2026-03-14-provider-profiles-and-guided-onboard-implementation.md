# Provider Profiles And Guided Onboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add persistent multi-provider profile storage, guided provider/channel supplementation during onboarding/import, canonical credential-source rendering, and runtime-ready active-provider semantics without breaking existing alpha-test behavior.

**Architecture:** Introduce provider profile storage behind a compatibility seam so runtime code can continue consuming a resolved active `ProviderConfig` while config loading/writing evolves. Then update migration/onboard/import flows to operate on retained provider profiles plus one active default, and finally wire in persistent switch primitives on top of that model.

**Tech Stack:** Rust, serde, existing `loongclaw-app` config/provider modules, daemon onboarding/import TUI, cargo test, cargo fmt.

---

### Task 1: Add failing config-storage tests for provider profiles

**Files:**
- Modify: `crates/app/src/config/runtime.rs`
- Modify: `crates/app/src/config/mod.rs`
- Test: `crates/app/src/config/runtime.rs`
- Test: `crates/app/src/config/mod.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn load_legacy_single_provider_into_active_provider_profile() {
    let config = load_config_from_str(legacy_toml).expect("config should parse");
    assert_eq!(config.active_provider_id().as_deref(), Some("openai"));
    assert_eq!(config.providers.len(), 1);
}

#[test]
fn write_config_uses_inline_env_reference_for_provider_credentials() {
    let mut config = LoongClawConfig::default();
    config.set_active_provider_profile("openai", provider_with_env("${OPENAI_API_KEY}"));
    let raw = write_config_to_string(&config).expect("config should write");
    assert!(raw.contains("api_key = \"${OPENAI_API_KEY}\""));
    assert!(!raw.contains("api_key_env"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app load_legacy_single_provider_into_active_provider_profile write_config_uses_inline_env_reference_for_provider_credentials -- --nocapture`
Expected: FAIL because provider profile storage and helpers do not exist.

**Step 3: Write minimal implementation**

- add provider profile storage to `LoongClawConfig`
- add active-provider helpers and compatibility normalization
- keep legacy `provider` readable during deserialization
- keep writes canonical to new storage model

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app load_legacy_single_provider_into_active_provider_profile write_config_uses_inline_env_reference_for_provider_credentials -- --nocapture`
Expected: PASS.

### Task 2: Add a resolved active-provider seam for runtime callers

**Files:**
- Modify: `crates/app/src/config/runtime.rs`
- Modify: `crates/app/src/provider/model_candidate_resolver_runtime.rs`
- Modify: `crates/app/src/provider/request_dispatch_runtime.rs`
- Modify: `crates/app/src/provider/provider_validation_runtime.rs`
- Modify: `crates/app/src/provider/profile_state_backend.rs`
- Test: `crates/app/src/config/runtime.rs`
- Test: `crates/app/src/provider/tests.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn resolved_active_provider_prefers_active_provider_profile() {
    let config = config_with_two_providers();
    assert_eq!(config.active_provider().kind, ProviderKind::Deepseek);
}

#[test]
fn provider_runtime_uses_resolved_active_provider() {
    let config = config_with_two_providers();
    assert_eq!(resolved_model_for(&config), "deepseek-chat");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app resolved_active_provider_prefers_active_provider_profile provider_runtime_uses_resolved_active_provider -- --nocapture`
Expected: FAIL because runtime still reads `config.provider` directly.

**Step 3: Write minimal implementation**

- add `active_provider()` and mutable variants on config
- migrate key runtime callsites to use resolved active provider accessors
- preserve existing behavior for legacy configs through compatibility fallback

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app resolved_active_provider_prefers_active_provider_profile provider_runtime_uses_resolved_active_provider -- --nocapture`
Expected: PASS.

### Task 3: Add failing provider-profile merge tests for import discovery

**Files:**
- Modify: `crates/daemon/src/migration/types.rs`
- Modify: `crates/daemon/src/migration/discovery.rs`
- Modify: `crates/daemon/src/migration/provider_selection.rs`
- Test: `crates/daemon/src/tests/migration.rs`
- Test: `crates/daemon/src/tests/import_cli.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn import_supplements_existing_provider_profiles_without_overwriting_other_profiles() {
    let merged = apply_import_to_base(base_config_with_openai(), candidate_with_deepseek());
    assert_eq!(merged.providers.len(), 2);
    assert_eq!(merged.active_provider_id().as_deref(), Some("openai"));
}

#[test]
fn import_keeps_distinct_openai_compatible_profiles_when_transport_differs() {
    let merged = apply_import_to_base(base_config_with_openrouter(), candidate_with_deepseek());
    assert_eq!(merged.providers.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon import_supplements_existing_provider_profiles_without_overwriting_other_profiles import_keeps_distinct_openai_compatible_profiles_when_transport_differs -- --nocapture`
Expected: FAIL because import still resolves to a single selected provider.

**Step 3: Write minimal implementation**

- introduce provider profile retention in migration types
- replace single `ProviderSelectionPlan` with profile-retention planning
- preserve transport identity during merge decisions
- keep one selected active provider candidate

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon import_supplements_existing_provider_profiles_without_overwriting_other_profiles import_keeps_distinct_openai_compatible_profiles_when_transport_differs -- --nocapture`
Expected: PASS.

### Task 4: Update onboarding/import TUI wording and profile review flow

**Files:**
- Modify: `crates/daemon/src/onboard_cli.rs`
- Modify: `crates/daemon/src/import_cli.rs`
- Modify: `crates/daemon/src/migration/render.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`
- Test: `crates/daemon/src/tests/import_cli.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn onboard_review_summarizes_active_provider_and_retained_profiles() {
    let lines = render_onboard_review_screen_lines(&config_with_profiles(), 80);
    assert!(lines.iter().any(|line| line.contains("active provider")));
    assert!(lines.iter().any(|line| line.contains("saved profiles")));
}

#[test]
fn onboard_credential_prompt_uses_credential_source_wording() {
    let lines = render_credential_source_selection_screen_lines(&config, "${OPENAI_API_KEY}", 80);
    assert!(lines.iter().any(|line| line.contains("choose credential source")));
    assert!(lines.iter().all(|line| !line.contains("credential env")));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon onboard_review_summarizes_active_provider_and_retained_profiles onboard_credential_prompt_uses_credential_source_wording -- --nocapture`
Expected: FAIL because onboarding still assumes one provider and `credential env` wording.

**Step 3: Write minimal implementation**

- change onboarding/import rendering from provider-kind selection to profile retention plus active-provider summary
- replace `credential env` text and prompts with `credential source`
- show `${ENV}` / inline secret / keep current / missing states consistently
- keep narrow-width rendering intact

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon onboard_review_summarizes_active_provider_and_retained_profiles onboard_credential_prompt_uses_credential_source_wording -- --nocapture`
Expected: PASS.

### Task 5: Evolve import preview/apply JSON and CLI resolution

**Files:**
- Modify: `crates/daemon/src/import_cli.rs`
- Modify: `crates/daemon/src/migration/types.rs`
- Test: `crates/daemon/src/tests/import_cli.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn import_preview_json_reports_retained_provider_profiles_and_active_candidate() {
    let payload = render_import_preview_json(&[candidate]).expect("preview should render");
    assert!(payload.contains("\"provider_profiles\""));
    assert!(payload.contains("\"active_provider\""));
}

#[test]
fn import_apply_preserves_existing_active_provider_when_only_supplementing_other_profiles() {
    let imported = apply_import_from_candidate(base_with_openai(), candidate_with_deepseek());
    assert_eq!(imported.active_provider_id().as_deref(), Some("openai"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-daemon import_preview_json_reports_retained_provider_profiles_and_active_candidate import_apply_preserves_existing_active_provider_when_only_supplementing_other_profiles -- --nocapture`
Expected: FAIL because preview/apply still encode one provider selection.

**Step 3: Write minimal implementation**

- update preview JSON schema to report retained profiles and active candidate
- update apply logic to supplement base config and keep current active provider unless explicitly changed
- preserve explicit active-provider change when onboarding/import user selected a new default

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-daemon import_preview_json_reports_retained_provider_profiles_and_active_candidate import_apply_preserves_existing_active_provider_when_only_supplementing_other_profiles -- --nocapture`
Expected: PASS.

### Task 6: Add persistent provider-switch primitives for runtime follow-up

**Files:**
- Modify: `crates/app/src/conversation/session_address.rs`
- Modify: `crates/app/src/config/runtime.rs`
- Modify: `crates/app/src/config/provider.rs`
- Test: `crates/app/src/conversation/session_address.rs`
- Test: `crates/app/src/provider/tests.rs`

**Step 1: Write the failing test**

Add tests covering:

```rust
#[test]
fn provider_switch_resolution_prefers_profile_id_then_kind_default() {
    let resolved = resolve_provider_switch_target(&config_with_duplicate_kind_profiles(), "openai");
    assert_eq!(resolved.as_deref(), Some("openai-main"));
}

#[test]
fn provider_switch_updates_active_and_last_provider() {
    let mut config = config_with_two_providers();
    switch_active_provider(&mut config, "deepseek-cn").expect("switch should succeed");
    assert_eq!(config.active_provider_id().as_deref(), Some("deepseek-cn"));
    assert_eq!(config.last_provider_id().as_deref(), Some("openai-main"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p loongclaw-app provider_switch_resolution_prefers_profile_id_then_kind_default provider_switch_updates_active_and_last_provider -- --nocapture`
Expected: FAIL because persistent switch helpers do not exist yet.

**Step 3: Write minimal implementation**

- add provider switch resolution helpers on config/provider modules
- keep integration points small and runtime-ready
- do not overreach into broad NL intent handling beyond deterministic primitives

**Step 4: Run test to verify it passes**

Run: `cargo test -p loongclaw-app provider_switch_resolution_prefers_profile_id_then_kind_default provider_switch_updates_active_and_last_provider -- --nocapture`
Expected: PASS.

### Task 7: Run focused suites, then broader regression suites

**Files:**
- Modify: `crates/app/src/config/runtime.rs`
- Modify: `crates/app/src/config/provider.rs`
- Modify: `crates/daemon/src/onboard_cli.rs`
- Modify: `crates/daemon/src/import_cli.rs`
- Modify: `crates/daemon/src/migration/discovery.rs`
- Modify: `crates/daemon/src/migration/types.rs`
- Modify: `crates/daemon/src/migration/render.rs`
- Modify: `crates/daemon/src/migration/provider_selection.rs`
- Test: `crates/app/src/provider/tests.rs`
- Test: `crates/daemon/src/tests/import_cli.rs`
- Test: `crates/daemon/src/tests/migration.rs`
- Test: `crates/daemon/src/tests/onboard_cli.rs`

**Step 1: Run focused test groups**

Run:

```bash
cargo test -p loongclaw-app active_provider -- --nocapture
cargo test -p loongclaw-daemon import_preview_json_reports_retained_provider_profiles_and_active_candidate -- --nocapture
cargo test -p loongclaw-daemon onboard_credential_prompt_uses_credential_source_wording -- --nocapture
```

Expected: PASS.

**Step 2: Run broader package suites**

Run:

```bash
cargo test -p loongclaw-app -- --test-threads=1
cargo test -p loongclaw-daemon -- --test-threads=1
```

Expected: PASS.

**Step 3: Run formatting verification**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.
