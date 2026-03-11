# LoongClaw Onboarding Redesign

**Date:** 2026-03-11

**Status:** Approved design for `alpha-test`

**Scope:** Redesign `loongclawd onboard` and the first `chat` entry experience so onboarding feels like a product journey instead of a linear config wizard.

---

## 1. Product Goal

`loongclawd onboard` should help a first-time user reach a successful first chat in 60 to 90 seconds.

The command should no longer behave like a sequence of free-form prompts. It should behave like a guided operator console with:

- clear next actions
- strong but restrained branding
- explicit trust and safety boundaries
- recoverable error paths
- a direct handoff into `chat`

This redesign targets the current CLI architecture in:

- `crates/daemon/src/onboard_cli.rs`
- `crates/app/src/chat.rs`

The `alpha-test` implementation should stay lightweight. It should not introduce a heavy full-screen TUI framework unless later work proves that need.

---

## 2. Design Intent

The onboarding visual direction is:

`Warm Operator Console`

This means:

- professional and trustworthy, not playful
- branded, but not decorative
- compact and readable on normal terminals
- able to degrade cleanly on narrow terminals and low-color environments

The visual identity should use the LoongClaw brand palette:

- primary accent: `#b1231c`
- warm highlight: `#fcf5e2`

The accent color should frame action and identity. It should not be reused as the generic warning or error color.

---

## 3. Core Principles

### 3.1 One Strong Thing Per Screen

Each screen gets one primary job:

- welcome establishes tone
- trust establishes boundaries
- provider chooses route
- credential resolves auth
- model confirms runtime target
- preflight confirms readiness
- success launches chat

### 3.2 Fast Happy Path, Rich Recovery Paths

The default path should be short. Recovery paths should be available without dumping the user into raw config editing.

### 3.3 Default First, Editable Later

Advanced customization should exist, but should not compete with first-run success.

### 3.4 Status Before Explanation

Users should see the current state before reading detail text.

Examples:

- `[ok] OPENAI_API_KEY is available`
- `[!] model probe was skipped`
- `[x] memory path is not writable`

### 3.5 Every Warning Needs a Next Action

Warnings and failures must always pair with a recoverable action:

- retry
- use fallback
- switch provider
- save and finish later

### 3.6 Branding Frames Action

Logo, color, and spacing should improve perceived quality without pushing the real action below the fold.

---

## 4. Command Boundary

The redesign assumes these command roles:

- `setup`
  - generate a default config template
  - bootstrap memory path
  - useful for users who want a file first
- `onboard`
  - guided first-run setup
  - the primary entry for first-time CLI users
  - owns provider selection, credential readiness, model selection, preflight, and success handoff
- `doctor`
  - diagnostics and optional repair
  - referenced by onboarding, but not embedded deeply in the alpha flow
- `chat`
  - day-to-day interaction entrypoint
  - should feel like a continuation of onboarding when launched from `onboard`

---

## 5. Banner and Version Strategy

### 5.1 Banner Family

The onboarding banner uses three responsive variants.

Wide, default when the terminal width is at least 80 columns:

```text
██╗      ██████╗  ██████╗ ███╗   ██╗ ██████╗  ██████╗██╗      █████╗ ██╗    ██╗
██║     ██╔═══██╗██╔═══██╗████╗  ██║██╔════╝ ██╔════╝██║     ██╔══██╗██║    ██║
██║     ██║   ██║██║   ██║██╔██╗ ██║██║  ███╗██║     ██║     ███████║██║ █╗ ██║
██║     ██║   ██║██║   ██║██║╚██╗██║██║   ██║██║     ██║     ██╔══██║██║███╗██║
███████╗╚██████╔╝╚██████╔╝██║ ╚████║╚██████╔╝╚██████╗███████╗██║  ██║╚███╔███╔╝
╚══════╝ ╚═════╝  ╚═════╝ ╚═╝  ╚═══╝ ╚═════╝  ╚═════╝╚══════╝╚═╝  ╚═╝ ╚══╝╚══╝
```

Split, when width is between 46 and 79 columns:

```text
██╗      ██████╗  ██████╗ ███╗   ██╗ ██████╗
██║     ██╔═══██╗██╔═══██╗████╗  ██║██╔════╝
██║     ██║   ██║██║   ██║██╔██╗ ██║██║  ███╗
██║     ██║   ██║██║   ██║██║╚██╗██║██║   ██║
███████╗╚██████╔╝╚██████╔╝██║ ╚████║╚██████╔╝
╚══════╝ ╚═════╝  ╚═════╝ ╚═╝  ╚═══╝ ╚═════╝

██████╗██╗      █████╗ ██╗    ██╗
██╔════╝██║     ██╔══██╗██║    ██║
██║     ██║     ███████║██║ █╗ ██║
██║     ██║     ██╔══██║██║███╗██║
╚██████╗███████╗██║  ██║╚███╔███╔╝
╚═════╝╚══════╝╚═╝  ╚═╝ ╚══╝╚══╝
```

Plain, when width is below 46 columns:

```text
LOONGCLAW
```

### 5.2 Version and Channel Line

Version information always stays on a single line under the banner.

Release builds:

```text
v0.1.2
```

Development builds:

```text
v0.1.2 · alpha-test · 1a2b3c4
```

Rules:

- release builds display only semantic version
- development builds display semantic version, channel or branch, and short git SHA
- if branch metadata is missing, fallback to `dev`
- if SHA is missing, omit only the SHA portion
- version text should visually read lighter than the banner

---

## 6. Visual System

### 6.1 Color Roles

- `#b1231c`
  - banner
  - current selection arrow
  - current step or short key emphasis
- `#fcf5e2`
  - version line
  - hero subtitle
  - selected summary highlights on success or review screens
- default terminal foreground
  - body text
  - descriptions
  - most list content

State colors should not reuse the primary brand red:

- success: green family
- warning: warm orange or yellow family
- danger: darker red or fallback terminal error color
- info: muted or neutral highlight

### 6.2 Layout Rules

- left-align all body content
- keep only one blank line between content blocks
- avoid full-width heavy borders
- use spacing and headings before decorative separators
- keep the first actionable control visible on the first screen height where possible

### 6.3 Footer Hints

Footer hints should collapse by available width and by available actions.

Wide:

```text
Enter confirm  j/k move  1-9 quick pick  ? details  b back  q quit
```

Medium:

```text
Enter confirm  ? details  b back  q quit
```

Narrow:

```text
Enter  b  q
```

---

## 7. Screen Flow

### 7.1 Screen Order

Primary path:

1. Welcome
2. Trust
3. Provider
4. Credential
5. Model
6. Advanced
7. Preflight
8. Success
9. Launch chat

Conditional screens:

- Existing config found
- Custom env var
- Review config summary

### 7.2 Welcome

Purpose:

- establish product feel
- communicate speed
- reassure users that advanced settings can be changed later

Reference layout:

```text
<LOONGCLAW banner>

v0.1.2 · alpha-test · 1a2b3c4
trusted local agent runtime

We'll set up your provider, verify local paths,
and start your first chat.

This usually takes under 90 seconds.
You can change advanced settings later.

> Continue
  Exit
```

### 7.3 Trust

Purpose:

- set safety boundary before auth or model selection
- make trust specific to real local paths

Reference layout:

```text
Workspace trust

LoongClaw may read local files and invoke allowed tools.
Use it only in a workspace you trust.

Workspace: /Users/chum/project
File root:  /Users/chum/project
Tool mode:  allowlisted shell + file tools

> I trust this workspace
  Exit
```

Optional detail panel on `?`:

```text
Prompts, local files, and enabled tools can affect runtime behavior.
If this workspace is untrusted, stop here.
```

### 7.4 Existing Config Found

Purpose:

- prevent destructive overwrite behavior
- make backup the default choice

Reference layout:

```text
Existing configuration found

Config path: ~/.loongclaw/config.toml

Choose how to continue.

> Back up existing config
  Replace existing config
  Review current config path
  Exit
```

### 7.5 Provider

Purpose:

- remove free-form provider entry
- surface current machine readiness

Grouping order:

1. Ready now
2. Popular
3. Regional
4. Local

Reference layout:

```text
2/6 Choose a provider

Ready now
> OpenAI        [ready] OPENAI_API_KEY found
  Kimi Coding   [ready] KIMI_CODING_API_KEY found

Popular
  Anthropic     [missing] ANTHROPIC_API_KEY
  OpenRouter    [missing] OPENROUTER_API_KEY

Regional
  Kimi          [missing] MOONSHOT_API_KEY
  Volcengine    [missing] ARK_API_KEY
  Zhipu         [missing] ZHIPU_API_KEY
  Minimax       [missing] MINIMAX_API_KEY
  DeepSeek      [missing] DEEPSEEK_API_KEY
  ZAI           [missing] ZAI_API_KEY

Local
  Ollama        [local] no API key required
```

### 7.6 Credential

Purpose:

- resolve auth before model probing
- keep missing credentials recoverable

States:

- default env found
- default env missing
- local provider, no key required

Reference layout when env is missing:

```text
3/6 Provider credential

Provider: Anthropic
Expected env var: ANTHROPIC_API_KEY
Status: [!] not found

Set the variable in your shell and return here.
You can also choose a custom env var name.

> Re-check
  Use a different env var name
  Switch provider
  Save and finish later
```

### 7.7 Custom Env Var

Purpose:

- let advanced users remap credential env names without pushing that burden onto everyone

Reference layout:

```text
Custom env var name

Default: ANTHROPIC_API_KEY

Env var name: [MY_TEAM_ANTHROPIC_KEY]

We'll look for this variable in the current environment.

> Save and re-check
  Back
```

### 7.8 Model

Purpose:

- pick a usable model
- avoid making probe failure a fatal first-run event

States:

- probe succeeded
- probe failed
- probe skipped
- local provider connectivity check

Reference layout when probe fails:

```text
4/6 Choose a model

Provider: OpenAI
Credential: [ok]
Model probe: [!] request failed

We'll use a default model for now.
You can retry, switch provider, or continue with the default.

> Use default: gpt-5
  Retry probe
  Enter manually
  Switch provider
```

### 7.9 Advanced

Purpose:

- keep high-value customization available
- keep low-value complexity out of the happy path

Default gate:

```text
5/6 Advanced options

Use defaults for:
- system prompt
- file root
- shell allowlist
- sqlite memory path

> Use defaults
  Customize now
```

Alpha-stage customization surface:

- system prompt
- file root

Do not expose shell allowlist and sqlite path editing in the first alpha redesign.

### 7.10 Preflight

Purpose:

- summarize readiness using product language instead of raw diagnostic output

Groups:

- Ready
- Needs attention
- Optional

Reference layout:

```text
6/6 Preflight

Ready
  [ok] OPENAI_API_KEY is available
  [ok] model is set to gpt-5
  [ok] memory path is writable
  [ok] tool file root is writable

Optional
  [i] shell allowlist still uses defaults
      You can tune this later with doctor.

> Start chat now
  Review config
  Save and exit
```

### 7.11 Review Config Summary

Purpose:

- show the effective configuration in human-readable form
- avoid dumping raw TOML into the first-run flow

Reference layout:

```text
Configuration summary

Provider     OpenAI
Model        gpt-5
Credential   OPENAI_API_KEY
File root    /Users/chum/project
Memory       ~/.loongclaw/memory.sqlite3

> Start chat now
  Show config path
  Back
```

### 7.12 Success

Purpose:

- transition from setup mindset into usage mindset

Reference layout:

```text
Ready to chat

Config:   ~/.loongclaw/config.toml
Provider: OpenAI
Model:    gpt-5
Memory:   ~/.loongclaw/memory.sqlite3

Try asking:
- Summarize the current repository
- Explain which tools are enabled
- Help me tune the shell allowlist

> Start chat now
  Review config path
  Exit
```

### 7.13 Chat First Screen

Purpose:

- preserve continuity from onboarding to usage

Reference layout:

```text
loongclaw // session default
provider openai   model gpt-5   memory on

/help commands   /history memory   /exit quit

Try one:
- summarize this repository
- inspect the current config
- explain the enabled tools

you>
```

---

## 8. Interaction Model

All onboarding screens should share the same core controls.

- `Enter`
  - confirm current selection
- `Up/Down`
  - move selection
- `j/k`
  - move selection
- `1-9`
  - quick pick when meaningful
- `b`
  - go back
- `q`
  - exit onboarding
- `?`
  - show contextual explanation or provider detail

Text input should appear only on screens that require actual value entry, such as custom env var name, system prompt, or file root.

---

## 9. State Semantics

Onboarding state should use product-level semantics, not just raw diagnostic codes.

User-facing markers:

- `[ok]`
- `[!]`
- `[x]`
- `[i]`

Meaning:

- `[ok]`
  - ready or verified
- `[!]`
  - caution or fallback path available
- `[x]`
  - blocking issue for first success
- `[i]`
  - informational or optional refinement

Rules:

- all status messages must include an object and a conclusion
- all blocking states must pair with a next action
- warnings should not be colored like brand accent

---

## 10. Recovery Paths

The redesign should make recovery a first-class product feature.

Credential missing should support:

- re-check
- use a different env var name
- switch provider
- save and finish later

Model probe failure should support:

- use default model
- retry probe
- enter manually
- switch provider

File root validation failure should support:

- retry
- choose another root
- reset to workspace root
- save and exit

Existing config should support:

- backup existing config
- replace existing config
- review config path
- exit

---

## 11. Accessibility and Terminal Degradation

The experience must remain usable when:

- colors are disabled
- `NO_COLOR` is set
- Unicode banner rendering is poor
- terminal width is narrow

Therefore:

- the interface cannot rely on color alone
- all screens must remain understandable with plain text markers
- the banner must fall back from wide to split to plain
- footer hints must collapse by width

---

## 12. Acceptance Criteria

The redesign is complete when:

- a standard 80-column terminal displays the wide banner without wrapping
- a 46 to 79-column terminal displays the split banner without clipping
- a narrow terminal falls back cleanly to plain text branding
- the happy path requires no free-form provider entry
- credentials are resolved before model probing
- model probe failure still allows a first-chat path
- preflight groups output into `Ready`, `Needs attention`, and `Optional`
- onboarding no longer defaults to destructive overwrite when config already exists
- success flow offers direct transition into chat
- the first chat screen includes contextual summary and starter prompts
- release builds and development builds are visually distinguishable

---

## 13. Alpha Implementation Priority

### P0

- responsive onboarding banner family
- single-line release and development version display
- welcome screen
- trust screen
- non-destructive existing-config handling
- provider selection UI with readiness grouping
- credential-before-model flow
- model fallback flow
- grouped preflight summary
- success screen with direct chat handoff
- upgraded chat first screen

### P1

- contextual `? details`
- advanced customization menu for system prompt and file root
- configuration summary screen
- width-aware footer hints
- improved local-provider connectivity path for Ollama

### P2

- richer doctor integration
- model recommendation refinement
- resume unfinished onboarding
- more context-sensitive starter prompts

### Explicit Non-Goals for Alpha

- heavy full-screen TUI framework
- animated welcome screen
- inline full config editor
- exposing every low-level runtime knob during onboarding

---

## 14. Summary

This redesign keeps LoongClaw grounded in its existing CLI architecture while dramatically improving perceived quality, safety clarity, and first-run usability.

The result should feel like a disciplined local agent console:

- branded
- trustworthy
- fast
- flexible
- hard to misuse
