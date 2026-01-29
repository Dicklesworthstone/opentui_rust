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
    /// Pixel count doesn't match expected dimensions.
    SizeMismatch { expected: usize, actual: usize },
    /// Dimension overflow (width * height exceeds usize).
    DimensionOverflow { width: u32, height: u32 },
    /// Buffer size mismatch in diff operation.
    BufferSizeMismatch {
        old_size: (u32, u32),
        new_size: (u32, u32),
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
            Self::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "size mismatch: expected {expected} elements, got {actual}"
                )
            }
            Self::DimensionOverflow { width, height } => {
                write!(f, "dimension overflow: {width}x{height} exceeds capacity")
            }
            Self::BufferSizeMismatch { old_size, new_size } => {
                write!(
                    f,
                    "buffer size mismatch: old={}x{}, new={}x{}",
                    old_size.0, old_size.1, new_size.0, new_size.1
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
