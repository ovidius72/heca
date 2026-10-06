//! A pane within a column: what it is, and how it animates when it moves.

use crate::layout::animation::{Animated, Animation, AnimationConfig};
use crate::layout::types::*;
use crate::runtime::{PaneClosePolicy, PaneRuntime};

impl Pane {
    /// Animate this pane moving vertically from an offset.
    pub fn animate_move_y_from(&mut self, from_y: f64, config: AnimationConfig) {
        self.move_offset.to_static();
        self.move_offset = Animated::Animating {
            animation: Animation::new(0.0, 1.0, config),
            from: Point::new(0.0, from_y),
            to: Point::new(0.0, 0.0),
        };
    }

    /// Animate this pane moving from a 2D offset.
    pub fn animate_move_from(&mut self, from: Point, config: AnimationConfig) {
        self.move_offset.to_static();
        self.move_offset = Animated::Animating {
            animation: Animation::new(0.0, 1.0, config),
            from,
            to: Point::new(0.0, 0.0),
        };
    }
}

/// A pane within a column.
///
/// This is heca's equivalent of NIRI's `Tile<W>` — it wraps the actual content
/// (terminal, neovim, browser) and tracks its layout state.
#[derive(Debug, Clone)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    /// User-set display name (from rename). `Some` **overrides** the process-derived
    /// title everywhere it's shown; `None` means the name tracks the running process.
    pub custom_name: Option<String>,
    pub runtime: PaneRuntime,
    pub close_policy: PaneClosePolicy,
    /// Preferred fixed height (None = auto).
    pub preferred_height: Option<f64>,
    /// Move animation offset (entry/exit animations).
    pub move_offset: Animated<Point>,
    /// Offset applied during interactive move Starting phase (rubberband).
    /// Cleared on transition to Moving. Not used by entry/exit animations.
    pub interactive_move_offset: Point,
}

impl Pane {
    pub fn new(id: PaneId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            custom_name: None,
            runtime: PaneRuntime::default(),
            close_policy: PaneClosePolicy::default(),
            preferred_height: None,
            move_offset: Animated::Static(Point::default()),
            interactive_move_offset: Point::default(),
        }
    }
}
