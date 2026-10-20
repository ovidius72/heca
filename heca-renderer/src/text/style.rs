//! **What a caller says about text** — its style, its box, and the halo behind it.

pub struct TextStyle<'a> {
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub faux_italic: bool,
    pub font_family: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub struct TextBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// An additive halo behind a text run, mirroring `heca-grid-ui`'s `Glow` on a rect.
///
/// The scene asks for a glow declaratively; *how* it is realized is this renderer's
/// choice. The glyph atlas stores coverage masks (`R8Unorm`) with one texel of
/// padding, so a multi-tap blur read in the fragment shader would bleed into
/// neighbouring atlas entries — hence the halo is built from offset copies of the
/// run drawn additively beneath the sharp glyphs. Swapping this for a true SDF text
/// pass later would change nothing above this layer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextGlow {
    /// Halo colour (alpha is the caller's overall strength).
    pub color: [f32; 4],
    /// Falloff radius in logical pixels — how far the halo reaches.
    pub radius: f32,
    /// Strength multiplier in `0.0..=1.0`.
    pub intensity: f32,
}

impl TextGlow {
    /// Whether this glow actually produces a halo. A glow with no reach, no
    /// strength, or no opacity draws nothing.
    ///
    /// This is the **single** definition of "inert", used both when emitting the
    /// halo and when hashing the cache key. If the two ever disagreed, a run whose
    /// glow draws nothing would still hash differently from an unglowing one and
    /// rebuild its geometry every frame.
    pub(super) fn is_visible(self) -> bool {
        self.radius > 0.0 && self.intensity > 0.0 && self.color[3] > 0.0
    }
}

/// Blur sigma as a fraction of [`TextGlow::radius`].
///
/// A Gaussian is visually spent by about 3 sigma, so a third of the requested radius
/// puts the halo's visible edge at that radius.
pub(super) const HALO_SIGMA_FRAC: f32 = 1.0 / 3.0;
/// Strength multiplier on a **dark** background, where the halo composites
/// additively.
///
/// Above `1.0` on purpose. Because the blurred mask is renormalized to full peak,
/// `1.0` would make a glyph's halo peak at exactly a lit surface's — which measures
/// equal but *looks* dimmer, since a glyph emits over a far smaller perimeter than a
/// button does. This compensates for the area a small emitter cannot cover.
pub(super) const HALO_DARK_GAIN: f32 = 1.6;
/// Strength multiplier on a **light** background, applied on top of the rect
/// shader's own light-background curve (`sqrt(base) * glow_alpha_scale`). Carries the
/// same small-emitter compensation as [`HALO_DARK_GAIN`], so the two themes stay in
/// step when either is retuned.
pub(super) const HALO_LIGHT_GAIN: f32 = 1.6;
