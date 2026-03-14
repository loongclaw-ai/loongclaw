# Provider Profiles And Guided Onboard Design

Date: 2026-03-14
Status: Approved for implementation

## Goal

Upgrade LoongClaw onboarding and import from a single-provider migration flow
into a guided runtime setup that can:

- retain multiple imported provider profiles
- keep exactly one active runtime provider at a time
- preserve provider transport behavior during migration
- merge imported channels and provider credentials without blind overwrite
- present one onboarding flow for broad users while keeping explicit power-user
  commands available

The design keeps channel onboarding/import registry-driven and extends provider
handling to first-class profiles instead of continuing to overload a single
`provider` field.

## Product Principles

### Principle 1: Onboard owns the broad-user happy path

Broad users should not need to understand import sources. If LoongClaw detects
legacy configs during onboarding, it should fold those findings into the normal
guided setup flow and only surface resolver screens when action is required.

### Principle 2: Import remains available for explicit control

`loongclaw import` remains a power-user entry point, but it should converge on
the same internal merge model, provider identity rules, and review summaries as
the onboarding flow.

### Principle 3: Providers and channels are both multi-source, but not the same

Channels are naturally multi-enabled and should continue using the channel
registry as the extension seam.

Providers need persistent storage for multiple runtime profiles but must still
resolve to one active default provider at runtime.

### Principle 4: Merge by runtime identity, not by source precedence

When imported content refers to the same provider runtime surface, LoongClaw
should supplement missing fields. When it refers to a different runtime surface,
LoongClaw should retain it as a separate provider profile.

Source files do not "win" globally. The final review screen summarizes the
resulting runtime config, not source precedence.

### Principle 5: Natural-language provider switching is persistent

When a user clearly asks LoongClaw to switch providers, that switch updates the
default runtime provider until changed again. Session-only switching is not the
primary model.

### Principle 6: Canonical secret config stays standard

New config writes should use inline credential fields with explicit env
references such as `${OPENAI_API_KEY}`. Legacy `*_env` fields remain readable
for compatibility but are not the canonical output form.

## Current Baseline

The current alpha-test baseline already supports:

- imported provider transport preservation through `base_url`,
  `chat_completions_path`, `wire_api`, and endpoint-like settings
- registry-driven channel migration and rendering
- guided onboarding with provider/model/credential prompts
- import preview and apply logic
- parsing inline env references such as `${VAR}`, `$VAR`, `env:VAR`, and `%VAR%`

The current gaps are architectural:

- `LoongClawConfig` still stores only one top-level `provider`
- import preview still assumes choosing one provider kind
- onboarding credential UX still centers on `credential env`
- runtime switching has no first-class persistent active-provider concept

## Target Configuration Model

Add first-class provider profile storage:

```toml
active_provider = "openai-main"
last_provider = "deepseek-cn"

[providers.openai-main]
kind = "openai"
model = "gpt-5"
api_key = "${OPENAI_API_KEY}"
wire_api = "responses"
default_for_kind = true

[providers.deepseek-cn]
kind = "openai_compatible"
model = "deepseek-chat"
base_url = "https://api.deepseek.com"
chat_completions_path = "/v1/chat/completions"
api_key = "${DEEPSEEK_API_KEY}"
```

Required semantics:

- many provider profiles may be stored
- exactly one provider id is active at runtime
- `last_provider` is optional state for explicit switch-back support
- one profile per provider kind may be marked `default_for_kind`
- current `provider` remains readable as a legacy alias during compatibility
  migration

## Provider Profile Identity

Provider profile identity is not just `kind`.

Identity inputs:

- provider kind
- `wire_api`
- explicit `endpoint` when present
- otherwise `base_url + chat_completions_path`
- credential binding shape when it changes runtime behavior

Non-identity inputs:

- current model
- preferred models
- retry/cache knobs

Rules:

- same identity => supplement missing fields
- different identity => retain as separate profiles
- same kind with different endpoint/base URL => separate profiles

## Compatibility Strategy

### Read compatibility

LoongClaw must continue reading:

- legacy top-level `provider`
- legacy `api_key_env`
- legacy `oauth_access_token_env`

These are normalized in memory into provider profiles plus canonical inline
credential references where possible.

### Write compatibility

New writes should:

- write `providers`
- write `active_provider`
- optionally write `last_provider`
- write `${ENV_NAME}` in inline credential-bearing fields
- omit legacy `*_env` fields from newly written configs

### Runtime compatibility seam

To avoid a massive first refactor, runtime/app callsites should gain a resolved
active-provider seam so most existing code can keep operating on
`&ProviderConfig` while config storage evolves under it.

## Onboard And Import UX

### Unified mental model

Users should choose what LoongClaw will run with, not which source file wins.

The guided flow should resolve:

- which provider profiles to retain
- which provider becomes active by default
- which channels to enable or supplement
- which credentials remain missing

### Onboard flow

1. Starting point
   - use current setup
   - review and supplement from detected configs
   - start fresh
2. Provider resolver
   - list retained provider profiles
   - highlight the active default
   - explain merge vs separate-profile decisions
3. Channel resolver
   - enable, keep, supplement, or skip per channel
4. Credential source
   - show `${ENV_NAME}`, inline secret, keep current, or missing
5. Review
   - summarize final runtime config, not source precedence

### Import flow

`loongclaw import` should expose the same decisions, but in an explicit command
surface. The preview JSON should evolve from `provider_selection` to a richer
provider profile summary:

- detected provider profiles
- profiles to save
- active provider candidate
- channels to enable or supplement

## Provider Switching Semantics

When the user says "switch to X":

1. try exact provider profile id
2. try provider kind
3. if only one profile matches, switch immediately
4. if multiple profiles of that kind exist, prefer `default_for_kind`
5. if still ambiguous, ask for clarification

Clear switch intent updates `active_provider` and optionally `last_provider`.
Mere discussion of providers must not trigger a switch.

## Channels, Tools, And Skills

Channels remain first-class multi-import surfaces and should stay driven by the
channel registry and descriptors.

Tools and skills are important migration surfaces, but they should only be
auto-imported through explicit adapters. Unsupported or low-confidence mappings
should surface as review guidance instead of silent migration.

## Rollout Phases

### Phase 1: Storage and guided migration

- add provider profile storage and active-provider state
- support compatibility read for legacy config
- update onboarding/import to retain multiple provider profiles
- switch credential UX from `credential env` to `credential source`
- keep channels multi-import and registry-driven

### Phase 2: Persistent runtime switching

- allow runtime and agent flows to change `active_provider`
- add explicit provider-management CLI affordances
- support optional `last_provider` switch-back behavior

### Phase 3: Richer migration surfaces

- adapter-driven tools/skills migration
- richer provider/channel resolvers
- expanded preview JSON and diagnostics

## Test Matrix

### Config tests

- legacy single-provider config loads into provider profiles
- legacy `*_env` remains readable
- new writes emit `${ENV}` and omit legacy `*_env`
- active-provider resolution remains stable

### Migration tests

- same-identity provider imports supplement fields
- different-identity imports retain separate profiles
- transport details remain preserved
- existing LoongClaw config plus imported config results in supplementation, not
  blind overwrite

### Onboard/import tests

- no detected sources keeps normal onboard
- existing config plus detected configs offers supplement flow
- review screen summarizes saved profiles and active provider
- import preview JSON reports retained profiles and active candidate

### Switching tests

- explicit switch intent changes active provider
- ambiguous provider-kind switch requests prompt for clarification
- non-switch discussion does not change active provider

### Extensibility tests

- adding a new channel descriptor does not require changing core resolver logic
- provider profile identity remains stable for openai-compatible transports with
  distinct endpoints
