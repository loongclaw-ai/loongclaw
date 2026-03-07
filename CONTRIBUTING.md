# Contributing to LoongClaw

Thanks for contributing. This guide defines the baseline workflow for external and internal contributors.

## Prerequisites

- Rust stable toolchain installed.
- `cargo` available in shell.
- GitHub account with fork access.

## Contribution Tracks

LoongClaw uses two tracks for OSS contribution risk.

### Track A: Routine and low-risk changes

Use Track A for:
- docs updates
- tests
- small refactors
- contained bug fixes

Required checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

### Track B: Higher-risk changes

Use Track B for:
- security-sensitive behavior
- API contract changes
- runtime/kernel policy changes
- architecture-impacting refactors

Track B flow:
1. Open an issue or PR draft with design intent first.
2. Wait for maintainer acknowledgement before deep implementation.
3. Run the same baseline checks as Track A plus any scenario/benchmark checks relevant to changed modules.

If you are unsure which track applies, open an issue and ask maintainers for triage.

## Standard Workflow

1. Fork the repository.
2. Create a branch from `main`.
3. Make focused commits.
4. Run required checks.
5. Open a pull request using the PR template.
6. Address review feedback and keep PR scope focused.

## Commit and PR Expectations

- Use clear, scoped commit messages.
- Keep one logical change per PR when possible.
- Link relevant issue IDs in PR description.
- Include risk notes for Track B changes.

## Review Policy

- At least one maintainer review is required.
- Track B changes require explicit maintainer approval.
- Maintainers may request design clarification before merge.

## Reporting Security Issues

Do not open public issues for security vulnerabilities. Follow [SECURITY.md](SECURITY.md).
