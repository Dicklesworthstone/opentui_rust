//! Unicode utilities for grapheme handling and display width.

mod bidi;
mod grapheme;
mod normalize;
mod search;
mod width;

pub use bidi::{
    BidiInfo, Direction, get_base_direction, get_bidi_embedding_levels, reorder_for_display,
    resolve_bidi,
};
pub use grapheme::{
    GraphemeInfo, GraphemeIterator, find_grapheme_boundary, grapheme_indices, grapheme_info,
    graphemes, is_ascii_only, split_graphemes_with_widths,
};
pub use normalize::{compare_normalized, is_normalized_nfc, normalize_nfc, normalize_nfd};
pub use search::{
    BreakType, LineBreakResult, TabStopResult, WrapBreakResult, calculate_text_width,
    find_line_breaks, find_position_by_width, find_tab_stops, find_wrap_breaks, find_wrap_position,
    get_prev_grapheme_start, is_ascii_only_fast, is_printable_ascii_only,
};
pub use width::{
    WidthMethod, clear_width_overrides, display_width, display_width_char,
    display_width_char_with_method, display_width_with_method, get_width_override,
    set_width_method, set_width_override, width_method,
};
