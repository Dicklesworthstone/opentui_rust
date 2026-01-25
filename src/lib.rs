//! `OpenTUI` - High-performance terminal UI library
//!
//! A Rust port of the `OpenTUI` Zig core, providing efficient cell-based
//! rendering with alpha blending, scissoring, and styled text support.

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
pub use renderer::{RenderStats, Renderer, RendererOptions};
pub use terminal::{RawModeGuard, Terminal, enable_raw_mode, is_tty, terminal_size};
pub use text::{EditBuffer, EditorView, TextBuffer, TextBufferView, VisualCursor, WrapMode};
pub use unicode::{WidthMethod, set_width_method};
