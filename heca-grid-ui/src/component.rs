//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::reactive::{signal, Signal, SignalGet};
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

/// An input event delivered to the component tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    PointerMoved { pos: Point },
    PointerPressed { pos: Point },
    PointerReleased { pos: Point },
}

/// Behavior shared by all components. Implementors provide access to their
/// [`Base`]; `paint`/`event` have sensible container defaults.
pub trait Component {
    /// Borrow this component's base.
    fn base(&self) -> &Base;
    /// Mutably borrow this component's base.
    fn base_mut(&mut self) -> &mut Base;

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
