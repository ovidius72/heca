//! Where the pointer is.

/// All mouse-related runtime state.
#[derive(Clone, Debug)]
pub struct MouseState {
    pub pos: (f32, f32),
    /// Last time edge scroll was processed (for frame-rate independence).
    pub last_edge_scroll_time: Option<std::time::Instant>,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            pos: (0.0, 0.0),
            last_edge_scroll_time: None,
        }
    }
}
