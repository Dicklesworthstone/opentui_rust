//! Fuzz target for color hex parsing.
//!
//! Tests that Rgba::from_hex handles arbitrary strings without panicking.

#![no_main]

use libfuzzer_sys::fuzz_target;
use opentui::color::Rgba;

fuzz_target!(|data: &str| {
    // Try to parse the string as a hex color
    // This should never panic, just return None for invalid input
    let _ = Rgba::from_hex(data);

    // Also try with a # prefix if not already present
    if !data.starts_with('#') {
        let with_hash = format!("#{data}");
        let _ = Rgba::from_hex(&with_hash);
    }

    // Try parsing valid UTF-8 substrings to find edge cases without panicking on char boundaries
    for i in 0..data.len().min(10) {
        if data.is_char_boundary(i) {
            let _ = Rgba::from_hex(&data[i..]);
        }
        let end = data.len() - i;
        if i < data.len() && data.is_char_boundary(end) {
            let _ = Rgba::from_hex(&data[..end]);
        }
    }
});
