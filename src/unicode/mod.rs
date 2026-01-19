//! Unicode utilities for grapheme handling and display width.

mod grapheme;
mod width;

pub use grapheme::{
    GraphemeInfo, GraphemeIterator, grapheme_indices, grapheme_info, graphemes, is_ascii_only,
};
pub use width::{
    WidthMethod, display_width, display_width_char, display_width_char_with_method,
    display_width_with_method, set_width_method, width_method,
};
