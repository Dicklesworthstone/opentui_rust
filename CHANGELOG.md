# Changelog

All notable changes to [opentui_rust](https://github.com/Dicklesworthstone/opentui_rust) are documented here.

This project is a Rust port of [anomalyco/opentui](https://github.com/anomalyco/opentui) (TypeScript) -- a high-performance terminal UI rendering engine with alpha blending, scissor clipping, and double-buffered diff rendering.

Format: capabilities-oriented, grouped by domain. Commit links point to the canonical GitHub repository.

---

## [Unreleased]

**Commits on `main` since v0.2.1** (2 commits, 2026-02-25 to 2026-03-13)

### Maintenance

- Remove stale `a.out` compiler output from tracked files ([`3f060f9`](https://github.com/Dicklesworthstone/opentui_rust/commit/3f060f9d7a6cdb012b62eac3af8f1ba8452ecb3a))
- Add cass (Cross-Agent Session Search) tool reference to AGENTS.md ([`91de673`](https://github.com/Dicklesworthstone/opentui_rust/commit/91de6732d23e66bc6f17039e93ade89684169844))

---

## [v0.2.1] -- 2026-02-22

**Tag only** (no GitHub Release). Published to [crates.io](https://crates.io/crates/opentui_rust/0.2.1). 6 commits over v0.2.0.

### License

- Switch from MIT to **MIT with OpenAI/Anthropic Rider** ([`3bdb712`](https://github.com/Dicklesworthstone/opentui_rust/commit/3bdb712990771e391c3d890c89a87b26d83153c5))
- Update README license references to match ([`77089a6`](https://github.com/Dicklesworthstone/opentui_rust/commit/77089a6f15f6d61c41ebcb6e2161afa21b86a032))

### Test Snapshots

- Update insta snapshots for ANSI sequence rendering tests ([`6d0ca31`](https://github.com/Dicklesworthstone/opentui_rust/commit/6d0ca31c7d98b43260eaa43baa933bf575a74174))
- Accept insta snapshots -- migrate `.snap.new` to canonical `.snap` files ([`90d89e0`](https://github.com/Dicklesworthstone/opentui_rust/commit/90d89e0843518668a4c1935bbf6790954222e659))

### Packaging

- Bump Cargo.toml version to 0.2.1 ([`5962dde`](https://github.com/Dicklesworthstone/opentui_rust/commit/5962dde10d5ed379bb7c7907e100e0ba10ab4e7c))
- Add GitHub social preview image (1280x640) ([`f6c16c8`](https://github.com/Dicklesworthstone/opentui_rust/commit/f6c16c81d9012a859afd7940998592936ead1e01))

---

## [v0.2.0] -- 2026-02-15

**First tagged release.** [GitHub Release](https://github.com/Dicklesworthstone/opentui_rust/releases/tag/v0.2.0). Published to [crates.io](https://crates.io/crates/opentui_rust/0.2.0). 314 commits from project init (2026-01-19) through release tag (2026-02-15).

```toml
[dependencies]
opentui_rust = "0.2.0"
```

### Core Rendering Engine (2026-01-19)

The full Zig-to-Rust port of the OpenTUI rendering engine, implemented in the initial commit series.

- **RGBA color system** with f32 components, Porter-Duff "over" alpha blending, HSV/hex/u8 constructors, and linear interpolation ([`7bfaaaf`](https://github.com/Dicklesworthstone/opentui_rust/commit/7bfaaaf986f6cff97e6720b84ee0b17a4db44560))
- **Cell model** with `CellContent` enum (char, grapheme cluster, empty, continuation), per-cell style with foreground/background/attributes ([`7bfaaaf`](https://github.com/Dicklesworthstone/opentui_rust/commit/7bfaaaf986f6cff97e6720b84ee0b17a4db44560))
- **OptimizedBuffer** with contiguous cell arrays, `set`/`set_blended`/`get`/`clear`/`fill_rect`/`draw_text`/`draw_box`/`draw_buffer` operations ([`ed57e24`](https://github.com/Dicklesworthstone/opentui_rust/commit/ed57e24ce26327efc0618045bf5ae13b9ba7e586))
- **Scissor clipping** via `ClipRect` with push/pop stack -- nested viewports clip drawing to their bounds ([`ed57e24`](https://github.com/Dicklesworthstone/opentui_rust/commit/ed57e24ce26327efc0618045bf5ae13b9ba7e586))
- **Opacity stacks** with push/pop multiplier -- hierarchical transparency for layered UI ([`ed57e24`](https://github.com/Dicklesworthstone/opentui_rust/commit/ed57e24ce26327efc0618045bf5ae13b9ba7e586))
- **Double-buffered renderer** with automatic diff detection -- only changed cells generate ANSI output ([`64a83b2`](https://github.com/Dicklesworthstone/opentui_rust/commit/64a83b24f8888a1a5d3fec72be9e727258f40e7e))
- **ANSI escape sequence generation** with buffered writer, synchronized output (`\x1b[?2026h`), true color support ([`0a5033b`](https://github.com/Dicklesworthstone/opentui_rust/commit/0a5033bd8fa592ec2c2bc8b4540bd0c5c7258dea))
- **Terminal abstraction** with raw mode, capability detection, cursor control, mouse protocol handling ([`cd8d6c0`](https://github.com/Dicklesworthstone/opentui_rust/commit/cd8d6c0ac752aa478ecafabad99b3400b77c9c87))
- **ANSI input parser** for keyboard, mouse, and bracketed paste events ([`c3624ed`](https://github.com/Dicklesworthstone/opentui_rust/commit/c3624eddd57b80b7be1ea0fe8f8c731fba542218))
- **Public API** with crate-level lint configuration (clippy pedantic + nursery) ([`1fb2cdb`](https://github.com/Dicklesworthstone/opentui_rust/commit/1fb2cdb5eed2e89c9bf1f9d48c86940cdf0ede40))

### Unicode and Text (2026-01-19 to 2026-01-30)

- **Grapheme clustering** via `unicode-segmentation` and **display width** via `unicode-width` ([`9983d37`](https://github.com/Dicklesworthstone/opentui_rust/commit/9983d3784a6456c455c3bfc9172d5528002d2612))
- **Rope-based text editing** (`ropey`) with undo/redo, `TextBuffer`, `TextBufferView`, `EditBuffer`, word/char wrapping ([`cf2cf27`](https://github.com/Dicklesworthstone/opentui_rust/commit/cf2cf274b849f3b6435551a07c0f37897cb5f57f))
- **BiDi text resolution** implementing UAX #9 with display reordering ([`fede04a`](https://github.com/Dicklesworthstone/opentui_rust/commit/fede04ab4336986f95e020d76088b5c50cce9363), [`0d9e337`](https://github.com/Dicklesworthstone/opentui_rust/commit/0d9e337a72fb67fa6426674d2d7f1db68f2c52ff))
- **NFC/NFD normalization module** ([`cb4c8cb`](https://github.com/Dicklesworthstone/opentui_rust/commit/cb4c8cb297f43846b247df1d7462fac6793dd372))
- **Character width override system** for CJK and special characters ([`6bb3c6f`](https://github.com/Dicklesworthstone/opentui_rust/commit/6bb3c6fa940538f7a08cf79ed80c7e0ac6c69fbc))
- `split_graphemes_with_widths` and `find_grapheme_boundary` utilities ([`eae5545`](https://github.com/Dicklesworthstone/opentui_rust/commit/eae554596b434595b05ca5a638f877b362a63407))

### GraphemePool (2026-01-24 to 2026-01-30)

Interned grapheme storage with ref counting for multi-codepoint grapheme clusters.

- Switch `CellContent` graphemes to packed encoded IDs with width encoding ([`2a3d13e`](https://github.com/Dicklesworthstone/opentui_rust/commit/2a3d13ecdba47e9803b206e369952538dc957fdd))
- Fragmentation ratio calculation and compaction heuristics ([`2d430b6`](https://github.com/Dicklesworthstone/opentui_rust/commit/2d430b6457c4458b3b6a1f32911cf70948c78060), [`b12a898`](https://github.com/Dicklesworthstone/opentui_rust/commit/b12a8984e1005a55a652b31fae67123a33514530))
- `CompactionResult` struct for pool compaction ID remapping ([`5fd321a`](https://github.com/Dicklesworthstone/opentui_rust/commit/5fd321a8ab40292ae398f6c9f77131a60bfb92b9))
- `alloc_batch` for bulk grapheme allocation ([`c2a6b48`](https://github.com/Dicklesworthstone/opentui_rust/commit/c2a6b48bf5bf17f2b2c412777befc2c52c921195))
- `get_memory_usage` introspection ([`48d4fe5`](https://github.com/Dicklesworthstone/opentui_rust/commit/48d4fe57d4652189d30c1d8c9cfb9e92983db203))
- Lifetime statistics tracking ([`f42c443`](https://github.com/Dicklesworthstone/opentui_rust/commit/f42c443f2d68232df82e1a9566b781acc8e2f573))
- Configurable and runtime-adjustable compaction threshold ([`65a4bf3`](https://github.com/Dicklesworthstone/opentui_rust/commit/65a4bf3d8e299dbc644944a7a1bb74d6892c1e87), [`0039d86`](https://github.com/Dicklesworthstone/opentui_rust/commit/0039d86a3250a348571de89adc164c77b2b9372f))
- Utilization monitoring and soft limits ([`79be9a3`](https://github.com/Dicklesworthstone/opentui_rust/commit/79be9a3327f72cf05a55a6d3639e7a563997754f))
- HashMap index for O(1) `intern()` lookup ([`f0b9718`](https://github.com/Dicklesworthstone/opentui_rust/commit/f0b9718508cac2b84676841e3aed91e1bc6ef92c))

### Syntax Highlighting (2026-01-24 to 2026-01-30)

- Token model and tokenizer base with `TokenKind`, `Theme`, `SyntaxStyleRegistry`, `TokenizerRegistry` ([`5deece8`](https://github.com/Dicklesworthstone/opentui_rust/commit/5deece85f65a70e24b08137db547dda9c7a97274))
- Python and TOML language tokenizers ([`31e5eec`](https://github.com/Dicklesworthstone/opentui_rust/commit/31e5eec1f2e451973effd7f77105f8c915c667c6))
- `HighlightedBuffer` wrapper for `TextBuffer` ([`5deece8`](https://github.com/Dicklesworthstone/opentui_rust/commit/5deece85f65a70e24b08137db547dda9c7a97274))

### Hyperlinks (OSC 8)

- `LinkPool` for hyperlink URL storage, link IDs packed into `TextAttributes` ([`bd902b0`](https://github.com/Dicklesworthstone/opentui_rust/commit/bd902b0cff2ad912bae54352c5989757b4f2d86a))
- OSC 8 hyperlink output in ANSI renderer ([`61cd5c2`](https://github.com/Dicklesworthstone/opentui_rust/commit/61cd5c20f526ef676a2ed2b4b46f1d83fd52be33))

### Renderer Capabilities (2026-01-25 to 2026-02-04)

- **Threaded renderer** (`ThreadedRenderer`) for off-main-thread terminal I/O ([`d530777`](https://github.com/Dicklesworthstone/opentui_rust/commit/d53077749f1b022e258aad2603704c2729ed5195))
- **Layer-based hit testing** with compositing and double-buffered hit grid ([`47061ed`](https://github.com/Dicklesworthstone/opentui_rust/commit/47061edec2b89b62266a561eefc67b49d46e8772), [`4288638`](https://github.com/Dicklesworthstone/opentui_rust/commit/4288638992a88d6ed14ab107c2937737701c7564))
- **Dirty region tracking** for efficient incremental redraws ([`5e201b5`](https://github.com/Dicklesworthstone/opentui_rust/commit/5e201b59a1e22ab2fea5390acff752ca3d66c8db))
- **Debug overlay** for renderer diagnostics ([`201442e`](https://github.com/Dicklesworthstone/opentui_rust/commit/201442ea647412b4ac4eaac7dcd50cef820eae64))
- **Pixel buffer** rendering and enhanced diff engine ([`001783b`](https://github.com/Dicklesworthstone/opentui_rust/commit/001783ba1e19055a0256aae6576fb0b6502d37b3))
- **Terminal capability detection** heuristics for hyperlinks and synchronized output ([`5b14212`](https://github.com/Dicklesworthstone/opentui_rust/commit/5b14212d57d8b79ea0aa486fe6e9b8eb0b7e579b))
- Buffer scrolling and history ([`5afd61d`](https://github.com/Dicklesworthstone/opentui_rust/commit/5afd61db0d3f4bafa67db36fe986d5a2b395a9b3))
- Pixel rendering and unicode search utilities in buffer module ([`76ebf51`](https://github.com/Dicklesworthstone/opentui_rust/commit/76ebf512acfd8b763650c14067b3196c67de8a3d))

### Demo Showcase (2026-01-27 to 2026-01-28)

Full interactive demo binary (`demo_showcase`) demonstrating all OpenTUI capabilities.

- Layout engine, state machine, theme system, and input handling ([`ffed943`](https://github.com/Dicklesworthstone/opentui_rust/commit/ffed943cf47829ebef39203ea5deadfd3a7f1e6f))
- Sidebar navigation with selection highlighting ([`dacff5a`](https://github.com/Dicklesworthstone/opentui_rust/commit/dacff5a42265c2e9213eb6a94cd98fbf1d9c5eaf))
- Overlay system (help, command palette, tour) ([`b7ca067`](https://github.com/Dicklesworthstone/opentui_rust/commit/b7ca06774a7a5368d2d1f77a2dd515d589872d02))
- Tour mode driver with deterministic timing ([`66b145d`](https://github.com/Dicklesworthstone/opentui_rust/commit/66b145dd6cfe3c11c6e8e0badc4404e059bcab6b))
- Editor panel with syntax highlighting ([`4fba001`](https://github.com/Dicklesworthstone/opentui_rust/commit/4fba0013e75aadc5f74816d66621be14bc4179a7))
- Preview panel with PixelBuffer/Grayscale demos ([`e757fac`](https://github.com/Dicklesworthstone/opentui_rust/commit/e757fac8f2a672833804ca227e7e8e3787e4cf16))
- Inspector/debug panel (Ctrl+D) ([`4a29ae8`](https://github.com/Dicklesworthstone/opentui_rust/commit/4a29ae8d0f4fc722ae34c8bd92cb7b593f268b76))
- Toast notification system ([`5b19ace`](https://github.com/Dicklesworthstone/opentui_rust/commit/5b19ace8ab003c52f7605b83adca1f9405da77f3))
- Mouse click handling with hit test system ([`d6040b6`](https://github.com/Dicklesworthstone/opentui_rust/commit/d6040b6339e442a0b659d978e152dbf0dace063d))
- OSC 8 hyperlink integration in overlays ([`5c19f2e`](https://github.com/Dicklesworthstone/opentui_rust/commit/5c19f2e8fb99758381e35ef276b3a513657d53a0))
- Capability gating and resilience framework ([`6d625c7`](https://github.com/Dicklesworthstone/opentui_rust/commit/6d625c79391cca2cb8aa902beec5aa015e1897ec))
- Headless modes: `--headless-smoke` for CI, `--headless-dump-json` for snapshot testing ([`90af173`](https://github.com/Dicklesworthstone/opentui_rust/commit/90af173b2cbea8330145b5684529626cb5c552e4), [`06e2fe4`](https://github.com/Dicklesworthstone/opentui_rust/commit/06e2fe4461be1d1d32d0fcba1c7038ffa79dd9f3))
- 6 additional demo sections with extended keyboard navigation ([`be003a6`](https://github.com/Dicklesworthstone/opentui_rust/commit/be003a615566a04ab140091187ca0ef7fb9ae592))
- Scene rendering engine and snapshot infrastructure ([`e383568`](https://github.com/Dicklesworthstone/opentui_rust/commit/e383568b38f8d7c869429f47c541dc83a30f9256))
- E2E runner and verification scripts (`scripts/demo_showcase_verify.sh`, `scripts/demo_showcase_e2e_pty.sh`) ([`d71ced7`](https://github.com/Dicklesworthstone/opentui_rust/commit/d71ced7feb5a28c244e7fcb38eba3f41620e6488))

### Performance (2026-01-27)

- Eliminate heap allocations in ANSI hot paths ([`31834b8`](https://github.com/Dicklesworthstone/opentui_rust/commit/31834b83a9e055709d6c19b9bf98f4b437bc9666))
- Eliminate bounds checking overhead in hot-path buffer diff ([`8a120b6`](https://github.com/Dicklesworthstone/opentui_rust/commit/8a120b667bfb66a0c0a3d3df2614b3b616746687))
- ASCII fast paths in `draw_text()` ([`412b94b`](https://github.com/Dicklesworthstone/opentui_rust/commit/412b94b727f27463fe62ecdc3e33dbc13decb043), [`b020786`](https://github.com/Dicklesworthstone/opentui_rust/commit/b020786863e15ecd307891e25f1a60a158d77bf0))
- Optimized u32 decimal writing for cursor sequences, eliminating `write!` macro overhead ([`c2b4f06`](https://github.com/Dicklesworthstone/opentui_rust/commit/c2b4f065ddc1bd20df7a3b16b81a262e06ca0db1), [`1b67661`](https://github.com/Dicklesworthstone/opentui_rust/commit/1b67661f6b76244207cf2b46b8c852d9441297a3))
- O(1) lookup in `to_256_color` replacing linear search ([`5f06fa7`](https://github.com/Dicklesworthstone/opentui_rust/commit/5f06fa773e6c93bb8662ef33c2ea800405c86dc3))
- Reuse `BufferDiff` allocation between frames ([`28ae18b`](https://github.com/Dicklesworthstone/opentui_rust/commit/28ae18be9f9fae06116b5850e96520c8a0cda802))

### Security

- Prevent terminal escape sequence injection in OSC 8 URLs and `set_title` ([`3629826`](https://github.com/Dicklesworthstone/opentui_rust/commit/36298266214cb0157859e1d7c1d07b7e18269622))
- Filter C1 control characters in `set_title` ([`88fef41`](https://github.com/Dicklesworthstone/opentui_rust/commit/88fef412cd66b94dba69133ff8edce42ea6d574c))
- Correct UTF-8 handling in OSC 8 URL escaping ([`41ff973`](https://github.com/Dicklesworthstone/opentui_rust/commit/41ff97323ef517d601b8a06ad1d5861f2d39807c))
- Escape C1 control characters in OSC 8 URLs ([`fbfb74f`](https://github.com/Dicklesworthstone/opentui_rust/commit/fbfb74f88600ae3902d43a2d60cbc0b8490d1875))

### Safety and Overflow Protection

- Overflow protection for `PixelBuffer` and `HitGrid` dimensions ([`065c3c1`](https://github.com/Dicklesworthstone/opentui_rust/commit/065c3c1b8f82e4344492430d41ac739a0285694b))
- Width saturation in `GraphemeId` encoding ([`d36f0fd`](https://github.com/Dicklesworthstone/opentui_rust/commit/d36f0fdcf9d08b2eef7d8432c95e1c7c4f957f4e))
- Overflow protection for cell index calculations ([`ccf4adb`](https://github.com/Dicklesworthstone/opentui_rust/commit/ccf4adb800434883220846258645ec3dc2a8f18d))
- Checked arithmetic in `PixelBuffer::from_pixels` ([`e62f5a4`](https://github.com/Dicklesworthstone/opentui_rust/commit/e62f5a4e18ae28444a219f994ba4eea525e1aef2))
- Prevent integer overflow and unbounded paste buffer growth ([`e5e2cb6`](https://github.com/Dicklesworthstone/opentui_rust/commit/e5e2cb693fa7b126ebee4a040743e9ecf79f668a))
- Prevent division by zero for zero-dimension buffers ([`08f5dad`](https://github.com/Dicklesworthstone/opentui_rust/commit/08f5dadc1bde348fd163557181e7073d49198101))

### Bug Fixes

- Track orphaned graphemes to prevent pool memory leaks ([`92c29b7`](https://github.com/Dicklesworthstone/opentui_rust/commit/92c29b7183d3758cb0c3b13ee941f31f977bd5ea))
- Correct cursor positioning when continuation cells are skipped ([`8db7a03`](https://github.com/Dicklesworthstone/opentui_rust/commit/8db7a0366798256c5ce01db2ede1ff8866433436))
- Emit cursor home before diff rendering ([`8a39255`](https://github.com/Dicklesworthstone/opentui_rust/commit/8a39255870578ba5598cd6367e096f23bbfe3849))
- Handle Tab/Enter/LF as proper key codes instead of Ctrl+letter ([`de9baa0`](https://github.com/Dicklesworthstone/opentui_rust/commit/de9baa0e6a29af909ef48767a9ab994a9e213171))
- Improve input parser robustness ([`c22c112`](https://github.com/Dicklesworthstone/opentui_rust/commit/c22c112bea5520249483d5fd92725015adec4dfa))
- Handle negative hue values in `from_hsv` ([`a93055f`](https://github.com/Dicklesworthstone/opentui_rust/commit/a93055f78d7d1bdc06e1255cbbda18ce915662ae))
- Prevent division-by-zero and fix box title positioning ([`d2a3146`](https://github.com/Dicklesworthstone/opentui_rust/commit/d2a314606fb640e5c9237dd7721d1e2c83ee77c1))
- Improve drawing bounds checking ([`c552232`](https://github.com/Dicklesworthstone/opentui_rust/commit/c5522327f1b87e1bf3c4efd4e16278bfb0eb7426))
- Pooled blending for wide character rendering ([`066ea9b`](https://github.com/Dicklesworthstone/opentui_rust/commit/066ea9bd56ac3a0897b0b91a0137a20c436a5bf0))
- Cell blending metadata and paste buffering ([`2c7262f`](https://github.com/Dicklesworthstone/opentui_rust/commit/2c7262f57dfbac7ffcc9266dd9d65a399250613d))
- Bounds validation for tokens and negative buffer coordinates ([`95778b6`](https://github.com/Dicklesworthstone/opentui_rust/commit/95778b617f8640a9f5d78d716fa6abe120ef2b48))
- Numerical stability and edge case handling improvements ([`4b55a32`](https://github.com/Dicklesworthstone/opentui_rust/commit/4b55a32edad611958f7de5647e67cbb02847c395))
- Cross-platform `suseconds_t` cast for macOS compatibility ([`3a491ce`](https://github.com/Dicklesworthstone/opentui_rust/commit/3a491ce63dac39cc5913d36b02f5abe9f10c2301))
- Prevent underflow in easing animation track drawing ([`14caafe`](https://github.com/Dicklesworthstone/opentui_rust/commit/14caafe13d897123744f54e345c433d563e029e3))

### Testing Infrastructure

- Comprehensive unit test suite with benchmarks and utility scripts ([`1036f03`](https://github.com/Dicklesworthstone/opentui_rust/commit/1036f03b78f67f6b9ac5e15e50aabad4fa8c109c))
- Insta snapshot fixtures for ANSI sequence tests ([`9428a0e`](https://github.com/Dicklesworthstone/opentui_rust/commit/9428a0ed3c6d716b885bd4f59781f49bbe097f43))
- **PTY+VT100 visual testing infrastructure** with multi-terminal support ([`bac7f94`](https://github.com/Dicklesworthstone/opentui_rust/commit/bac7f945b883df06c19d8450117e2e7252358878), [`b7ac2ca`](https://github.com/Dicklesworthstone/opentui_rust/commit/b7ac2ca9ce963f8d64a6c87d5a7ff64522f78839))
- Input simulation module for E2E testing ([`57ad5b9`](https://github.com/Dicklesworthstone/opentui_rust/commit/57ad5b94f9576375158297913af05545c0d54e87))
- Sequence analysis module for PTY tests ([`f67bd97`](https://github.com/Dicklesworthstone/opentui_rust/commit/f67bd97e999c8d1daefc3de5f3e56d0538b67ec7))
- 95+ new unit tests in comprehensive expansion ([`b80a587`](https://github.com/Dicklesworthstone/opentui_rust/commit/b80a58714e94b943d3eafa1bf2c9c315ee8932bf))
- 30 edge case tests for buffer operations ([`9f18216`](https://github.com/Dicklesworthstone/opentui_rust/commit/9f18216e5c6f7ca8e21b53a8b8175b1e46a41649))
- Memory safety regression tests ([`ed7062c`](https://github.com/Dicklesworthstone/opentui_rust/commit/ed7062c4a569b2c5e7824470a2110a4e5ac6b81b))
- E2E test enhancements and property-based tests ([`a8fe695`](https://github.com/Dicklesworthstone/opentui_rust/commit/a8fe695e5435d74393a56164891e1a376979d243))
- Comprehensive EditBuffer edge case and stress tests ([`04063a0`](https://github.com/Dicklesworthstone/opentui_rust/commit/04063a03caf1a13367d7502df25a5ac32ecd2aac))
- Tour determinism regression tests ([`e3fd923`](https://github.com/Dicklesworthstone/opentui_rust/commit/e3fd923ac35376c4e8d5b211b9203dd323ebd886))
- Comprehensive timing metrics for E2E PTY tests ([`44cd8db`](https://github.com/Dicklesworthstone/opentui_rust/commit/44cd8dbc7053e4a39728afc93f75e4367f783736))
- Layered gradient hit test ([`f4dc155`](https://github.com/Dicklesworthstone/opentui_rust/commit/f4dc155be402d9e5dcc14828f1e2ff388e81b1d1))

### Benchmarks

- Realistic application workload benchmarks ([`0444a0b`](https://github.com/Dicklesworthstone/opentui_rust/commit/0444a0b2f86c6eb5e75b186f21c635ed9179ec10))
- ANSI generation and render cycle benchmarks ([`8b5f71e`](https://github.com/Dicklesworthstone/opentui_rust/commit/8b5f71e8834540dacf66e6439fb15b27c1303505))
- Color creation method benchmarks ([`fbc3c7d`](https://github.com/Dicklesworthstone/opentui_rust/commit/fbc3c7ded08264eefda524c94b924264daa64ba2))
- Input parsing benchmarks ([`d82381d`](https://github.com/Dicklesworthstone/opentui_rust/commit/d82381de758068d48fe212c3bea39b42db834be0))
- Unicode benchmarks with ZWJ and box-drawing tests ([`55b3437`](https://github.com/Dicklesworthstone/opentui_rust/commit/55b343709bf9ac8d86c0121a2b3bf41075085115))

### CI/CD

- GitHub Actions workflows and Dependabot configuration ([`4981f90`](https://github.com/Dicklesworthstone/opentui_rust/commit/4981f90fa63fbee59eeef8b1800b5e25ffd85f25))
- E2E tests in CI with failure artifact capture ([`4d335cd`](https://github.com/Dicklesworthstone/opentui_rust/commit/4d335cdced4aaaff853209b042e878b0421c3048))
- Dedicated PTY tests workflow with retry logic ([`6a7f745`](https://github.com/Dicklesworthstone/opentui_rust/commit/6a7f74521648ddb50e6400d5179a6a9cf561c14e))
- Coverage gates with module-specific thresholds ([`6c7a29a`](https://github.com/Dicklesworthstone/opentui_rust/commit/6c7a29aa0afd7aebedbc6701498c48bbb3e80600))
- Test coverage and performance regression infrastructure ([`2b23a40`](https://github.com/Dicklesworthstone/opentui_rust/commit/2b23a400a1de8fa97ed529c49e665a0e1b023354))

### Packaging and Docs

- Crate renamed from internal name to `opentui_rust` ([`a9a1a64`](https://github.com/Dicklesworthstone/opentui_rust/commit/a9a1a6423aac59b8beea7f5c55521294e626a18c))
- Expanded crate docs and crates.io metadata ([`a548ac5`](https://github.com/Dicklesworthstone/opentui_rust/commit/a548ac558aee29fd263a871faa9c7a6dc6e3ed2e))
- Comprehensive public API documentation ([`38ee7e7`](https://github.com/Dicklesworthstone/opentui_rust/commit/38ee7e71e1f7bc973a3760dc7c536f3a4b5e7ce5))
- Hello world and full editor examples ([`d86b678`](https://github.com/Dicklesworthstone/opentui_rust/commit/d86b67821119c739dca8832a7120ff04d8a236ca))
- Zig-to-Rust feature parity verification documentation ([`fb4b5fa`](https://github.com/Dicklesworthstone/opentui_rust/commit/fb4b5fa7b382c4b264c207ee8a7c72c8a888d2aa))
- Demo Showcase README section with keybindings and CLI flags ([`962c8bf`](https://github.com/Dicklesworthstone/opentui_rust/commit/962c8bf84f5ba7f8bd543f8b5ab5aa74f0531bb4))

### 256-Color Improvements

- Improved 256-color grayscale mapping accuracy ([`dd2f948`](https://github.com/Dicklesworthstone/opentui_rust/commit/dd2f9485edab870549fafdf86271823359dba5a9))

---

## Pre-release Development Timeline

| Date | Milestone |
|------|-----------|
| 2026-01-19 | Project initialized; full Zig-to-Rust port of core engine (color, buffer, cell, unicode, ansi, text, terminal, input, renderer) in 16 commits |
| 2026-01-21 | Buffer/diff rendering improvements, public API documentation, benchmark expansion, MIT license added |
| 2026-01-24 | Syntax highlighting token model, cell blending fixes, GraphemePool spec, link IDs packed into TextAttributes |
| 2026-01-25 | Threaded renderer, feature parity expansion, comprehensive benchmarks (ANSI, color, input, unicode, application workloads), E2E tests in CI |
| 2026-01-26--27 | Security hardening (OSC 8 injection, C1 filtering), overflow protection, performance hot-path optimization, demo_showcase binary build-out |
| 2026-01-28 | Safety fixes (overflow, division-by-zero, memory leaks), 95+ new tests, PTY visual testing, comprehensive E2E suites, GraphemePool API expansion |
| 2026-01-29 | Threaded rendering improvements, clippy warning cleanup (24.5% reduction), cursor position fix |
| 2026-01-30 | GraphemePool compaction/statistics, BiDi text resolution (UAX #9), NFC/NFD normalization, character width overrides, demo sections expansion |
| 2026-01-31 | Layer-based hit testing and transparency compositing |
| 2026-02-02 | Dependency updates (insta 1.46.3) |
| 2026-02-04 | Crate renamed to `opentui_rust`, published to crates.io, dirty regions, debug overlay |
| 2026-02-15 | **v0.2.0 tagged and released on GitHub** |
| 2026-02-21--22 | License updated to MIT + OpenAI/Anthropic Rider, **v0.2.1 tagged** |

---

[Unreleased]: https://github.com/Dicklesworthstone/opentui_rust/compare/v0.2.1...HEAD
[v0.2.1]: https://github.com/Dicklesworthstone/opentui_rust/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/Dicklesworthstone/opentui_rust/releases/tag/v0.2.0
