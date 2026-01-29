//! Terminal capability query response parsing.
//!
//! Parses responses from terminal capability queries:
//! - DA1 (Primary Device Attributes): `ESC[c`
//! - DA2 (Secondary Device Attributes): `ESC[>c`
//! - XTVERSION: `ESC[>0q`
//! - Pixel resolution: `ESC[14t`
//! - Kitty keyboard protocol: `ESC[?u`

use crate::ansi::sequences;

/// Maximum length for DCS response parsing.
///
/// Defense-in-depth limit to prevent DoS via maliciously long responses.
/// Same limit as used in the input parser.
const MAX_DCS_RESPONSE_LENGTH: usize = 64 * 1024;

/// Query sequence constants for terminal capability detection.
pub mod query_constants {
    pub use crate::ansi::sequences::query::DEVICE_ATTRIBUTES as DA1;
    pub use crate::ansi::sequences::query::DEVICE_ATTRIBUTES_SECONDARY as DA2;
    pub use crate::ansi::sequences::query::KITTY_KEYBOARD;
    pub use crate::ansi::sequences::query::PIXEL_RESOLUTION;
    pub use crate::ansi::sequences::query::XTVERSION;
}

/// Response from a terminal capability query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalResponse {
    /// Primary device attributes (DA1) response.
    /// Response format: `ESC [ ? Ps ; Ps ; ... c`
    DeviceAttributes {
        /// Whether this is from DA1 (true) or DA2 (false).
        primary: bool,
        /// Parameter values from the response.
        params: Vec<u32>,
    },

    /// Terminal version (XTVERSION) response.
    /// Response format: `ESC P > | name version ST`
    XtVersion {
        /// Terminal name (e.g., "kitty", "foot", "alacritty").
        name: String,
        /// Version string.
        version: String,
    },

    /// Pixel size response.
    /// Response format: `ESC [ 4 ; height ; width t`
    PixelSize {
        /// Width in pixels.
        width: u16,
        /// Height in pixels.
        height: u16,
    },

    /// Kitty keyboard protocol response.
    /// Response format: `ESC [ ? flags u`
    KittyKeyboard {
        /// Keyboard protocol flags.
        flags: u32,
    },

    /// Unknown or unparseable response.
    Unknown(Vec<u8>),
}

impl TerminalResponse {
    /// Parse a terminal response from raw bytes.
    #[must_use]
    pub fn parse(input: &[u8]) -> Option<Self> {
        if input.len() < 3 {
            return None;
        }

        // Check for ESC prefix
        if input[0] != 0x1b {
            return None;
        }

        // Try different parsers
        if let Some(resp) = Self::parse_da1(input) {
            return Some(resp);
        }
        if let Some(resp) = Self::parse_da2(input) {
            return Some(resp);
        }
        if let Some(resp) = Self::parse_xtversion(input) {
            return Some(resp);
        }
        if let Some(resp) = Self::parse_pixel_size(input) {
            return Some(resp);
        }
        if let Some(resp) = Self::parse_kitty_keyboard(input) {
            return Some(resp);
        }

        Some(TerminalResponse::Unknown(input.to_vec()))
    }

    /// Parse DA1 response: `ESC [ ? Ps ; Ps ; ... c`
    fn parse_da1(input: &[u8]) -> Option<Self> {
        // Look for ESC [ ? ... c
        if input.len() < 4 || input[1] != b'[' || input[2] != b'?' {
            return None;
        }

        // Find the 'c' terminator
        let end = input.iter().position(|&b| b == b'c')?;
        if end < 3 {
            return None;
        }

        // Parse parameters between ESC[? and c
        let params_str = std::str::from_utf8(&input[3..end]).ok()?;
        let params: Vec<u32> = params_str
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();

        Some(TerminalResponse::DeviceAttributes {
            primary: true,
            params,
        })
    }

    /// Parse DA2 response: `ESC [ > Pp ; Pv ; Pc c`
    fn parse_da2(input: &[u8]) -> Option<Self> {
        // Look for ESC [ > ... c
        if input.len() < 4 || input[1] != b'[' || input[2] != b'>' {
            return None;
        }

        // Find the 'c' terminator
        let end = input.iter().position(|&b| b == b'c')?;
        if end < 3 {
            return None;
        }

        // Parse parameters between ESC[> and c
        let params_str = std::str::from_utf8(&input[3..end]).ok()?;
        let params: Vec<u32> = params_str
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();

        Some(TerminalResponse::DeviceAttributes {
            primary: false,
            params,
        })
    }

    /// Parse XTVERSION response: `ESC P > | text ST`
    /// ST (String Terminator) is `ESC \` or `\x9c`
    fn parse_xtversion(input: &[u8]) -> Option<Self> {
        // Look for DCS (ESC P or 0x90)
        // Defense-in-depth: reject overly long responses to prevent DoS
        if input.len() < 5 || input.len() > MAX_DCS_RESPONSE_LENGTH {
            return None;
        }

        let start = if input[0] == 0x1b && input[1] == b'P' {
            2
        } else if input[0] == 0x90 {
            1
        } else {
            return None;
        };

        // Check for >| prefix
        if input.get(start) != Some(&b'>') || input.get(start + 1) != Some(&b'|') {
            return None;
        }

        // Find ST (String Terminator): ESC \ or 0x9c
        let content_start = start + 2;
        let st_pos = input[content_start..]
            .windows(2)
            .position(|w| w == b"\x1b\\")
            .map(|p| content_start + p)
            .or_else(|| {
                input[content_start..]
                    .iter()
                    .position(|&b| b == 0x9c)
                    .map(|p| content_start + p)
            })?;

        let content = std::str::from_utf8(&input[content_start..st_pos]).ok()?;

        // Parse "name version" format
        let (name, version) = content.find(' ').map_or_else(
            || (content.to_string(), String::new()),
            |space_pos| {
                (
                    content[..space_pos].to_string(),
                    content[space_pos + 1..].to_string(),
                )
            },
        );

        Some(TerminalResponse::XtVersion { name, version })
    }

    /// Parse pixel size response: `ESC [ 4 ; height ; width t`
    fn parse_pixel_size(input: &[u8]) -> Option<Self> {
        // Look for ESC [ 4 ; ... t
        if input.len() < 6 || input[1] != b'[' || input[2] != b'4' || input[3] != b';' {
            return None;
        }

        // Find the 't' terminator
        let end = input.iter().position(|&b| b == b't')?;
        if end < 5 {
            return None;
        }

        // Parse "height;width" from ESC[4;height;width t
        let params_str = std::str::from_utf8(&input[4..end]).ok()?;
        let mut parts = params_str.split(';');
        let height: u16 = parts.next()?.parse().ok()?;
        let width: u16 = parts.next()?.parse().ok()?;

        Some(TerminalResponse::PixelSize { width, height })
    }

    /// Parse Kitty keyboard response: `ESC [ ? flags u`
    fn parse_kitty_keyboard(input: &[u8]) -> Option<Self> {
        // Look for ESC [ ? ... u
        if input.len() < 4 || input[1] != b'[' || input[2] != b'?' {
            return None;
        }

        // Find the 'u' terminator
        let end = input.iter().position(|&b| b == b'u')?;
        if end < 3 {
            return None;
        }

        // Parse flags from ESC[?flags u
        let flags_str = std::str::from_utf8(&input[3..end]).ok()?;
        let flags: u32 = flags_str.parse().ok()?;

        Some(TerminalResponse::KittyKeyboard { flags })
    }

    /// Check if DA1 response indicates sixel support.
    /// Sixel is indicated by parameter 4 in the DA1 response.
    #[must_use]
    pub fn has_sixel(&self) -> bool {
        if let TerminalResponse::DeviceAttributes {
            primary: true,
            params,
        } = self
        {
            params.contains(&4)
        } else {
            false
        }
    }

    /// Get terminal name from XTVERSION response.
    #[must_use]
    pub fn terminal_name(&self) -> Option<&str> {
        if let TerminalResponse::XtVersion { name, .. } = self {
            Some(name)
        } else {
            None
        }
    }
}

/// Get all capability query sequences as a single string.
#[must_use]
pub fn all_queries() -> String {
    format!(
        "{}{}{}{}{}",
        sequences::query::DEVICE_ATTRIBUTES,
        sequences::query::DEVICE_ATTRIBUTES_SECONDARY,
        sequences::query::XTVERSION,
        sequences::query::PIXEL_RESOLUTION,
        sequences::query::KITTY_KEYBOARD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_da1_response_basic() {
        // Basic DA1 response: ESC [ ? 1 ; 2 c
        let input = b"\x1b[?1;2c";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::DeviceAttributes { primary, params } => {
                assert!(primary, "Should be DA1 (primary)");
                assert_eq!(params, vec![1, 2]);
            }
            _ => panic!("Expected DeviceAttributes"),
        }
    }

    #[test]
    fn test_parse_da1_response_with_sixel() {
        // DA1 response with sixel support (param 4): ESC [ ? 62 ; 4 ; 6 c
        let input = b"\x1b[?62;4;6c";
        let response = TerminalResponse::parse(input).unwrap();

        assert!(response.has_sixel(), "Should detect sixel support");
    }

    #[test]
    fn test_parse_da2_response() {
        // DA2 response: ESC [ > 1 ; 4000 ; 20 c
        let input = b"\x1b[>1;4000;20c";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::DeviceAttributes { primary, params } => {
                assert!(!primary, "Should be DA2 (secondary)");
                assert_eq!(params, vec![1, 4000, 20]);
            }
            _ => panic!("Expected DeviceAttributes"),
        }
    }

    #[test]
    fn test_parse_xtversion_kitty() {
        // XTVERSION response from kitty: ESC P > | kitty(0.26.5) ST
        let input = b"\x1bP>|kitty(0.26.5)\x1b\\";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::XtVersion { name, version } => {
                assert!(name.contains("kitty"), "Should detect kitty");
                assert!(version.contains("0.26.5") || name.contains("0.26.5"));
            }
            _ => panic!("Expected XtVersion"),
        }
    }

    #[test]
    fn test_parse_xtversion_alacritty() {
        // XTVERSION response from alacritty
        let input = b"\x1bP>|alacritty 0.12.0\x1b\\";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::XtVersion { name, version } => {
                assert_eq!(name, "alacritty");
                assert_eq!(version, "0.12.0");
            }
            _ => panic!("Expected XtVersion"),
        }
    }

    #[test]
    fn test_parse_pixel_size_response() {
        // Pixel size response: ESC [ 4 ; 900 ; 1440 t
        let input = b"\x1b[4;900;1440t";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::PixelSize { width, height } => {
                assert_eq!(width, 1440);
                assert_eq!(height, 900);
            }
            _ => panic!("Expected PixelSize"),
        }
    }

    #[test]
    fn test_parse_kitty_keyboard_response() {
        // Kitty keyboard response: ESC [ ? 1 u
        let input = b"\x1b[?1u";
        let response = TerminalResponse::parse(input).unwrap();

        match response {
            TerminalResponse::KittyKeyboard { flags } => {
                assert_eq!(flags, 1);
            }
            _ => panic!("Expected KittyKeyboard"),
        }
    }

    #[test]
    fn test_parse_unknown_response() {
        // Unknown sequence
        let input = b"\x1b[99z";
        let response = TerminalResponse::parse(input).unwrap();

        assert!(matches!(response, TerminalResponse::Unknown(_)));
    }

    #[test]
    fn test_query_sequences_correct() {
        // Verify query sequences are valid
        assert_eq!(query_constants::DA1, "\x1b[c");
        assert_eq!(query_constants::DA2, "\x1b[>c");
        assert_eq!(query_constants::XTVERSION, "\x1b[>0q");
        assert_eq!(query_constants::PIXEL_RESOLUTION, "\x1b[14t");
        assert_eq!(query_constants::KITTY_KEYBOARD, "\x1b[?u");
    }

    #[test]
    fn test_all_queries_sends_all() {
        let all = all_queries();
        assert!(all.contains("\x1b[c"), "Should contain DA1");
        assert!(all.contains("\x1b[>c"), "Should contain DA2");
        assert!(all.contains("\x1b[>0q"), "Should contain XTVERSION");
        assert!(all.contains("\x1b[14t"), "Should contain pixel resolution");
        assert!(all.contains("\x1b[?u"), "Should contain kitty keyboard");
    }

    #[test]
    fn test_capabilities_updated_from_da1() {
        let input = b"\x1b[?62;4c";
        let response = TerminalResponse::parse(input).unwrap();
        assert!(response.has_sixel());
    }

    #[test]
    fn test_terminal_name_extraction() {
        let input = b"\x1bP>|foot 1.15.3\x1b\\";
        let response = TerminalResponse::parse(input).unwrap();
        assert_eq!(response.terminal_name(), Some("foot"));
    }
}
