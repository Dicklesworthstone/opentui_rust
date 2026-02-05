//! `OpenTUI` - High-performance terminal UI rendering engine
//!
//! OpenTUI is a Rust port of the OpenTUI Zig core. It is a rendering engine,
//! not a framework: you get precise control over buffers, cells, colors, and
//! text without a prescribed widget tree or event loop.
//!
//! # How This Crate Fits In
//!
//! This repository is a single-crate system. The `opentui` crate is the core
//! engine that applications build on. You provide your own application loop
//! and input handling; OpenTUI provides the rendering, text, and terminal
//! primitives that make that loop fast and correct.
//!
//! # Architecture At A Glance
//!
//! - `renderer`: Double-buffered rendering, diff detection, hit testing
//! - `buffer`: Cell grids, scissor clipping, opacity stacking, compositing
//! - `cell` / `style` / `color`: The core visual primitives (Cell, Style, Rgba)
//! - `ansi`: ANSI escape emission with state tracking for minimal output
//! - `terminal`: Raw mode and capability detection (mouse, sync output, color)
//! - `text`: Rope-backed text buffers, editing, wrapping, and views
//! - `unicode`: Grapheme iteration and display-width calculation
//! - `input`: Parser that turns raw terminal bytes into structured events
//! - `highlight`: Tokenization and theming for syntax-highlighted buffers
//! - `grapheme_pool` / `link`: Interned graphemes and OSC 8 hyperlink storage
//! - `event` / `error`: Lightweight callbacks and error types
//!
//! # Data Flow
//!
//! ```text
//! App draws into OptimizedBuffer
//!     -> Renderer diffs back vs front buffers
//!     -> AnsiWriter emits minimal ANSI sequences
//!     -> Terminal writes to stdout (optionally with sync output)
//! ```
//!
//! This flow is intentionally simple: the rendering engine owns output timing
//! and correctness, while your application owns structure and behavior.

// Crate-level lint configuration
#![warn(unsafe_code)] // Unsafe code needs justification (required for termios FFI)
#![allow(dead_code)] // Public API functions not yet used internally
#![allow(clippy::cast_possible_truncation)] // Intentional coordinate casts
#![allow(clippy::cast_sign_loss)] // Intentional coordinate conversions
#![allow(clippy::cast_precision_loss)] // Intentional for color math
#![allow(clippy::cast_possible_wrap)] // Intentional coordinate conversions
#![allow(clippy::module_name_repetitions)] // Allow Cell::CellContent etc
#![allow(clippy::struct_excessive_bools)] // Terminal state needs multiple flags
#![allow(clippy::missing_errors_doc)] // Docs WIP
#![allow(clippy::missing_panics_doc)] // Docs WIP
#![allow(clippy::missing_const_for_fn)] // Many functions could be const, not critical
#![allow(clippy::doc_markdown)] // Allow technical names without backticks
#![allow(clippy::use_self)] // Allow explicit type names in impl blocks
#![allow(clippy::format_push_string)] // format! with push_str is fine
#![allow(clippy::needless_pass_by_value)] // Allow pass by value for small Copy types
#![allow(clippy::suboptimal_flops)] // Standard math notation is clearer than mul_add
#![allow(clippy::branches_sharing_code)] // Code clarity over DRY in branching
#![allow(clippy::inherent_to_string)] // to_string methods are convenient
#![allow(clippy::should_implement_trait)] // from_str naming is intentional
#![allow(clippy::collapsible_if)] // Sometimes nested ifs are clearer
#![allow(clippy::cast_lossless)] // as casts are fine for primitive widening
#![allow(clippy::items_after_statements)] // Common pattern in tests
#![allow(clippy::redundant_clone)] // Clones in tests for clarity are fine
#![allow(clippy::semicolon_if_nothing_returned)] // Style preference
#![allow(clippy::needless_collect)] // Collect for assertions is clear

pub mod ansi;
pub mod buffer;
pub mod cell;
pub mod color;
pub mod error;
pub mod event;
pub mod grapheme_pool;
pub mod highlight;
pub mod input;
pub mod link;
pub mod renderer;
pub mod style;
pub mod terminal;
pub mod text;
pub mod unicode;

// Re-export core types at crate root
pub use cell::{Cell, CellContent, GraphemeId};
pub use color::Rgba;
pub use error::{Error, Result};
pub use event::{LogLevel, emit_event, emit_log, set_event_callback, set_log_callback};
pub use grapheme_pool::GraphemePool;
pub use link::LinkPool;
pub use style::{Style, TextAttributes};

// Re-export input types
pub use input::{Event, InputParser, KeyCode, KeyEvent, KeyModifiers, MouseEvent};

// Re-export ANSI types
pub use ansi::ColorMode;

// Re-export commonly used types
pub use buffer::OptimizedBuffer;
pub use highlight::{HighlightedBuffer, Theme, ThemeRegistry, Token, TokenKind, TokenizerRegistry};
pub use renderer::{Rect, RenderStats, Renderer, RendererOptions};
pub use terminal::{
    Capabilities, ColorSupport, RawModeGuard, Terminal, enable_raw_mode, is_tty, terminal_size,
};
pub use text::{EditBuffer, EditorView, TextBuffer, TextBufferView, VisualCursor, WrapMode};
pub use unicode::{WidthMethod, set_width_method};
