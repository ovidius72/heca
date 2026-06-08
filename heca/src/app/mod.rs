//! App-level orchestration modules.
//!
//! These modules keep runtime wiring concerns out of `main.rs` while preserving
//! the current behavior and ownership boundaries.

pub mod events;
pub mod focus;
pub mod input;
pub mod keyboard;
pub mod lifecycle;
pub mod mutations;
pub mod registry;
pub mod render;
pub mod selection;
pub mod startup;

