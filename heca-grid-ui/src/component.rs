//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{
    Border, BracketCmd, DrawCommand, Glow, RectCmd, Scene, TextAlign, TextCmd,
};
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
    PointerMoved { pos: Point },
    PointerPressed { pos: Point },
    PointerReleased { pos: Point },
    /// Keyboard event — delivered to the focused component only.
    Key { key: GridKey, pressed: bool },
    /// Modifier keys changed — broadcast to the whole tree so widgets can track
    /// state (e.g. for word-wise editing). Observers should return `Handled::No`.
    ModifiersChanged(Modifiers),
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

/// Painting context handed to [`Component::paint`]. Wraps the [`Scene`] and the
/// active [`Theme`], and exposes the shared Tron drawing helpers.
pub struct PaintCx<'a> {
    scene: &'a mut Scene,
    theme: &'a Theme,
}

impl<'a> PaintCx<'a> {
    /// Create a painting context over `scene` using `theme`.
    pub fn new(scene: &'a mut Scene, theme: &'a Theme) -> Self {
        Self { scene, theme }
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
        self.rect(rect, Color::rgb(255, 255, 255).with_alpha(a), None, radius, None);
    }

    /// Dim `rect` with a background-colored scrim — the standard look for a
    /// **disabled** widget. `radius` must match the widget's corner radius so the
    /// scrim follows its rounded shape. DRY: every widget reuses this instead of
    /// dimming each color by hand.
    pub fn dim(&mut self, rect: Rectangle, radius: f32) {
        let a = (DISABLED_SCRIM * 255.0).round() as u8;
        self.rect(rect, self.theme.background.with_alpha(a), None, radius, None);
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

    /// Scale a glow by the theme intensity; drop it entirely when intensity is Off.
    fn scaled_glow(&self, glow: Option<Glow>) -> Option<Glow> {
        let scale = self.theme.intensity.glow_scale();
        glow.filter(|_| scale > 0.0).map(|g| Glow {
            intensity: g.intensity * scale,
            ..g
        })
    }
}
