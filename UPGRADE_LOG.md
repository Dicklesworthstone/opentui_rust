# Dependency Upgrade Log

**Date:** 2026-01-19  |  **Project:** opentui  |  **Language:** Rust

## Summary

- **Updated:** 1  |  **Already Latest:** 3  |  **Failed:** 0  |  **Needs Attention:** 0

## Current Versions (Already at Latest)

### bitflags: 2.10.0
- **Status:** Already at latest stable
- **Notes:** Using major version spec `"2"` which correctly resolved to 2.10.0

### ropey: 1.6.1
- **Status:** Already at latest stable (1.x line)
- **Notes:** Ropey 2.0 is in beta - staying on stable 1.x line per maintainer recommendation

### unicode-segmentation: 1.12.0
- **Status:** Already at latest stable
- **Notes:** Using major version spec `"1"` which correctly resolved to 1.12.0

### unicode-width: 0.2.2
- **Status:** Already at latest in 0.2.x line
- **Notes:** Using `"0.2"` spec which resolved to 0.2.2

## Updates

### criterion: 0.5.1 → 0.8.1
- **Type:** Dev dependency
- **Breaking Changes:**
  - Minimum Rust version: 1.80
  - `Bencher::iter_with_large_setup` now requires return type parameter
  - Some deprecated APIs removed
- **Migration:** Updated version spec in Cargo.toml
- **Tests:** ✓ Passed (79 tests)

## Actions Taken

1. Updated `criterion` version in Cargo.toml from `"0.5"` to `"0.8"`
2. Pinned dependency versions to more specific ranges for reproducibility
3. Updated Cargo.lock via `cargo update`
4. Fixed clippy lint errors revealed by stricter analysis:
   - `src/color.rs`: Replaced manual float operations with `mul_add()` for better accuracy
   - `src/renderer/diff.rs`: Moved use statements before variable declarations in tests
   - `src/unicode/grapheme.rs`: Used `.count()` instead of `.collect().len()`
   - `examples/hello.rs`: Moved use statement to top of file
   - `benches/buffer.rs`: Added `#![allow(clippy::semicolon_if_nothing_returned)]`

## Verification

```bash
cargo check      # ✓ Passed
cargo test       # ✓ 79 tests passed
cargo clippy --all-targets -- -D warnings  # ✓ Passed
```
