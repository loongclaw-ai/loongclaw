## Summary

- What changed?
- Why this change is needed?

## Scope

- [ ] Small and focused
- [ ] Includes docs updates (if needed)
- [ ] No unrelated refactors

## Risk Track

- [ ] Track A (routine/low-risk)
- [ ] Track B (higher-risk/policy-impacting)

If Track B, include design/risk notes:

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] Additional scenario/benchmark checks (if applicable)
- [ ] If this changes config/env fallback, limits, or defaults: include before/after behavior and regression coverage for explicit path, fallback path, and boundary values
- [ ] If tests mutate process-global env: document how state is restored or serialized

## Linked Issues

Closes #
