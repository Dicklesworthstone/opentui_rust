//! Fuzz target for the ANSI input parser.
//!
//! Tests that the input parser handles arbitrary byte sequences without panicking.
//! This is critical for security since the parser handles untrusted terminal input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use opentui::input::{InputParser, ParseError};

fuzz_target!(|data: &[u8]| {
    // Create a fresh parser for each input
    let mut parser = InputParser::new();

    // Try to parse the entire input, consuming as much as possible
    let mut remaining = data;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 10000;

    while !remaining.is_empty() && iterations < MAX_ITERATIONS {
        iterations += 1;

        match parser.parse(remaining) {
            Ok((_event, consumed)) => {
                // Successfully parsed an event - advance the buffer
                if consumed == 0 {
                    // Defensive: avoid infinite loop if parser claims 0 bytes consumed
                    remaining = &remaining[1..];
                } else {
                    remaining = &remaining[consumed..];
                }
            }
            Err(ParseError::Empty) => {
                // Buffer is empty - done
                break;
            }
            Err(ParseError::Incomplete) => {
                // Need more data - done with this input
                break;
            }
            Err(ParseError::UnrecognizedSequence(_)) => {
                // Unknown sequence - skip one byte and continue
                if !remaining.is_empty() {
                    remaining = &remaining[1..];
                }
            }
            Err(ParseError::InvalidUtf8) => {
                // Invalid UTF-8 - skip one byte and continue
                if !remaining.is_empty() {
                    remaining = &remaining[1..];
                }
            }
        }
    }

    // Ensure we didn't hit the iteration limit (would indicate infinite loop)
    assert!(
        iterations < MAX_ITERATIONS,
        "Parser appears to be in an infinite loop"
    );
});
