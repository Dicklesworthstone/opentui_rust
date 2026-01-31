//! Test fixtures and helpers for OpenTUI tests.
//!
//! This module provides reusable test infrastructure:
//!
//! - [`MockTerminal`] - Captures ANSI output without real PTY
//! - [`MockInput`] - Provides scripted input events
//! - [`assertions`] - Buffer and cell comparison helpers
//! - [`test_data`] - Sample data generators for tests
//!
//! # Example
//!
//! ```ignore
//! use opentui_test_fixtures::{MockTerminal, assert_buffer_eq};
//!
//! let mut term = MockTerminal::new(80, 24);
//! term.write_all(b"\x1b[2J"); // Clear screen
//! assert!(term.output().contains("\x1b[2J"));
//! ```

#![allow(clippy::nursery)] // Test fixtures prioritize clarity over pedantry
#![allow(clippy::pedantic)] // Test fixtures prioritize clarity over pedantry

pub mod assertions;
pub mod mock_input;
pub mod mock_terminal;
pub mod test_data;

pub use assertions::*;
pub use mock_input::*;
pub use mock_terminal::*;
pub use test_data::*;
