//! Fuzz target for keyboard sequence parsing.
//!
//! Focuses on escape sequences that represent keyboard input.
//! Generates structured inputs to stress-test specific parser paths.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use opentui::input::{InputParser, ParseError};

/// Structured input for keyboard fuzzing.
#[derive(Arbitrary, Debug)]
struct KeyboardInput {
    /// The type of sequence to generate.
    seq_type: SequenceType,
    /// Raw bytes to append (for edge cases).
    suffix: Vec<u8>,
}

#[derive(Arbitrary, Debug)]
enum SequenceType {
    /// Raw escape followed by arbitrary bytes.
    EscapeSequence { bytes: Vec<u8> },
    /// CSI sequence: ESC [ <params> <final>
    CsiSequence {
        params: Vec<u8>,
        intermediate: Option<u8>,
        final_byte: u8,
    },
    /// SS3 sequence: ESC O <final>
    Ss3Sequence { final_byte: u8 },
    /// Function key: ESC [ <n> ~
    FunctionKey { n: u8 },
    /// Arrow with modifiers: ESC [ 1 ; <mod> <dir>
    ModifiedArrow { modifier: u8, direction: u8 },
    /// Just raw bytes.
    RawBytes { bytes: Vec<u8> },
}

impl KeyboardInput {
    /// Convert to bytes for parsing.
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = match &self.seq_type {
            SequenceType::EscapeSequence { bytes: inner } => {
                let mut v = vec![0x1b]; // ESC
                v.extend(inner.iter().take(100)); // Limit length
                v
            }
            SequenceType::CsiSequence {
                params,
                intermediate,
                final_byte,
            } => {
                let mut v = vec![0x1b, b'[']; // ESC [
                v.extend(params.iter().take(50));
                if let Some(i) = intermediate {
                    v.push(*i);
                }
                v.push(*final_byte);
                v
            }
            SequenceType::Ss3Sequence { final_byte } => {
                vec![0x1b, b'O', *final_byte] // ESC O <final>
            }
            SequenceType::FunctionKey { n } => {
                let mut v = vec![0x1b, b'['];
                // Convert n to decimal ASCII
                if *n >= 100 {
                    v.push(b'0' + (n / 100));
                }
                if *n >= 10 {
                    v.push(b'0' + ((n / 10) % 10));
                }
                v.push(b'0' + (n % 10));
                v.push(b'~');
                v
            }
            SequenceType::ModifiedArrow {
                modifier,
                direction,
            } => {
                vec![
                    0x1b,
                    b'[',
                    b'1',
                    b';',
                    b'0' + (modifier % 10),
                    *direction,
                ]
            }
            SequenceType::RawBytes { bytes } => bytes.iter().take(100).copied().collect(),
        };

        // Append suffix (limited)
        bytes.extend(self.suffix.iter().take(50));
        bytes
    }
}

fuzz_target!(|input: KeyboardInput| {
    let bytes = input.to_bytes();
    if bytes.is_empty() {
        return;
    }

    let mut parser = InputParser::new();
    let mut remaining = bytes.as_slice();
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000;

    while !remaining.is_empty() && iterations < MAX_ITERATIONS {
        iterations += 1;

        match parser.parse(remaining) {
            Ok((_event, consumed)) => {
                if consumed == 0 {
                    remaining = &remaining[1..];
                } else {
                    remaining = &remaining[consumed..];
                }
            }
            Err(ParseError::Empty | ParseError::Incomplete) => break,
            Err(_) => {
                if !remaining.is_empty() {
                    remaining = &remaining[1..];
                }
            }
        }
    }
});
