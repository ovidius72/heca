//! **The theme's rhythm** — the steps of space every widget is laid out with, configurable in a
//! theme's `[spacing]` table.

use serde::{Deserialize, Serialize};

/// **How much air each step of space is**, as a share of the font it is used at — so a larger
/// font gets proportionally more room. A widget names a step (`Spacing::Sm`) and never a size; a
/// theme that wants a denser or an airier look changes these, and every gap and padding follows.
///
/// ```toml
/// [spacing]
/// xs = 0.2    # tighter than the default 0.25
/// md = 1.0
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpacingScale {
    /// A hairline of air: lines that belong to one block.
    pub hairline: f32,
    /// The smallest gap between separate things.
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    /// The widest step: room around a thin target the pointer has to find.
    pub xl: f32,
}

impl Default for SpacingScale {
    fn default() -> Self {
        Self {
            hairline: 0.08,
            xs: 0.25,
            sm: 0.5,
            md: 0.85,
            lg: 1.25,
            xl: 2.0,
        }
    }
}
