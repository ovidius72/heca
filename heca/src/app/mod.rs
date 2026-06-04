//! App-level orchestration modules.
//!
//! These modules keep runtime wiring concerns out of `main.rs` while preserving
//! the current behavior and ownership boundaries.

pub mod focus;
pub mod mutations;
pub mod registry;
pub mod render;
