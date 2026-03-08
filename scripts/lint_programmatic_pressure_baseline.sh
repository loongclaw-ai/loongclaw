#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MATRIX_PATH="${1:-examples/benchmarks/programmatic-pressure-matrix.json}"
BASELINE_PATH="${2:-examples/benchmarks/programmatic-pressure-baseline.json}"
OUTPUT_PATH="${3:-target/benchmarks/programmatic-pressure-baseline-lint-report.json}"

cargo run -p loongclaw-daemon -- benchmark-programmatic-pressure-lint \
  --matrix "$MATRIX_PATH" \
  --baseline "$BASELINE_PATH" \
  --enforce-gate \
  --output "$OUTPUT_PATH"
