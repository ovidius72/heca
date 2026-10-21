//! Shared text-field metrics, configurable in a theme's `[input]` table.
use serde::{Deserialize, Serialize};

/// Defaults for single- and multi-line fields. Widgets may override the line limit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputStyle {
    /// Natural width in logical pixels.
    pub width: f32,
    /// Inner padding in logical pixels (scales with the control size variant).
    pub padding: f32,
    /// Caret stroke width in logical pixels.
    pub caret_width: f32,
    /// Full blink cycle in seconds. Non-positive means a steady caret.
    pub blink_period: f32,
    /// Rest glow radius in logical pixels.
    pub glow_radius: f32,
    /// Maximum visible lines before a multi-line field scrolls.
    pub max_lines: usize,
}
impl Default for InputStyle {
    fn default() -> Self {
        Self { width: 240.0, padding: 10.0, caret_width: 1.5,
            blink_period: 1.0, glow_radius: 12.0, max_lines: 6 }
    }
}
