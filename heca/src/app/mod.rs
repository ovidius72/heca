//! App-level orchestration modules.
//!
//! These modules keep runtime wiring concerns out of `main.rs` while preserving
//! the current behavior and ownership boundaries.

pub mod backend_factory;
pub mod backend_store;
pub mod events;
pub mod focus;
pub mod git_monitor;
pub mod input;
pub mod interaction;
pub mod keyboard;
pub mod lifecycle;
pub mod mutations;
pub mod pane_ops;
pub mod process_monitor;
pub mod registry;
pub mod render;
pub mod selection;
pub mod selection_model;
pub mod startup;
pub mod terminal_host;
pub mod terminal_metrics;
pub mod terminal_render;
