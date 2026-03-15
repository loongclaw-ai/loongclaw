# Onboarding

## User Story

As a first-time LoongClaw user, I want a guided setup flow so that I can reach a
working assistant without editing raw config or guessing which command comes
next.

## Acceptance Criteria

- [ ] `loongclaw onboard` is the default first-run path called out in product docs.
- [ ] Onboarding detects reusable provider, channel, or workspace settings when
      available and explains what it found before writing config.
- [ ] The happy path ends with explicit next-step guidance for:
      `loongclaw ask --message "..."` and `loongclaw chat`.
- [ ] Rerunning onboarding does not silently overwrite an existing config unless
      the user explicitly opts into a destructive path such as `--force`.
- [ ] Onboarding uses the same provider, memory, and channel configuration
      surfaces that the runtime uses after setup.
- [ ] When preflight checks fail, onboarding points users to `loongclaw doctor`
      or `loongclaw doctor --fix` as the repair path.

## Out of Scope

- Packaging and installer distribution strategy
- Full channel pairing or browser-based setup UIs
- Arbitrary advanced config editing during first run
