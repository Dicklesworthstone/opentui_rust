#![allow(clippy::nursery)] // Test infra prioritizes clarity over pedantry
#![allow(clippy::pedantic)] // Test infra prioritizes clarity over pedantry

pub mod analysis;
pub mod artifacts;
pub mod golden;
pub mod harness;
pub mod input_sim;
pub mod metrics;
pub mod mock_terminal;
pub mod pty;
