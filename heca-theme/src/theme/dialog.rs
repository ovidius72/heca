use serde::{Deserialize, Serialize};

/// Shared dialog sizing. Width scales with the UI font and remains bounded by the viewport.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogStyle {
    /// Maximum panel width in multiples of its resolved font size.
    pub max_width_em: f32,
}

impl Default for DialogStyle {
    fn default() -> Self { Self { max_width_em: 40.0 } }
}
