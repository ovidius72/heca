//! **The draw vocabulary**: every command a widget can put in a [`Scene`](super::Scene), and what
//! each one carries.

use crate::color::Color;
use heca_core::layout::Rectangle;

/// One drawable element. Adding a new effect is one variant here plus one branch
/// in the renderer — component code is untouched.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Solid or rounded rectangle, with optional border and additive outer glow.
    Rect(RectCmd),
    /// L-shaped corner brackets framing a rectangle (Tron reticle).
    Brackets(BracketCmd),
    /// A positioned text run.
    Text(TextCmd),
    /// A scanline overlay confined to a region.
    Scanline(ScanlineCmd),
    /// Push a clip rectangle; subsequent commands are clipped to it.
    PushClip(Rectangle),
    /// Pop the most recent clip rectangle.
    PopClip,
    /// **Work only the host can do, recorded in scene order.** See [`HostDraw`].
    Host(HostCmd),
}

/// **A request the drawing pass records and the host performs.**
///
/// Some content cannot be expressed as rectangles and text, and must not be forced into them:
///
/// - a **terminal** is rasterised into a texture, because its cell glyphs are the hottest path in
///   the app — drawing them as ordinary commands is rejected;
/// - a **frosted backdrop** is the frame so far, blurred, which is a pass over what is already
///   drawn rather than a shape.
///
/// Neither is a special case in this crate. A widget says *what it wants* and where; the host owns
/// the GPU and does it. That keeps this library free of graphics types — the same bargain
/// [`Text`](DrawCommand::Text) already makes, where the scene names a role and the renderer owns the
/// atlas.
///
/// **Recorded in scene order, with the clip stack resolved**, so a surface inside a scroll region
/// clips like anything else and a backdrop blurs exactly what was drawn before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HostDraw {
    /// **Put the surface `id` here.** The host holds the texture and knows nothing else is needed;
    /// this crate never sees a texture, a format or a device.
    ///
    /// Who allocates the id is the host's business. A plugin rendering its own content places it
    /// with one builder on its own widget — no host-private type, no registry.
    Surface {
        /// Opaque to this crate: the host maps it to whatever it rasterised.
        id: u64,
    },
    /// **Blur whatever has been drawn behind me, here.** `radius` is the blur and `corner` is how
    /// round the blurred box is — the corner radius of the widget that asked, so a frosted rounded
    /// frame is not square under its own border. Both are in logical pixels.
    Backdrop { radius: f32, corner: f32 },
}

/// A [`HostDraw`] and the box it applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCmd {
    pub draw: HostDraw,
    /// Where it goes, in logical pixels, already placed by the paint context.
    pub rect: Rectangle,
    /// How strongly it composites, `0.0..=1.0`. Carries the paint context's
    /// [opacity](crate::PaintCx::with_opacity) like every other command, so a surface inside a
    /// fading overlay fades with it — and a frost fades in with the surface that asked for it,
    /// rather than holding the session out of focus and snapping sharp in one frame.
    pub alpha: f32,
}

/// A filled/rounded rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectCmd {
    pub rect: Rectangle,
    pub fill: Color,
    pub border: Option<Border>,
    /// Corner radius in logical pixels (0.0 = sharp).
    pub radius: f32,
    pub glow: Option<Glow>,
    /// A soft drop shadow cast *behind* this rect (darkens the background).
    /// Independent of [`glow`](RectCmd::glow) (which only adds light).
    pub shadow: Option<Shadow>,
}

/// A soft drop shadow: a dark, blurred, offset halo drawn **behind** a shape to
/// lift it off the background. Unlike [`Glow`] (additive light), it composites a
/// dark color *with alpha* so it reads on dark themes where a glow can't. It is
/// independent of the glow + border tokens, so it shows even when both are off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Shadow color, including its alpha (the umbra strength).
    pub color: Color,
    /// Blur / falloff radius in logical pixels.
    pub radius: f32,
    /// Horizontal offset of the cast shadow (logical px; positive = right).
    pub dx: f32,
    /// Vertical offset of the cast shadow (logical px; positive = down).
    pub dy: f32,
}

/// A rectangle outline.
///
/// Serializable so a description can override it (F003/P017/T7). Its `color` is a
/// [`Color`], whose serde form is a hex string, so a border crosses as
/// `{"color": "#rrggbbaa", "width": 1.0}`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Border {
    pub color: Color,
    pub width: f32,
}

/// An additive neon glow halo around a shape.
///
/// Serializable for the same reason as [`Border`] — see F003/P017/T7.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Glow {
    pub color: Color,
    /// Falloff radius in logical pixels.
    pub radius: f32,
    /// Multiplier on glow strength (driven by theme intensity).
    pub intensity: f32,
}

/// Corner brackets framing a rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BracketCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Length of each bracket arm in logical pixels.
    pub len: f32,
    pub thickness: f32,
    pub glow: Option<Glow>,
}

/// Horizontal text alignment within the target rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Which embedded font family a text run is shaped with — **three faces, three jobs**. A widget
/// names the role; the host maps it to a family, so nothing here knows a font's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontRole {
    /// The theme's monospace text family (default).
    #[default]
    Text,
    /// The embedded icon font (Phosphor); the codepoint is a glyph.
    Icon,
    /// The embedded **Nerd Font** ([`NfIcon`](crate::widgets::NfIcon)); the codepoint is one of its
    /// glyphs.
    ///
    /// A third role rather than a variant of `Icon` because it is a different font with a different
    /// job: Phosphor is the app's pictogram set, the Nerd Font supplies the glyphs Phosphor has none
    /// of — keyboard keys above all (`⇧`, `⌘`, `⎋`), which is why a keycap uses it. The alternative
    /// was drawing those as plain text in the UI face, and the UI face has no `⌃ ⌥ ⌘ ⎋`: they render
    /// as tofu. Verified against the embedded font's cmap, not assumed.
    NerdFont,
}

/// The **font style** of a text run: what the shaper does to the glyphs.
///
/// Decorations (underline, strikethrough) are deliberately **not** here — a line is not a glyph
/// attribute, it is a rect. The widget draws them itself, from the theme, like any other chrome (see
/// [`Label`](crate::widgets::Label)). Keeping the two apart is what lets the renderer stay a pure
/// text shaper.
///
/// It is a value rather than a pile of `bool` parameters so that the *next* attribute doesn't break
/// [`PaintCx::text`](crate::component::PaintCx::text)'s signature a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    /// The bold weight (a real face — the embedded family ships one).
    pub bold: bool,
    /// Slanted. Rendered as a **synthesized oblique** (a glyph shear), because the embedded family
    /// has no italic face — see the bridge in `heca-renderer`.
    pub italic: bool,
}

impl TextStyle {
    /// Upright, regular weight — the default.
    pub const REGULAR: Self = Self {
        bold: false,
        italic: false,
    };
    /// Bold, upright.
    pub const BOLD: Self = Self {
        bold: true,
        italic: false,
    };
    /// Regular weight, slanted.
    pub const ITALIC: Self = Self {
        bold: false,
        italic: true,
    };

    /// Set the weight (chainable, so a state-derived flag reads straight through:
    /// `TextStyle::REGULAR.bold(is_active)`).
    pub const fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Set the slant.
    pub const fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// A run of text positioned within a rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCmd {
    pub rect: Rectangle,
    pub text: String,
    pub color: Color,
    pub size: f32,
    pub align: TextAlign,
    /// Weight + slant (decorations are drawn by the widget, not shaped — see [`TextStyle`]).
    pub style: TextStyle,
    /// Which font family shapes this run (text vs. icon glyph font).
    pub font: FontRole,
    /// Additive halo behind the glyphs, or `None` for flat text (the default, and
    /// what every terminal run uses).
    ///
    /// This is **declarative, exactly like [`RectCmd::glow`]**: the scene says *this
    /// run glows, this colour, this falloff* and the renderer decides how to realize
    /// it. `heca-renderer` currently does so by blurring the glyph's coverage mask
    /// into its own atlas entry and drawing that behind the sharp glyph — but that is
    /// a renderer detail, and replacing it changes no scene code and no widget.
    pub glow: Option<Glow>,
}

/// A scanline overlay confined to a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanlineCmd {
    pub rect: Rectangle,
    pub color: Color,
    /// Vertical spacing between lines in logical pixels.
    pub spacing: f32,
    /// Per-line opacity multiplier (0.0..=1.0).
    pub opacity: f32,
}
