# LoongClaw

LoongClaw is a Rust-first Agentic OS foundation focused on stable kernel contracts,
strict policy boundaries, and highly pluggable runtime orchestration.

## Workspace Layout

- `crates/kernel` (`loongclaw-kernel`): core architecture contracts and execution kernel.
- `crates/daemon` (`loongclaw-daemon` / `loongclawd`): runnable daemon wired to kernel policy and runtime controls.

## Core Design

The kernel enforces layered execution planes with core/extension separation:

- pack/policy boundaries
- harness runtime routing
- runtime/tool/memory/connector planes
- audit and deterministic timeline controls
- integration, plugin IR, bootstrap activation, architecture guard, and awareness snapshots

For full details, see [Layered Kernel Design](docs/layered-kernel-design.md).

## Current Validation Status

- `loongclaw-kernel`: 39 unit tests passing.
- `loongclaw-daemon`: 80 unit tests passing.
- `loongclawd` smoke/spec execution verified.
- `programmatic` pressure benchmark gate (matrix + baseline) verified.

## Quick Start

```bash
cargo test -p loongclaw-kernel
cargo test -p loongclaw-daemon
cargo run -p loongclaw-daemon --bin loongclawd
cargo run -p loongclaw-daemon --bin loongclawd -- run-spec --spec examples/spec/runtime-extension.json --print-audit
cargo run -p loongclaw-daemon --bin loongclawd -- run-spec --spec examples/spec/tool-search.json --print-audit
cargo run -p loongclaw-daemon --bin loongclawd -- run-spec --spec examples/spec/programmatic-tool-call.json --print-audit
cargo run -p loongclaw-daemon --bin loongclawd -- benchmark-programmatic-pressure --matrix examples/benchmarks/programmatic-pressure-matrix.json --enforce-gate
./scripts/benchmark_programmatic_pressure.sh
```

## Documentation Index

- [Documentation Home](docs/index.md)
- [Roadmap](docs/roadmap.md)
- [Spec Runner Reference](docs/reference/spec-runner.md)
- [Plugin Runtime Governance](docs/reference/plugin-runtime-governance.md)
- [Programmatic Tool Call](docs/reference/programmatic-tool-call.md)
- [Programmatic Pressure Benchmark](docs/reference/programmatic-pressure-benchmark.md)
- [Plugin Manifest Format](docs/reference/plugin-manifest-format.md)

## Open Source Contribution

- [Contributing Guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Security Policy](SECURITY.md)
