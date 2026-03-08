# LoongClaw Agent Guide

This document is intentionally mirrored in `CLAUDE.md` and `AGENTS.md`.

## 1. Architecture Contract

```text
loongclawd (bin)
  -> crates/daemon
       -> crates/kernel
       -> crates/protocol

crates/kernel   -> (no internal loongclaw crate deps)
crates/protocol -> (no daemon/kernel imports)
```

Non-negotiable boundaries:
- `crates/kernel` owns policy, pack boundaries, and execution-plane contracts.
- `crates/protocol` is transport/routing foundation only; keep it runtime-business-logic free.
- `crates/daemon` composes runtime channels/tools/providers and may depend on `kernel` + `protocol`.
- Cross-layer behavior changes must include tests in the affected crate(s).

## 2. Commands Cheat Sheet

- Build workspace: `cargo build --workspace`
- Format check: `cargo fmt --all -- --check`
- Lint: `cargo clippy --workspace --all-targets --all-features`
- Strict lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Test workspace: `cargo test --workspace`
- Test all features: `cargo test --workspace --all-features`
- Canonical verify: `task verify`
- Extended verify: `task verify:full`
- Run daemon help: `cargo run -p loongclaw-daemon --bin loongclawd -- --help`
- Run install script: `./scripts/install.sh --setup`

## 3. Code Generation Workflow

No generated source is required in the current Rust workspace.

If code generation is introduced later:
- Document generator entrypoints and output files here.
- Add regeneration commands to `Taskfile.yml`.
- Ensure generated artifacts are reproducible and included/excluded intentionally.

## 4. Non-Negotiable Rules

- Keep kernel contract behavior backward-compatible unless an explicit breaking-change decision is documented.
- Do not bypass policy checks for tool/runtime/connector actions.
- Keep strict lint and all-feature tests healthy; run them before high-risk merges.
- Never commit credentials, tokens, or private endpoints.
- Keep `CLAUDE.md` and `AGENTS.md` mirrored in the same change.

## 5. Verification Gates

- Default gate before completion: `task verify`.
- For runtime/policy/benchmark changes: run `task verify:full`.
- CI enforces:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --all-features`
  - `cargo test --workspace`
