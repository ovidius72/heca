//! **Visual decoration** — surfaces only.

use crate::color::Color;
use crate::component::Component;
use crate::scene::{Border, Glow};

/// Visual decoration builders — **surfaces only**.
pub trait StyleExt: Component + Sized {
    /// **Background fill** — a colour, or a theme colour by name that follows the theme:
    /// `.background(Fill::theme("surface"))`. See [`Fill`](crate::style::Fill).
    fn background(mut self, fill: impl Into<crate::style::Fill>) -> Self {
        let visual = &mut self.authored_style_mut().visual;
        match fill.into() {
            crate::style::Fill::Color(colour) => {
                visual.fill = Some(colour);
                visual.fill_theme = None;
            }
            crate::style::Fill::Theme(name) => {
                visual.fill = None;
                visual.fill_theme = Some(name);
            }
        }
        self
    }
    /// Border outline.
    fn border(mut self, c: Color, width: f32) -> Self {
        self.authored_style_mut().visual.border = Some(Border { color: c, width });
        self
    }
    /// Neon outer glow (default radius/intensity).
    fn glow(mut self, c: Color) -> Self {
        self.authored_style_mut().visual.glow = Some(Glow {
            color: c,
            radius: 8.0,
            intensity: 1.0,
        });
        self
    }
    /// Neon outer glow with explicit falloff radius and intensity.
    fn glow_with(mut self, c: Color, radius: f32, intensity: f32) -> Self {
        self.authored_style_mut().visual.glow = Some(Glow {
            color: c,
            radius,
            intensity,
        });
        self
    }
    /// Corner radius.
    fn radius(mut self, r: f32) -> Self {
        self.authored_style_mut().visual.radius = r;
        self
    }

    /// Semantic font multiplier relative to the inherited base font (e.g. `2.0`
    /// for a header, `0.8` for a caption). Scales with a global font change.
    fn font_scale(mut self, scale: f32) -> Self {
        self.authored_style_mut().visual.font_scale = scale;
        self
    }
}
