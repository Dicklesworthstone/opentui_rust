//! Input parsing for terminal events.
//!
//! This module provides ANSI sequence parsing for keyboard, mouse, and other
//! terminal events. It supports both legacy VT sequences and modern extensions
//! like SGR mouse encoding.

mod event;
mod keyboard;
mod parser;

pub use event::{Event, FocusEvent, PasteEvent, ResizeEvent};
pub use keyboard::{KeyCode, KeyEvent, KeyModifiers};
pub use parser::{InputParser, ParseError, ParseResult};

// Re-export mouse types from terminal module (they're re-exported there)
pub use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};
