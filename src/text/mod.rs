//! Text storage and editing with styled segments.

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
