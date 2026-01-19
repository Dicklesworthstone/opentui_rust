//! Error types for OpenTUI.

use std::fmt;
use std::io;

/// Result type alias for OpenTUI operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for OpenTUI operations.
#[derive(Debug)]
pub enum Error {
    /// I/O error from terminal operations.
    Io(io::Error),
    /// Invalid color format (e.g., malformed hex string).
    InvalidColor(String),
    /// Buffer dimension error (e.g., zero width/height).
    InvalidDimensions { width: u32, height: u32 },
    /// Position out of bounds.
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidColor(s) => write!(f, "invalid color format: {s}"),
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::OutOfBounds {
                x,
                y,
                width,
                height,
            } => {
                write!(
                    f,
                    "position ({x}, {y}) out of bounds for {width}x{height} buffer"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::InvalidColor("not-a-color".to_string());
        assert!(err.to_string().contains("invalid color format"));

        let err = Error::InvalidDimensions {
            width: 0,
            height: 100,
        };
        assert!(err.to_string().contains("0x100"));

        let err = Error::OutOfBounds {
            x: 10,
            y: 20,
            width: 5,
            height: 5,
        };
        assert!(err.to_string().contains("(10, 20)"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
