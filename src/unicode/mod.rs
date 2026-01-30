//! Unicode utilities for grapheme handling and display width.

mod grapheme;
mod search;
mod width;

pub use grapheme::{
    GraphemeInfo, GraphemeIterator, find_grapheme_boundary, grapheme_indices, grapheme_info,
    graphemes, is_ascii_only, split_graphemes_with_widths,
};
pub use search::{
    BreakType, LineBreakResult, TabStopResult, WrapBreakResult, calculate_text_width,
    find_line_breaks, find_position_by_width, find_tab_stops, find_wrap_breaks, find_wrap_position,
    get_prev_grapheme_start, is_ascii_only_fast, is_printable_ascii_only,
};
pub use width::{
    WidthMethod, display_width, display_width_char, display_width_char_with_method,
    display_width_with_method, set_width_method, width_method,
};
