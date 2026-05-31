//! heca layout engine — NIRI-inspired scrollable tiling with workspaces and overview.
//!
//! This module implements the core layout system:
//!
//! - **`session`**: Top-level `Session` managing workspaces, overview, and workspace switching
//! - **`workspace`**: `Workspace` containing a `ScrollingSpace` + floating panes
//! - **`scrolling`**: `ScrollingSpace` — horizontal scrolling columns (NIRI's core innovation)
//! - **`column`**: `Column` containing `Pane`s arranged vertically
//! - **`view_offset`**: `ViewOffset` — animated horizontal scroll with gesture support
//! - **`animation`**: `Animation`, `SwipeTracker`, easing functions
//! - **`types`**: Shared types (`ColumnWidth`, `WindowHeight`, `Rectangle`, etc.)
//!
//! ## Architecture (matching NIRI)
//!
//! ```text
//! Session
//! └── workspaces: Vec<Workspace>          ← vertical stack
//!     ├── Workspace 1
//!     │   ├── scrolling: ScrollingSpace   ← horizontal columns
//!     │   │   ├── Column 1: [Pane a]
//!     │   │   ├── Column 2: [Pane b, Pane c]  ← vertical stack
//!     │   │   └── Column 3: [Pane d]
//!     │   └── floating: Vec<FloatingPane>
//!     ├── Workspace 2
//!     │   └── ...
//!     └── overview: zoomed-out thumbnails ← expose view
//! ```
//!
//! ## NIRI Key Concepts Preserved
//!
//! 1. **Horizontal scrolling** via `ViewOffset` — columns scroll left/right, snapping to active
//! 2. **Vertical stacking** within columns — panes tile top-to-bottom with gaps
//! 3. **Dynamic workspaces** — vertical stack of workspaces, switch with gesture/animation
//! 4. **Overview/Expose** — all workspaces rendered as scaled thumbnails with shadows
//! 5. **Animated transitions** — all focus changes, column moves, and workspace switches animate

pub mod animation;
pub mod column;
pub mod scrolling;
pub mod session;
pub mod types;
pub mod view_offset;
pub mod workspace;

pub use column::{Column, Pane};
pub use scrolling::ScrollingSpace;
pub use session::{OverviewState, Session, WorkspaceSwitch};
pub use types::*;
pub use view_offset::ViewOffset;
pub use workspace::Workspace;
