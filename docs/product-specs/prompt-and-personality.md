# Prompt And Personality

## User Story

As a LoongClaw operator, I want native prompt and personality presets so that I
can start with a consistent LoongClaw identity without manually writing a full
system prompt.

## Acceptance Criteria

- [ ] LoongClaw has a native base prompt owned by the product rather than only a
      free-form prompt string.
- [ ] Onboarding offers three default personalities:
      `calm_engineering`, `friendly_collab`, and `autonomous_executor`.
- [ ] Interactive onboarding shows preset descriptions and decision guidance
      without dumping the raw prompt body onto the screen.
- [ ] All personalities share the same safety-first operating boundaries.
- [ ] Personality selection can affect tone and action style without weakening
      security requirements.
- [ ] Non-interactive onboarding supports personality selection with a stable
      CLI flag.
- [ ] Advanced users can still provide a full inline system prompt override via
      `--system-prompt` or direct config editing.

## Out of Scope

- Arbitrary end-user personality editing in the first release
- Raw system-prompt editing inside the happy-path onboarding flow
- Full workspace template pack generation
- Migration import/nativeization flows
