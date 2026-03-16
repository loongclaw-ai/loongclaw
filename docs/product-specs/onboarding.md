# Onboarding

## User Story

As a first-time LoongClaw user, I want a guided setup flow so that I can reach a
working assistant without editing raw config or guessing which command comes
next.

## Acceptance Criteria

- [ ] `loongclaw onboard` is the default first-run path called out in product docs.
- [ ] Onboarding detects reusable provider, channel, or workspace settings when
      available and explains what it found before writing config.
- [ ] The start-fresh path shows the available provider ids and post-setup
      channel ids in alphabetical order so the user has concrete examples
      before typing.
- [ ] The credential step asks for an environment-variable name, not the secret
      value itself, and shows concrete examples such as `OPENAI_API_KEY`.
- [ ] Interactive onboarding uses preset personality and memory-profile steps
      instead of exposing a raw rendered system prompt body.
- [ ] The happy path ends with explicit next-step guidance that promotes
      `loongclaw ask --config ... --message "..."` as the primary smoke test,
      keeps `loongclaw chat --config ...` as the secondary CLI path, and lists
      enabled channel handoff commands afterward.
- [ ] Rerunning onboarding does not silently overwrite an existing config unless
      the user explicitly opts into a destructive path such as `--force`.
- [ ] Onboarding uses the same provider, memory, and channel configuration
      surfaces that the runtime uses after setup.
- [ ] When preflight checks fail, onboarding points users to `loongclaw doctor`
      or `loongclaw doctor --fix` as the repair path.

## Out of Scope

- Package-manager distribution strategy beyond the documented bootstrap installer;
  see [Installation](installation.md)
- Full channel pairing or browser-based setup UIs
- Arbitrary advanced config editing during first run
- Free-form prompt editing as part of the default interactive flow
