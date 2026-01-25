//! Entry point for E2E syntax highlighting tests.
//!
//! Run with:
//!   cargo test --test `highlight_e2e` -- --nocapture
//!
//! CI: included in the default `cargo test` run.

#[path = "e2e/highlight_e2e.rs"]
mod highlight_e2e;
