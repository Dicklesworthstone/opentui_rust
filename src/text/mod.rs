//! Text storage and editing with styled segments.
//!
//! This module provides rope-backed text buffers for efficient editing of
//! large documents.
//!
//! # Grapheme Pool Policy
//!
//! This module's public API uses `&str` for text content, keeping grapheme pool
//! details internal. Multi-codepoint grapheme clusters (emoji, ZWJ sequences) are
//! handled transparently during rendering using [`GraphemeId::placeholder`] for
//! width-aware layout without pool allocation. Users needing direct grapheme pool
//! access should use [`GraphemePool`] and [`GraphemeId`] from the crate root.
//!
//! Key types:
//!
//! - [`TextBuffer`]: Styled text storage with syntax highlighting support
//! - [`EditBuffer`]: Editable buffer with cursor movement and undo/redo
//! - [`EditorView`]: Visual rendering with line numbers and selection
//! - [`TextBufferView`]: Viewport configuration with wrapping modes
//!
//! # Examples
//!
//! ## Basic Text Buffer
//!
//! ```
//! use opentui::TextBuffer;
//!
//! let buffer = TextBuffer::with_text("Hello, world!");
//! assert_eq!(buffer.len_chars(), 13);
//! assert_eq!(buffer.len_lines(), 1);
//! ```
//!
//! ## Editable Buffer with Undo
//!
//! ```
//! use opentui::EditBuffer;
//!
//! let mut editor = EditBuffer::new();
//! editor.insert("Hello");
//! editor.commit(); // Create undo checkpoint
//! editor.insert(" World");
//! editor.commit();
//! assert_eq!(editor.text(), "Hello World");
//!
//! // Undo the last insert
//! editor.undo();
//! assert_eq!(editor.text(), "Hello");
//!
//! // Redo brings it back
//! editor.redo();
//! assert_eq!(editor.text(), "Hello World");
//! ```

mod buffer;
mod edit;
mod editor;
mod rope;
mod segment;
mod view;

pub use buffer::TextBuffer;
pub use edit::EditBuffer;
pub use editor::{EditorView, VisualCursor};
pub use rope::RopeWrapper;
pub use segment::StyledSegment;
pub use view::{
    LineInfo, LocalSelection, Selection, TextBufferView, TextMeasure, Viewport, WrapMode,
};
