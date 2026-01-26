# Dependency Upgrade Log

**Date:** 2026-01-26  |  **Project:** opentui  |  **Language:** Rust

## Summary (2026-01-26)

- **Updated:** 2  |  **Already Latest:** 10  |  **Failed:** 0  |  **Needs Attention:** 0

## Updates (2026-01-26)

### tracing: 0.1.40 → 0.1.44
- **Breaking:** None noted (release notes mention a panic fix in `record_all`).
- **Tests:** `cargo test --quiet` ✓ (full suite; `benchmarks_validate::list_benchmarks` ~48s).

### tracing-subscriber: 0.3.19 → 0.3.22
- **Breaking:** None noted; release notes indicate 0.3.21 was yanked due to `EnvFilter` parsing, 0.3.22 is the fix.
- **Tests:** `cargo test --quiet` ✓ (same run as above).

## Already Latest (2026-01-26)

- bitflags: 2.10.0
- libc: 0.2.180
- ropey: 1.6.1
- unicode-segmentation: 1.12.0
- unicode-width: 0.2.2
- criterion: 0.8.1
- proptest: 1.9.0
- insta: 1.46.1
- serde: 1.0.228
- serde_json: 1.0.149

## Verification (2026-01-26)

```bash
CARGO_TARGET_DIR=target-upgrade cargo test --quiet
CARGO_TARGET_DIR=target-upgrade cargo check --all-targets
CARGO_TARGET_DIR=target-upgrade cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo audit
```

**Date:** 2026-01-25  |  **Project:** opentui  |  **Language:** Rust

## Summary

- **Updated:** 1  |  **Already Latest:** 3  |  **Failed:** 0  |  **Needs Attention:** 0

## API Changes (2026-01-25)

- **TextAttributes:** now `u32` with link ID packed into bits 8-31 (flags remain bits 0-7).
- **Style:** removed `link_id` field; use `Style::with_link()` or `TextAttributes::with_link_id()`.
- **Cell/Renderer/AnsiWriter:** hyperlink handling now reads link IDs from packed attributes.
- **GraphemePool/GraphemeId:** new interned grapheme storage with width-aware `GraphemeId` encoding.
- **LinkPool:** new hyperlink pool used by renderers to resolve OSC 8 URLs.
- **ThreadedRenderer:** added channel-based renderer (`opentui::renderer::ThreadedRenderer`) with `present()`, `invalidate()`, and `shutdown()`.
- **Highlighting:** added `HighlightedBuffer`, `SyntaxStyleRegistry`, `TokenizerRegistry`, and `Theme` types to the highlight module.

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
cargo check --all-targets                  # ✓ Passed
cargo clippy --all-targets -- -D warnings  # ✓ Passed
cargo fmt --check                          # ✓ Passed
cargo test                                 # ✓ Passed
cargo bench --bench buffer                 # ✓ Completed
```
