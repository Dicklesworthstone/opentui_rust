# Test Coverage Guide

This document describes how to generate and view test coverage for OpenTUI.

## Quick Start

```bash
# Generate coverage and open HTML report
cargo llvm-cov --open

# Generate coverage with specific threshold check
cargo llvm-cov --fail-under-lines 70
```

## Coverage Tooling

OpenTUI uses [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) for accurate coverage measurement. This tool leverages LLVM's native coverage instrumentation.

### Installation

```bash
cargo install cargo-llvm-cov
```

The nightly toolchain with `llvm-tools-preview` component is required:

```bash
rustup component add llvm-tools-preview --toolchain nightly
```

## Local Coverage Commands

### HTML Report (Recommended)

Generates a browsable HTML report:

```bash
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

### Quick Console Summary

```bash
cargo llvm-cov
```

### JSON Output (for CI/tooling)

```bash
cargo llvm-cov --json --output-path coverage.json
```

### Codecov Format

```bash
cargo llvm-cov --codecov --output-path codecov.json
```

### LCOV Format

```bash
cargo llvm-cov --lcov --output-path lcov.info
```

## Coverage Thresholds

The project enforces these minimum coverage targets:

| Module | Target | Rationale |
|--------|--------|-----------|
| Overall | ≥70% | Baseline requirement |
| `src/color.rs` | ≥95% | Core blending algorithms |
| `src/cell.rs` | ≥95% | Critical data structure |
| `src/buffer/` | ≥90% | Primary drawing surface |
| `src/ansi/` | ≥90% | Terminal output correctness |
| `src/input/` | ≥90% | Event parsing accuracy |
| `src/text/` | ≥85% | Text editing functionality |
| `src/renderer/` | ≥85% | Diff detection logic |
| `src/terminal/` | ≥75% | FFI limitations apply |

### Check Threshold Locally

```bash
# Fail if overall coverage drops below 70%
cargo llvm-cov --fail-under-lines 70
```

## CI Integration

Coverage runs automatically on every PR and push to main. The CI workflow:

1. Generates JSON, HTML, and Codecov-format reports
2. Checks module-specific thresholds
3. Uploads to Codecov for tracking
4. Posts coverage summary as PR comment
5. Archives HTML report as artifact

See `.github/workflows/ci.yml` (coverage job) for details.

## Excluding Code from Coverage

### Temporary Exclusions

For code that cannot be tested (e.g., FFI, platform-specific):

```rust
#[cfg(not(tarpaulin_include))]
fn untestable_ffi_function() {
    // ...
}
```

### Permanent Exclusions

Configure in `codecov.yml`:

```yaml
ignore:
  - "src/bin/**"
  - "tests/**"
  - "benches/**"
```

## Improving Coverage

### Finding Uncovered Code

1. Generate HTML report: `cargo llvm-cov --html`
2. Open `target/llvm-cov/html/index.html`
3. Navigate to low-coverage files
4. Look for red-highlighted uncovered lines

### Common Gaps

- Error handling paths (test with invalid inputs)
- Edge cases (empty strings, zero dimensions, max values)
- Platform-specific code (may need mocking)
- Panic paths (use `#[should_panic]` tests)

### Property-Based Testing

Use proptest to find edge cases automatically:

```rust
proptest! {
    #[test]
    fn rgba_blending_properties(
        r in 0.0f32..=1.0,
        g in 0.0f32..=1.0,
        b in 0.0f32..=1.0,
        a in 0.0f32..=1.0,
    ) {
        let color = Rgba::new(r, g, b, a);
        // Properties to verify...
    }
}
```

## Viewing Coverage History

Coverage trends are tracked on Codecov. After CI runs, view:

- Per-file coverage breakdown
- Historical trends
- Per-PR coverage changes
- Module-level analysis

## Troubleshooting

### "LLVM tools not found"

```bash
rustup component add llvm-tools-preview --toolchain nightly
```

### Slow Coverage Generation

Use workspace flag to avoid redundant compilation:

```bash
cargo llvm-cov --workspace
```

### Coverage Differs from Local vs CI

Ensure you're running all features:

```bash
cargo llvm-cov --all-features --workspace
```

### Specific Test Coverage

Run coverage for a specific test:

```bash
cargo llvm-cov --test color_tests
```

## Related Documentation

- [cargo-llvm-cov documentation](https://github.com/taiki-e/cargo-llvm-cov)
- [Codecov documentation](https://docs.codecov.com/)
- [LLVM Source-Based Code Coverage](https://clang.llvm.org/docs/SourceBasedCodeCoverage.html)
