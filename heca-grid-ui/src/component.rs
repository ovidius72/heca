//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, BracketCmd, DrawCommand, Glow, RectCmd, Scene, TextAlign, TextCmd};
use crate::style::Style;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle, Size};

/// State shared by every component. Concrete widgets embed this.
pub struct Base {
    /// Layout + visual style.
    pub style: Style,
    /// This component's node in the layout tree (set during layout).
    pub node: Option<taffy::NodeId>,
    /// Absolute bounds in logical pixels, filled in after layout.
    pub bounds: Rectangle,
    /// Whether this component is rendered.
    pub visible: Signal<bool>,
    /// Whether this component is disabled — dimmed, non-interactive, and skipped
    /// by focus traversal. A common, base-level property every widget inherits.
    pub disabled: Signal<bool>,
    /// Whether this component currently holds keyboard focus.
    pub focused: Signal<bool>,
    /// Whether the focus ring should show — true for keyboard focus, false for
    /// mouse focus (focus-visible behavior).
    pub focus_visible: Signal<bool>,
    /// Explicit Tab-order index (like HTML `tabindex`). Focusables with an index
    /// are visited first in ascending order; those without (`None`) follow in
    /// tree position order. Set via [`LayoutExt::tab_index`](crate::builders::LayoutExt::tab_index).
    pub tab_index: Option<i32>,
    /// Child components, laid out by this component's flex container.
    pub children: Vec<Box<dyn Component>>,
    /// Resolved font size in logical px, written by the layout pass: the widget's
    /// own `style.font_size` if it set one (> 0), otherwise the theme's base font.
    /// Widgets read **this** for text + size, so a global font flows in for free.
    pub font: f32,
}

impl Base {
    /// A new base with default style and an empty child list.
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            node: None,
            bounds: Rectangle::from_size(Size::new(0.0, 0.0)),
            visible: signal(true),
            disabled: signal(false),
            focused: signal(false),
            focus_visible: signal(false),
            tab_index: None,
            children: Vec::new(),
            font: 15.0,
        }
    }
}

impl Default for Base {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an event handler: whether the event was consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    Yes,
    No,
}

/// A renderer-agnostic keyboard key. No `winit` types leak into this crate; the
/// host maps its platform keys onto this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridKey {
    Char(char),
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
}

/// Keyboard modifier state, renderer-agnostic. The host maps its platform
/// modifiers onto this and broadcasts changes via [`Event::ModifiersChanged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The Cmd/Super/Windows key.
    pub meta: bool,
}

/// An input event delivered to the component tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    PointerMoved {
        pos: Point,
    },
    PointerPressed {
        pos: Point,
    },
    PointerReleased {
        pos: Point,
    },
    /// Keyboard event — delivered to the focused component only.
    Key {
        key: GridKey,
        pressed: bool,
    },
    /// Modifier keys changed — broadcast to the whole tree so widgets can track
    /// state (e.g. for word-wise editing). Observers should return `Handled::No`.
    ModifiersChanged(Modifiers),
    /// Wheel/scroll by `delta` lines (positive = scroll down the content). The
    /// host routes this to the open overlay, or to the widget under the cursor.
    Scroll {
        delta: f32,
    },
}

/// Behavior shared by all components. Implementors provide access to their
/// [`Base`]; `paint`/`event` have sensible container defaults.
pub trait Component {
    /// Borrow this component's base.
    fn base(&self) -> &Base;
    /// Mutably borrow this component's base.
    fn base_mut(&mut self) -> &mut Base;

    /// Whether this component participates in keyboard focus traversal
    /// (Tab/Shift+Tab). Interactive widgets override this to `true`.
    fn focusable(&self) -> bool {
        false
    }

    /// Whether this component currently has an **open overlay** (e.g. a `Select`
    /// dropdown). The host routes pointer/key events to an overlay-active widget
    /// first, so it can capture clicks/keys outside its layout bounds. Default
    /// `false`; see [`FocusManager`](crate::focus::FocusManager).
    fn overlay_active(&self) -> bool {
        false
    }

    /// Emit draw commands. Default: paint the base chrome, then children.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base().visible.get_untracked() {
            return;
        }
        cx.paint_base(self.base());
        for child in &self.base().children {
            child.paint(cx);
        }
    }

    /// Handle an event. Default: route to children, last-added first.
    fn event(&mut self, ev: &Event) -> Handled {
        for child in self.base_mut().children.iter_mut().rev() {
            if child.event(ev) == Handled::Yes {
                return Handled::Yes;
            }
        }
        Handled::No
    }

    /// Called when this component gains keyboard focus. Default: set the focus
    /// flag (which drives the focus ring). Override to add behaviour.
    /// `visible` is true for keyboard focus (show the ring), false for mouse.
    fn on_focus(&mut self, visible: bool) {
        self.base_mut().focused.set(true);
        self.base_mut().focus_visible.set(visible);
    }

    /// Called when this component loses keyboard focus. Default: clear it.
    fn on_blur(&mut self) {
        self.base_mut().focused.set(false);
        self.base_mut().focus_visible.set(false);
    }

    /// The taffy style for this component's layout node. Default: the base
    /// style mapped via [`Style::to_taffy`]. Layout containers that need extra
    /// taffy config (e.g. [`Grid`](crate::widgets::Grid) injecting `display:
    /// grid` + track templates) override this.
    fn taffy_style(&self) -> taffy::Style {
        self.base().style.to_taffy()
    }

    /// Recompute size from the resolved font ([`Base::font`]). Widgets whose
    /// dimensions depend on font size override this; the layout pass calls it on
    /// every node after resolving the font, so a global font reflows the tree
    /// without per-widget wiring. Default: no-op.
    fn remeasure(&mut self) {}

    /// Advance time-based animations by `dt` seconds. Returns `true` if still
    /// animating, so the host can schedule another frame. Default: recurse.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base_mut().children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

/// Scrim alpha used to dim a disabled widget — applied by [`PaintCx::dim`].
const DISABLED_SCRIM: f32 = 0.55;

/// Bright bracket length along each edge of a [`PaintCx::bracket_frame`],
/// measured from the corner (in addition to the rounded arc). The straight
/// midsection between the two brackets on an edge is dimmed back to a line.
const BRACKET_ARM_LEN: f32 = 12.0;
/// Bright corner brackets are drawn thicker than the subtle border for emphasis.
const BRACKET_WIDTH_MUL: f32 = 2.0;
/// Alpha of the fill-colored overlay used to dim the straight border midsections.
/// ~0.7 over the bright accent border leaves a ~30% accent line — matching the
/// subtle continuous border, while the corners stay fully bright.
const BRACKET_STRAIGHT_DIM: u8 = 178;

/// Painting context handed to [`Component::paint`]. Wraps the [`Scene`] and the
/// active [`Theme`], and exposes the shared Tron drawing helpers.
pub struct PaintCx<'a> {
    scene: &'a mut Scene,
    theme: &'a Theme,
    /// Visible viewport size in logical px. Widgets that place overlays (e.g. a
    /// `Select` dropdown) use it to flip/cap against the screen. Defaults to
    /// "infinite" so non-host callers (tests) keep the open-below behavior.
    viewport: Size,
}

impl<'a> PaintCx<'a> {
    /// Create a painting context over `scene` using `theme`.
    pub fn new(scene: &'a mut Scene, theme: &'a Theme) -> Self {
        Self {
            scene,
            theme,
            viewport: Size::new(f64::MAX, f64::MAX),
        }
    }

    /// Set the visible viewport size (the host passes the window size).
    pub fn with_viewport(mut self, viewport: Size) -> Self {
        self.viewport = viewport;
        self
    }

    /// The visible viewport size in logical px.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Run `f` with draws routed to the scene's **overlay layer** (painted on
    /// top of everything). Used by popovers/dropdowns for correct z-order.
    pub fn with_overlay(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.begin_overlay();
        f(self);
        self.scene.end_overlay();
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Queue a rounded rectangle with optional border and glow.
    pub fn rect(
        &mut self,
        rect: Rectangle,
        fill: Color,
        border: Option<Border>,
        radius: f32,
        glow: Option<Glow>,
    ) {
        self.scene.push(DrawCommand::Rect(RectCmd {
            rect,
            fill,
            border,
            radius,
            glow: self.scaled_glow(glow),
        }));
    }

    /// Queue **flat** L-shaped corner brackets framing `rect` — prominent angles
    /// with no glow (for container/pane chrome, vs the glowing focus ring).
    pub fn corner_brackets_plain(&mut self, rect: Rectangle, color: Color) {
        self.scene.push(DrawCommand::Brackets(BracketCmd {
            rect,
            color,
            len: 16.0,
            thickness: 1.5,
            glow: None,
        }));
    }

    /// Flat corner brackets with a custom arm length. Use `len = radius` so the
    /// bracket arms end exactly where a rounded border's arc begins.
    pub fn corner_brackets_len(&mut self, rect: Rectangle, color: Color, len: f32) {
        self.scene.push(DrawCommand::Brackets(BracketCmd {
            rect,
            color,
            len,
            thickness: 1.5,
            glow: None,
        }));
    }

    /// Queue L-shaped corner brackets framing `rect` (a Tron reticle).
    pub fn corner_brackets(&mut self, rect: Rectangle, color: Color) {
        let glow = self.scaled_glow(Some(Glow {
            color,
            radius: 6.0,
            intensity: 1.0,
        }));
        self.scene.push(DrawCommand::Brackets(BracketCmd {
            rect,
            color,
            len: 12.0,
            thickness: 1.5,
            glow,
        }));
    }

    /// Queue a text run within `rect` at an explicit logical `size`. The renderer
    /// centers the text within `rect` (per `align` horizontally, always centered
    /// vertically) using real glyph metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        rect: Rectangle,
        text: &str,
        color: Color,
        size: f32,
        align: TextAlign,
        bold: bool,
    ) {
        self.scene.push(DrawCommand::Text(TextCmd {
            rect,
            text: text.to_string(),
            color,
            size,
            align,
            bold,
        }));
    }

    /// Queue a brightening "press flash" overlay over `rect`. `amount` is the
    /// flash strength in `0.0..=1.0` (see [`Flash`](crate::effects::Flash));
    /// `radius` must match the widget's corner radius so the overlay follows a
    /// rounded shape instead of poking square corners past it.
    pub fn flash(&mut self, rect: Rectangle, amount: f32, radius: f32) {
        if amount <= 0.0 {
            return;
        }
        let a = (amount.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.rect(
            rect,
            Color::rgb(255, 255, 255).with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Dim `rect` with a background-colored scrim — the standard look for a
    /// **disabled** widget. `radius` must match the widget's corner radius so the
    /// scrim follows its rounded shape. DRY: every widget reuses this instead of
    /// dimming each color by hand.
    pub fn dim(&mut self, rect: Rectangle, radius: f32) {
        let a = (DISABLED_SCRIM * 255.0).round() as u8;
        self.rect(
            rect,
            self.theme.background.with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Draw the prominent **flat corner-bracket frame** used by container chrome
    /// ([`Pane`](crate::widgets::Pane), [`DockFrame`](crate::widgets::DockFrame)):
    /// a bright accent border tracing the full rounded perimeter, with the
    /// straight midsection of each edge dimmed back to a subtle ~30% line — so
    /// only the rounded corners plus a short arm stay bright. `fill` is the
    /// container's own fill (used to color the dimming overlay so it blends in);
    /// it falls back to the theme background. DRY: the frame is defined once here
    /// instead of per-widget.
    pub fn bracket_frame(&mut self, rect: Rectangle, fill: Option<Color>) {
        let (accent, background, radius, border_width) = {
            let t = self.theme;
            (t.accent, t.background, t.radius, t.border_width)
        };
        let b = rect;

        // Bright accent border tracing the full rounded perimeter. The renderer's
        // bracket primitive only draws square 90° corners, so instead of brackets
        // we draw a full rounded border (which has the radius) and then dim its
        // straight midsections — leaving the rounded corners + short arms bright.
        let bracket_width = border_width * BRACKET_WIDTH_MUL;
        self.rect(
            b,
            Color::TRANSPARENT,
            Some(Border { color: accent, width: bracket_width }),
            radius,
            None,
        );

        // Dim the straight midsection of each edge back to a subtle ~30% line, so
        // only the rounded corners (plus a `BRACKET_ARM_LEN` arm) stay bright. The
        // overlay is the fill (or background) color at ~0.7 alpha, with no glow.
        let cover = fill.unwrap_or(background).with_alpha(BRACKET_STRAIGHT_DIM);
        let keep = f64::from(radius + BRACKET_ARM_LEN);
        let t = f64::from(bracket_width) + 1.0;
        let (x, y, w, h) = (b.loc.x, b.loc.y, b.size.w, b.size.h);

        let mid_w = w - 2.0 * keep;
        if mid_w > 0.0 {
            self.rect(Rectangle::new(Point::new(x + keep, y), Size::new(mid_w, t)), cover, None, 0.0, None);
            self.rect(Rectangle::new(Point::new(x + keep, y + h - t), Size::new(mid_w, t)), cover, None, 0.0, None);
        }
        let mid_h = h - 2.0 * keep;
        if mid_h > 0.0 {
            self.rect(Rectangle::new(Point::new(x, y + keep), Size::new(t, mid_h)), cover, None, 0.0, None);
            self.rect(Rectangle::new(Point::new(x + w - t, y + keep), Size::new(t, mid_h)), cover, None, 0.0, None);
        }
    }

    /// Paint the shared chrome for a component's base (background/border/glow).
    pub fn paint_base(&mut self, base: &Base) {
        let s = &base.style;
        if s.fill.is_none() && s.border.is_none() && s.glow.is_none() {
            return;
        }
        self.rect(
            base.bounds,
            s.fill.unwrap_or(Color::TRANSPARENT),
            s.border,
            s.radius,
            s.glow,
        );
    }

    /// Scale a glow by the theme's `glow_size` token (the **sole** owner of glow:
    /// presence + halo radius). `intensity` is intentionally *not* applied here —
    /// it controls only the scanline/CRT overlay — so the two settings no longer
    /// overlap. `GlowLevel::None` drops the glow entirely.
    fn scaled_glow(&self, glow: Option<Glow>) -> Option<Glow> {
        let size = self.theme.glow_size.radius_scale();
        glow.filter(|_| size > 0.0).map(|g| Glow {
            radius: g.radius * size,
            ..g
        })
    }
}
