# OpenTUI Test Harness

This document mirrors the beads_rust harness pattern for conformance and benchmarking.

## Overview

The harness provides:

1. **Conformance tests** - Fixture-based checks for API behavior and rendering outputs
2. **Benchmarks** - Quick regression checks and Criterion benchmarks
3. **Artifact logging** - Optional JSON/text dumps for debugging

## Quick Start

```bash
# Conformance fixtures
scripts/conformance.sh

# Benchmarks
scripts/bench.sh --quick
scripts/bench.sh --criterion
```

## Script Reference

| Script | Purpose | Duration | When to Use |
|---------|---------|----------|-------------|
| `scripts/conformance.sh` | Fixture-based parity checks | ~10-30s | Behavior changes |
| `scripts/bench.sh --quick` | Fast regression checks | ~10-30s | Perf sanity |
| `scripts/bench.sh --criterion` | Full Criterion suite | 2-5min | Perf work |

## Conformance

Conformance tests run fixture-based checks against `tests/conformance/fixtures/opentui.json`.

```bash
scripts/conformance.sh
scripts/conformance.sh --verbose
scripts/conformance.sh --json
scripts/conformance.sh --filter wrap
scripts/conformance.sh --check-fixtures
```

### Regenerating fixtures from legacy (reference capture)

The legacy capture script recomputes `expected_output` using the legacy engine and overwrites
`tests/conformance/fixtures/opentui.json` deterministically.

```bash
# 1) Build the legacy native library (requires Zig 0.15.2)
cd legacy_opentui/packages/core/src/zig
/opt/zig-0.15.2/zig build -Doptimize=ReleaseFast

# 1.5) Record the legacy commit hash used for capture
cd ../../../..
git rev-parse HEAD

# 2) Run the capture (writes fixtures in this repo)
bun run capture:conformance --lib-path ./src/zig/lib/x86_64-linux/libopentui.so
```

Optional flags:
```bash
bun run capture:conformance \
  --lib-path ./src/zig/lib/x86_64-linux/libopentui.so \
  --input ../../../../tests/conformance/fixtures/opentui.json \
  --output ../../../../tests/conformance/fixtures/opentui.json \
  --captured-at 2026-01-25T00:00:00Z \
  --version 0.1.74
```

**Output**
- Summary JSON: `target/test-artifacts/conformance_summary.json`
- Artifacts: `target/test-artifacts/conformance/<test>/`

## Benchmarks

```bash
scripts/bench.sh --quick
scripts/bench.sh --criterion
scripts/bench.sh --save baseline-v1
scripts/bench.sh --baseline baseline-v1
```

**Output**
- Summary JSON: `target/test-artifacts/benchmark_summary.json`
- Artifacts: `target/test-artifacts/benchmark/`
- Criterion reports: `target/criterion/`

### Highlight Benchmarks

The `highlight` benchmark suite targets the syntax highlighting hot paths.

```bash
cargo bench --bench highlight
```

**Targets (guidance, not hard limits):**

| Operation | Target |
|-----------|--------|
| Single line tokenize | <1μs |
| Incremental update (1 line) | <1ms |
| Full highlight (1000 lines) | <50ms |
| Full highlight (10000 lines) | <500ms |
| Theme switch | <100μs |

`highlight_memory_estimate` reports token count and text bytes as a lightweight
allocation proxy for highlight data structures.

## Artifact Logging

Enable detailed artifact logging (tests only):

```bash
HARNESS_ARTIFACTS=1 cargo test --test conformance -- --nocapture
HARNESS_PRESERVE_SUCCESS=1 cargo test --test conformance
```

Artifacts are written to `target/test-artifacts/<suite>/<test>/`.

## Environment Variables

| Variable | Default | Description |
|---------|---------|-------------|
| `HARNESS_ARTIFACTS` | 0 | Enable artifact logging for tests |
| `HARNESS_PRESERVE_SUCCESS` | 0 | Keep artifacts on success |
| `HARNESS_ARTIFACTS_DIR` | target/test-artifacts | Base artifacts directory |
| `CONFORMANCE_TIMEOUT` | 120 | Per-test timeout (seconds) |
| `BENCH_TIMEOUT` | 300 | Benchmark timeout (seconds) |
| `BENCH_CONFIRM` | 0 | Skip full benchmark confirmation |

## Troubleshooting

**Conformance fixtures missing**
```bash
scripts/conformance.sh --check-fixtures
```

**Benchmarks too slow**
```bash
scripts/bench.sh --quick
```
