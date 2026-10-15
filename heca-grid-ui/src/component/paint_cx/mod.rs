//! **The painting context** handed to [`Component::paint`]: the scene, the theme, and what is in
//! force where a widget paints.

use super::*;

mod chrome;
mod draw;
mod layers;
mod transform;

/// Scrim alpha used to dim a disabled widget — applied by [`PaintCx::dim`].
const DISABLED_SCRIM: f32 = 0.55;

/// Logical-px slack around the viewport kept un-culled, so a shape's glow/shadow
/// halo spilling in from just off-screen still draws. See [`PaintCx::culled`].
const CULL_MARGIN: f64 = 96.0;

/// Bright bracket length along each edge of a [`PaintCx::bracket_frame`],
/// measured from the corner (in addition to the rounded arc). The straight
/// midsection between the two brackets on an edge is dimmed back to a line.
const BRACKET_ARM_LEN: f32 = 12.0;
/// Bright corner brackets are drawn a touch thicker than the subtle border for
/// emphasis — an **additive** boost so they don't balloon at large border widths
/// (a multiplier made them far too heavy at e.g. `border_width = 3`).
const BRACKET_WIDTH_BOOST: f32 = 1.0;
/// Alpha of the subtle continuous accent line that traces the whole perimeter of
/// a [`PaintCx::bracket_frame`] — the dimmed "midsection" the bright corners sit
/// on top of. ~0.31 leaves a faint accent line.
const BRACKET_DIM_ALPHA: u8 = 80;

/// Painting context handed to [`Component::paint`]. Wraps the [`Scene`] and the
/// active [`Theme`], and exposes the shared Tron drawing helpers.
pub struct PaintCx<'a> {
    pub(super) scene: &'a mut Scene,
    pub(super) theme: &'a Theme,
    /// Visible viewport size in logical px. Widgets that place overlays (e.g. a
    /// `Select` dropdown) use it to flip/cap against the screen. Defaults to
    /// "infinite" so non-host callers (tests) keep the open-below behavior.
    pub(super) viewport: Size,
    /// The **inherited content color** for the subtree currently being painted — see
    /// [`with_content_color`](Self::with_content_color). `None` at the root.
    pub(super) content_color: Option<Color>,
    /// The inherited **control tone** for the subtree currently being painted — see
    /// [`with_accent`](Self::with_accent). `None` at the root, and `None` almost
    /// everywhere: a container has to say it wants the controls inside it to follow its hue.
    pub(super) published_accent: Option<Color>,
    pub(super) content_glow: Option<Glow>,
    /// Translation applied to every draw emitted through this context — see
    /// [`with_translate`](Self::with_translate). `(0, 0)` normally: a widget paints at its bounds.
    pub(super) offset: (f64, f64),
    /// Alpha multiplier applied to every draw emitted through this context — see
    /// [`with_opacity`](Self::with_opacity). `1.0` normally: a widget paints at its own colours.
    pub(super) opacity: f32,
    /// Whether what is painted is **a picture of something that lives elsewhere** — see
    /// [`with_echo`](Self::with_echo).
    pub(super) echo: bool,
    /// The active [scale](PaintCx::with_scale) — multiplicative, like `opacity`.
    pub(super) scale: f32,
    /// The fixed point the scale shrinks toward, already in scene coordinates.
    pub(super) scale_origin: Point,
    /// The clip currently in force — every open [`with_clip`](Self::with_clip) intersected, in the
    /// same space the callers passed (a widget's own layout coordinates, before
    /// [`placed`](Self::placed)), so a value read off `Base::bounds` can be compared with it
    /// directly. `None` is *"nothing clips this"*.
    ///
    /// Read by [`clip`](Self::clip) — a widget that draws into the **overlay band** needs it,
    /// because that band starts unclipped and geometry alone would let a keycap land outside the
    /// dock it belongs to (F003/P082/T438).
    pub(super) clip: Option<Rectangle>,
    /// The nearest clip with round corners, already placed and scaled into the scene — see
    /// [`with_rounded_clip`](Self::with_rounded_clip). `None` when nothing round clips this.
    pub(super) rounded: Option<crate::scene::Rounded>,
    /// What the widget being painted cuts its children to, in its own layout coordinates — see
    /// [`Component::children_clip`]. Applied by [`paint_child`] to each child it draws.
    pub(super) children_clip: Option<crate::scene::Rounded>,
}

impl<'a> PaintCx<'a> {
    /// Create a painting context over `scene` using `theme`.
    pub fn new(scene: &'a mut Scene, theme: &'a Theme) -> Self {
        Self {
            scene,
            theme,
            viewport: Size::new(f64::MAX, f64::MAX),
            content_color: None,
            published_accent: None,
            content_glow: None,
            offset: (0.0, 0.0),
            opacity: 1.0,
            echo: false,
            scale: 1.0,
            scale_origin: Point::new(0.0, 0.0),
            clip: None,
            rounded: None,
            children_clip: None,
        }
    }
}
