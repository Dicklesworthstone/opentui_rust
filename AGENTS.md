# AGENTS.md — OpenTUI (opentui_rust)

> Guidelines for AI coding agents working in this Rust codebase.

---

## RULE NUMBER 1: NO FILE DELETION

**YOU ARE NEVER ALLOWED TO DELETE A FILE WITHOUT EXPRESS PERMISSION.** Even a new file that you yourself created, such as a test code file. You have a horrible track record of deleting critically important files or otherwise throwing away tons of expensive work. As a result, you have permanently lost any and all rights to determine that a file or folder should be deleted.

**YOU MUST ALWAYS ASK AND RECEIVE CLEAR, WRITTEN PERMISSION BEFORE EVER DELETING A FILE OR FOLDER OF ANY KIND.**

---

## Irreversible Git & Filesystem Actions — DO NOT EVER BREAK GLASS

1. **Absolutely forbidden commands:** `git reset --hard`, `git clean -fd`, `rm -rf`, or any command that can delete or overwrite code/data must never be run unless the user explicitly provides the exact command and states, in the same message, that they understand and want the irreversible consequences.
2. **No guessing:** If there is any uncertainty about what a command might delete or overwrite, stop immediately and ask the user for specific approval. "I think it's safe" is never acceptable.
3. **Safer alternatives first:** When cleanup or rollbacks are needed, request permission to use non-destructive options (`git status`, `git diff`, `git stash`, copying to backups) before ever considering a destructive command.
4. **Mandatory explicit plan:** Even after explicit user authorization, restate the command verbatim, list exactly what will be affected, and wait for a confirmation that your understanding is correct. Only then may you execute it—if anything remains ambiguous, refuse and escalate.
5. **Document the confirmation:** When running any approved destructive command, record (in the session notes / final response) the exact user text that authorized it, the command actually run, and the execution time. If that record is absent, the operation did not happen.

---

## Toolchain: Rust & Cargo

We only use **Cargo** in this project, NEVER any other package manager.

- **Edition:** Rust 2024 (nightly required — see `rust-toolchain.toml`)
- **MSRV:** 1.85
- **Dependency versions:** Explicit versions for stability
- **Configuration:** Cargo.toml only
- **Unsafe code:** Warn level (`#![warn(unsafe_code)]`) — required for libc/termios FFI in `src/terminal/raw.rs`

### Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ropey` | Rope data structure for efficient text editing |
| `unicode-segmentation` | Grapheme cluster iteration |
| `unicode-width` | Display width calculation |
| `bitflags` | TextAttributes bitflags (bold, italic, etc.) |
| `libc` | Terminal raw mode via termios FFI |
| `criterion` | Benchmarking (dev-dependency) |

### Release Profile

The release build should optimize for both speed and size:

```toml
[profile.release]
opt-level = "z"     # Optimize for size (lean library)
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
panic = "abort"     # Smaller binary, no unwinding overhead
strip = true        # Remove debug symbols
```

---

## Project Semantics (OpenTUI)

This is a **Rust port of the OpenTUI Zig core** (~15,900 LOC). It's a high-performance terminal UI **rendering engine** (not a framework). Keep these design principles intact:

### Core Principles

1. **Rendering engine, not framework:** Provides primitives (buffers, cells, colors, text) without forcing application structure. No widget trees, no layout systems, no event loops.

2. **Correctness over convenience:**
   - **Real alpha blending** using Porter-Duff "over" compositing
   - **Proper grapheme handling** via unicode-segmentation
   - **Accurate character widths** via unicode-width
   - **Immutable rope** for text that doesn't corrupt on edits

3. **Performance by default:**
   - **Diff rendering:** Only changed cells generate ANSI output
   - **Synchronized output:** Uses `\x1b[?2026h` to eliminate flicker
   - **Zero allocations** on hot paths (cell updates, blending)
   - **SIMD-friendly** memory layout (contiguous cell arrays)

4. **Terminal respect:**
   - Automatic cleanup on drop (restores cursor, exits alt screen)
   - Proper mouse protocol handling (SGR and X11)
   - True color support with graceful fallback

### Architecture Overview

```
Application
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│                        OpenTUI                               │
├─────────────────────────────────────────────────────────────┤
│  Renderer ──▶ Buffer ◀── Text                               │
│  • Double buf    • Cells       • Rope                       │
│  • Diff detect   • Scissor     • Segments                   │
│  • Hit grid      • Opacity     • Edit/Undo                  │
│       │              │              │                        │
│       ▼              ▼              ▼                        │
│  Terminal       Cell          Unicode                        │
│  • ANSI codes   • Char/Graph  • Graphemes                   │
│  • Mouse        • Style       • Width calc                  │
│  • Cursor       • Blending    • Segmentation                │
└─────────────────────────────────────────────────────────────┘
    │
    ▼
stdout (ANSI TTY)
```

### Module Structure

| Directory | Purpose |
|-----------|---------|
| `src/lib.rs` | Public API exports and crate-level lints |
| `src/color.rs` | RGBA type, Porter-Duff blending, HSV/hex conversions |
| `src/style.rs` | TextAttributes bitflags, Style builder |
| `src/cell.rs` | Cell type, CellContent enum (char/grapheme/empty/continuation) |
| `src/ansi/` | ANSI escape sequence generation and buffered output |
| `src/buffer/` | OptimizedBuffer with scissor stack, opacity stack, drawing ops |
| `src/text/` | Rope-backed TextBuffer, EditBuffer with cursor/undo, views |
| `src/renderer/` | Double-buffered rendering with diff detection, hit grid |
| `src/terminal/` | Raw mode via termios FFI, capabilities detection, cursor/mouse |
| `src/unicode/` | Grapheme iteration, display width calculation |
| `src/input/` | Input parsing (keyboard, mouse, escape sequences) |
| `src/highlight/` | Syntax highlighting infrastructure |
| `src/link.rs` | Hyperlink pool (OSC 8) |
| `src/event.rs` | Event emission and logging callbacks |
| `src/error.rs` | Error types and Result alias |

### Critical Algorithms

1. **Alpha Blending (Porter-Duff "over"):**
   ```rust
   // src/color.rs - blend_over method
   result.r = src.r * src.a + dst.r * dst.a * (1.0 - src.a)
   result.a = src.a + dst.a * (1.0 - src.a)
   // Then divide RGB by result.a to un-premultiply
   ```

2. **Diff Rendering:** Compare front and back buffers cell-by-cell; only emit ANSI sequences for changed cells.

3. **Scissor Clipping:** Maintain a stack of clip rectangles; all drawing operations check `is_within_scissor()` before modifying cells.

4. **SGR vs X11 Mouse Protocol:** Parser must distinguish between CSI < ... M (SGR) and CSI M <btn><x><y> (X11) formats.

---

## Code Editing Discipline

### No Script-Based Changes

**NEVER** run a script that processes/changes code files in this repo. Brittle regex-based transformations create far more problems than they solve.

- **Always make code changes manually**, even when there are many instances
- For many simple changes: use parallel subagents
- For subtle/complex changes: do them methodically yourself

### No File Proliferation

If you want to change something or add a feature, **revise existing code files in place**.

**NEVER** create variations like:
- `bufferV2.rs`
- `cell_improved.rs`
- `renderer_enhanced.rs`

New files are reserved for **genuinely new functionality** that makes zero sense to include in any existing file. The bar for creating new files is **incredibly high**.

---

## Backwards Compatibility

We do not care about backwards compatibility—we're in early development with no users. We want to do things the **RIGHT** way with **NO TECH DEBT**.

- Never create "compatibility shims"
- Never create wrapper functions for deprecated APIs
- Just fix the code directly

---

## Compiler Checks (CRITICAL)

**After any substantive code changes, you MUST verify no errors were introduced:**

```bash
# Check for compiler errors and warnings
cargo check --all-targets

# Check for clippy lints (pedantic + nursery are enabled)
cargo clippy --all-targets -- -D warnings

# Verify formatting
cargo fmt --check
```

If you see errors, **carefully understand and resolve each issue**. Read sufficient context to fix them the RIGHT way.

### Clippy Configuration

This project enables `pedantic` and `nursery` lints. The following allows are configured at module level where justified:

| Lint | Where Allowed | Reason |
|------|---------------|--------|
| `unsafe_code` | `src/terminal/raw.rs` | FFI with libc for termios |
| `too_many_arguments` | parser functions | Complex parsing state |
| `match_same_arms` | match expressions | Explicit disambiguation |
| `option_if_let_else` | text editing | Clarity over conciseness |

When adding new allow directives, include a comment explaining why.

---

## Testing

### Unit Tests

The test suite includes 115+ tests covering all functionality:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test module
cargo test color
cargo test cell
cargo test buffer
cargo test unicode
cargo test text
cargo test renderer
cargo test input
```

### Test Categories

| Module | Focus Areas |
|--------|-------------|
| `color` | RGBA blending, hex/HSV conversion, clamping |
| `cell` | Content types, style application |
| `buffer` | Scissor clipping, opacity, drawing ops |
| `unicode` | Grapheme iteration, width methods |
| `text` | Rope operations, cursor movement, undo/redo |
| `renderer` | Diff detection |
| `input` | Keyboard/mouse parsing, escape sequences |

### Performance Testing

```bash
# Run benchmarks
cargo bench --bench buffer

# View HTML report
open target/criterion/report/index.html
```

Key benchmarks:
- `buffer_clear` - Full buffer clear operations
- `buffer_draw_text` - Text rendering performance
- `buffer_blend` - Alpha blending throughput

---

## CI/CD Pipeline

### Jobs Overview

| Job | Trigger | Purpose | Blocking |
|-----|---------|---------|----------|
| `check` | PR, push | Format, clippy, tests | Yes |
| `coverage` | PR, push | Coverage thresholds | Yes |
| `benchmarks` | push to main | Performance regression | Warn only |

### Check Job

Runs format, clippy, and unit tests:
- `cargo fmt --check` - Code formatting
- `cargo clippy --all-targets -- -D warnings` - Lints (pedantic + nursery enabled)
- `cargo test` - Full test suite

### Coverage Targets

| Module | Target |
|--------|--------|
| Overall | ≥ 70% |
| `src/color.rs` | ≥ 90% |
| `src/buffer/` | ≥ 80% |
| `src/text/` | ≥ 75% |

---

## Performance Requirements

Terminal rendering happens on every frame. Performance is critical:

- **Cell operations:** Must be zero-allocation
- **Blending:** Hot path, no branching on happy path
- **Diff detection:** O(n) where n = changed cells, not total cells
- **Text rendering:** Grapheme iteration must not allocate per-grapheme

### Benchmarking Guidelines

When modifying hot paths, run benchmarks before and after:

```bash
# Baseline
git stash
cargo bench --bench buffer -- --save-baseline before

# With changes
git stash pop
cargo bench --bench buffer -- --save-baseline after

# Compare
cargo bench --bench buffer -- --load-baseline before --baseline after
```

---

## Unsafe Code Policy

Unsafe code is **warned** (not forbidden) because terminal control requires FFI:

### Allowed Unsafe

1. **termios FFI** (`src/terminal/raw.rs`):
   - `tcgetattr` / `tcsetattr` for raw mode
   - `isatty` for TTY detection
   - `ioctl` with `TIOCGWINSZ` for terminal size

### Requirements for Unsafe

- Module-level `#![allow(unsafe_code)]`
- Safety comment explaining why it's safe
- Minimal scope (wrap in safe functions)

---

## Third-Party Library Usage

If you aren't 100% sure how to use a third-party library, **SEARCH ONLINE** to find the latest documentation and mid-2025 best practices.

Key libraries to know:
- **ropey:** Rope data structure for text editing
- **unicode-segmentation:** `.graphemes(true)` for cluster iteration
- **unicode-width:** `.width()` for display width

---

## MCP Agent Mail — Multi-Agent Coordination

A mail-like layer that lets coding agents coordinate asynchronously via MCP tools and resources. Provides identities, inbox/outbox, searchable threads, and advisory file reservations with human-auditable artifacts in Git.

### Why It's Useful

- **Prevents conflicts:** Explicit file reservations (leases) for files/globs
- **Token-efficient:** Messages stored in per-project archive, not in context
- **Quick reads:** `resource://inbox/...`, `resource://thread/...`

### Same Repository Workflow

1. **Register identity:**
   ```
   ensure_project(project_key=<abs-path>)
   register_agent(project_key, program, model)
   ```

2. **Reserve files before editing:**
   ```
   file_reservation_paths(project_key, agent_name, ["src/**"], ttl_seconds=3600, exclusive=true)
   ```

3. **Communicate with threads:**
   ```
   send_message(..., thread_id="FEAT-123")
   fetch_inbox(project_key, agent_name)
   acknowledge_message(project_key, agent_name, message_id)
   ```

4. **Quick reads:**
   ```
   resource://inbox/{Agent}?project=<abs-path>&limit=20
   resource://thread/{id}?project=<abs-path>&include_bodies=true
   ```

### Macros vs Granular Tools

- **Prefer macros for speed:** `macro_start_session`, `macro_prepare_thread`, `macro_file_reservation_cycle`, `macro_contact_handshake`
- **Use granular tools for control:** `register_agent`, `file_reservation_paths`, `send_message`, `fetch_inbox`, `acknowledge_message`

### Common Pitfalls

- `"from_agent not registered"`: Always `register_agent` in the correct `project_key` first
- `"FILE_RESERVATION_CONFLICT"`: Adjust patterns, wait for expiry, or use non-exclusive reservation
- **Auth errors:** If JWT+JWKS enabled, include bearer token with matching `kid`

---

## bv — Graph-Aware Triage Engine

bv is a graph-aware triage engine for Beads projects (`.beads/beads.jsonl`). It computes PageRank, betweenness, critical path, cycles, HITS, eigenvector, and k-core metrics deterministically.

**Scope boundary:** bv handles *what to work on* (triage, priority, planning). For agent-to-agent coordination (messaging, work claiming, file reservations), use MCP Agent Mail.

**CRITICAL: Use ONLY `--robot-*` flags. Bare `bv` launches an interactive TUI that blocks your session.**

### The Workflow: Start With Triage

**`bv --robot-triage` is your single entry point.** It returns:
- `quick_ref`: at-a-glance counts + top 3 picks
- `recommendations`: ranked actionable items with scores, reasons, unblock info
- `quick_wins`: low-effort high-impact items
- `blockers_to_clear`: items that unblock the most downstream work
- `project_health`: status/type/priority distributions, graph metrics
- `commands`: copy-paste shell commands for next steps

```bash
bv --robot-triage        # THE MEGA-COMMAND: start here
bv --robot-next          # Minimal: just the single top pick + claim command
```

---

## UBS — Ultimate Bug Scanner

**Golden Rule:** `ubs <changed-files>` before every commit. Exit 0 = safe. Exit >0 = fix & re-run.

### Commands

```bash
ubs file.rs file2.rs                    # Specific files (< 1s) — USE THIS
ubs $(git diff --name-only --cached)    # Staged files — before commit
ubs --only=rust,toml src/               # Language filter (3-5x faster)
ubs --ci --fail-on-warning .            # CI mode — before PR
ubs .                                   # Whole project (ignores target/, Cargo.lock)
```

### Output Format

```
⚠️  Category (N errors)
    file.rs:42:5 – Issue description
    💡 Suggested fix
Exit code: 1
```

Parse: `file:line:col` → location | 💡 → how to fix | Exit 0/1 → pass/fail

---

## ast-grep vs ripgrep

**Use `ast-grep` when structure matters.** It parses code and matches AST nodes, ignoring comments/strings, and can **safely rewrite** code.

**Use `ripgrep` when text is enough.** Fastest way to grep literals/regex.

### Rule of Thumb

- Need correctness or **applying changes** → `ast-grep`
- Need raw speed or **hunting text** → `rg`
- Often combine: `rg` to shortlist files, then `ast-grep` to match/modify

### Rust Examples

```bash
# Find structured code (ignores comments)
ast-grep run -l Rust -p 'fn $NAME($$$ARGS) -> $RET { $$$BODY }'

# Find all unwrap() calls
ast-grep run -l Rust -p '$EXPR.unwrap()'

# Quick textual hunt
rg -n 'blend_over' -t rust

# Combine speed + precision
rg -l -t rust 'unwrap\(' | xargs ast-grep run -l Rust -p '$X.unwrap()' --json
```

---

## Beads Workflow Integration

This project uses [beads_rust](https://github.com/Dicklesworthstone/beads_rust) (`br`) for issue tracking. Issues are stored in `.beads/` and tracked in git.

**Important:** `br` is non-invasive—it NEVER executes git commands. After `br sync --flush-only`, you must manually run `git add .beads/ && git commit`.

### Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
br ready              # Show issues ready to work (no blockers)
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br create --title="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason="Completed"
br close <id1> <id2>  # Close multiple issues at once
br sync --flush-only  # Export to JSONL (NO git operations)
```

### Workflow Pattern

1. **Start**: Run `br ready` to find actionable work
2. **Claim**: Use `br update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `br close <id>`
5. **Sync**: Run `br sync --flush-only` then manually commit

---

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   br sync --flush-only    # Export beads to JSONL (no git ops)
   git add .beads/         # Stage beads changes
   git add <other files>   # Stage code changes
   git commit -m "..."     # Commit everything
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

---

Note for Codex/GPT-5.2:

You constantly bother me and stop working with concerned questions that look similar to this:

```
Unexpected changes (need guidance)

- Working tree still shows edits I did not make in Cargo.toml, Cargo.lock, src/text/buffer.rs. Please advise whether to keep/commit/revert these before any further work. I did not touch them.

Next steps (pick one)

1. Decide how to handle the unrelated modified files above so we can resume cleanly.
2. Triage issues.
3. If you want a full suite run later, fix clippy blockers and re‑run cargo test --all.
```

NEVER EVER DO THAT AGAIN. The answer is literally ALWAYS the same: those are changes created by the potentially dozen of other agents working on the project at the same time. This is not only a common occurence, it happens multiple times PER MINUTE. The way to deal with it is simple: you NEVER, under ANY CIRCUMSTANCE, stash, revert, overwrite, or otherwise disturb in ANY way the work of other agents. Just treat those changes identically to changes that you yourself made. Just fool yourself into think YOU made the changes and simply don't recall it for some reason.
