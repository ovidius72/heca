//! Drag-and-drop framework types.
//!
//! This module provides the core types for a surface-agnostic drag-and-drop
//! system. Each drag surface (sidebar, inspector, etc.) owns a
//! [`SurfaceDragState`] and participates via [`DragSurfaceId`] enum dispatch.
//! A single [`DragContext`] coordinates which surface is actively dragging.
//!
//! ## Design rules
//!
//! - **No GPU code.** This crate emits state; the renderer rasterizes it.
//! - **No app-specific types.** `WmAction` lives in `heca`; click actions
//!   are tracked at the app layer, not in [`DragPhase`].
//! - **Closed set of surfaces.** [`DragSurfaceId`] is an enum, not a trait,
//!   so the compiler enforces exhaustiveness and zero-cost dispatch.
//! - **Per-surface state.** Each surface has independent hover/source/ghost.
//!   [`DragContext`] only tracks *which* surface is active.

mod context;
mod item;
mod math;
mod resolve;
mod state;

pub use context::DragContext;
pub use item::{DragItemId, DragSurfaceId};
pub use math::{DEFAULT_DRAG_THRESHOLD_SQ, rubberband};
pub use resolve::{DropHit, DropSide, resolve_at, resolve_at_filtered, source_at};
pub use state::{DragLabel, DragPhase, SurfaceDragState};
