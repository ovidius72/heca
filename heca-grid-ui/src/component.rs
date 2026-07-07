//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::drag::{DragItemId, DropSide};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, DrawCommand, FontRole, Glow, RectCmd, Scene, Shadow, TextAlign, TextCmd};
use crate::style::Style;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

thread_local! {
    /// Host-installed hook the widget tree calls to schedule the next frame. The
    /// event loop wires it to its redraw request; widgets reach it via
    /// [`request_frame`]. Thread-local because the UI runs single-threaded.
    static FRAME_REQUEST: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Install the callback the widget tree uses to ask the host for the next frame.
///
/// The host (event loop) wires this to its "schedule a redraw" call once at
/// startup. It is the bridge that lets a widget which invalidates itself (an
/// animation step, a caret move, a reactive state change) wake the renderer —
/// the foundation for repainting only what changed instead of every frame.
pub fn install_frame_request(f: impl Fn() + 'static) {
    FRAME_REQUEST.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Ask the host to schedule a frame. The host coalesces repeated requests into a
/// single redraw. A no-op until [`install_frame_request`] is set (e.g. in
/// headless tests), so widget code can always call it safely.
pub fn request_frame() {
    FRAME_REQUEST.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f();
        }
    });
}

/// Logical-pixel margin added around each damaged widget so glow/shadow halos —
/// which paint outside the widget's rect — are included in the redrawn region.
const DAMAGE_PAD: f64 = 64.0;

/// Walk the tree and union the bounds of every widget flagged
/// [`needs_paint`](Base::needs_paint) (padded for glow/shadow reach), **clearing
/// the flags**. Returns the damage rect to repaint, or `None` if nothing changed.
///
/// The host calls this each frame: on an animation/timed frame the result scissors
/// the render to just the changed pixels; on an input frame the host repaints in
/// full (an event can change unknown things) but still calls this to clear flags.
/// Hidden subtrees are skipped — their bounds are stale.
pub fn collect_damage(root: &dyn Component) -> Option<Rectangle> {
    fn union(a: Rectangle, b: Rectangle) -> Rectangle {
        let x0 = a.loc.x.min(b.loc.x);
        let y0 = a.loc.y.min(b.loc.y);
        let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
        let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
        Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
    }
    fn walk(c: &dyn Component, acc: &mut Option<Rectangle>) {
        let b = c.base();
        if !b.visible.get_untracked() || b.style.hidden {
            return;
        }
        if b.needs_paint() {
            b.clear_needs_paint();
            // Overlay widgets (tooltip bubble, command palette) paint outside their
            // own `bounds`; `damage_bounds` lets them report the rect that actually
            // changed. Default is `bounds`, so ordinary widgets are unaffected.
            let r = c.damage_bounds();
            let padded = Rectangle::new(
                Point::new(r.loc.x - DAMAGE_PAD, r.loc.y - DAMAGE_PAD),
                Size::new(r.size.w + 2.0 * DAMAGE_PAD, r.size.h + 2.0 * DAMAGE_PAD),
            );
            *acc = Some(acc.map_or(padded, |a| union(a, padded)));
        }
        for ch in &b.children {
            walk(ch.as_ref(), acc);
        }
    }
    let mut acc = None;
    walk(root, &mut acc);
    acc
}

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
    /// If set, this widget is a **drag source**: a press inside its bounds can
    /// begin a drag carrying this opaque id (the app maps it back to a pane /
    /// column / etc.). Universal opt-in via [`DragExt::draggable`](crate::builders::DragExt::draggable);
    /// resolved generically by [`drag::source_at`](crate::drag::source_at).
    pub drag_source: Option<DragItemId>,
    /// If set, this widget is a **drop target**: a drag released over its bounds
    /// drops onto this opaque id. Universal opt-in via
    /// [`DragExt::drop_target`](crate::builders::DragExt::drop_target); resolved
    /// generically by [`drag::resolve_at`](crate::drag::resolve_at).
    pub drop_target: Option<DragItemId>,
    /// If set, this widget is a **hint target**: the universal leader/vimium
    /// picker assigns it a letter and, on the keypress, the host fires the intent
    /// it mapped this opaque id to. Universal opt-in via
    /// [`HintExt::hint_target`](crate::builders::HintExt::hint_target); enumerated
    /// generically by [`hint::collect_hint_targets`](crate::hint::collect_hint_targets).
    pub hint_target: Option<crate::hint::HintTargetId>,
    /// Resolved font size in logical px, written by the layout pass: the widget's
    /// own `style.font_size` if it set one (> 0), otherwise the theme's base font.
    /// Widgets read **this** for text + size, so a global font flows in for free.
    pub font: f32,
    /// Repaint flag for the retained renderer: set when this widget's visuals
    /// changed and cleared once it's repainted. Starts `true` (everything paints
    /// on the first frame). The renderer repaints only widgets whose flag is set,
    /// and unions their bounds into the frame's damage region.
    needs_paint: Cell<bool>,
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
            drag_source: None,
            drop_target: None,
            hint_target: None,
            font: 15.0,
            needs_paint: Cell::new(true),
        }
    }

    /// Mark this widget as needing a repaint and ask the host for a frame. Call on
    /// a visual change the host wouldn't otherwise know about — an animation step,
    /// a caret move, an imperative state edit. (Reactive state changes route here
    /// too, at their mutation site.)
    pub fn mark_needs_paint(&self) {
        self.needs_paint.set(true);
        request_frame();
    }

    /// Whether this widget needs repainting.
    pub fn needs_paint(&self) -> bool {
        self.needs_paint.get()
    }

    /// Clear the repaint flag — the renderer calls this once the widget is painted.
    pub fn clear_needs_paint(&self) {
        self.needs_paint.set(false);
    }

    /// The widget's size-variant **padding/dimension** multiplier — what widgets
    /// multiply their intrinsic padding / fixed dims by in `remeasure`. (The font is
    /// scaled separately, by [`WidgetSize::font_scale`], during layout.) At `Small`
    /// this is tighter than the font so controls get compact, not just smaller. See
    /// [`WidgetSize`](crate::style::WidgetSize).
    pub fn size_scale(&self) -> f32 {
        self.style.size.pad_scale()
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
            paint_child(child.as_ref(), cx);
        }
    }

    /// Handle an event. Default: route to children, last-added first.
    fn event(&mut self, ev: &Event) -> Handled {
        route_event(&mut self.base_mut().children, ev)
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

    /// Called by the layout engine **after** this node's bounds (and all its
    /// descendants' bounds) have been (re)computed and written to `Base::bounds`.
    /// Post-order: children fire before the parent. Default: no-op. Override to
    /// react to a fresh layout — e.g. a scroll viewport resets any shift it had
    /// baked into its children's bounds (they are now back at their natural
    /// positions), so the next paint re-applies the shift from scratch instead
    /// of compounding. This is what lets an embeddable scroll viewport reuse the
    /// whole-page scroll pattern (shift subtree bounds + clip) without owning the
    /// layout/scroll cycle.
    fn on_layout(&mut self) {}

    /// Advance time-based animations by `dt` seconds. Returns `true` if a
    /// **continuous** animation is still running (eases, slides, spinners), so the
    /// host schedules another frame at its frame cap. Default: recurse.
    ///
    /// A widget whose visual changes only at sparse, known moments (e.g. a blinking
    /// caret toggling ~twice a second) should return `false` here and instead report
    /// the time to its next change via [`next_redraw`](Self::next_redraw), so the
    /// host can sleep until then rather than redrawing every frame.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base_mut().children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// Seconds until this subtree next needs a **timed** redraw (independent of the
    /// continuous-animation signal from [`tick`](Self::tick)) — e.g. a focused
    /// `Input`'s caret returns the time to its next blink toggle. The host wakes at
    /// the soonest such time across the tree instead of redrawing continuously.
    /// `None` = no timed redraw pending. Default: the soonest across children.
    fn next_redraw(&self) -> Option<f32> {
        let mut soonest = None;
        for child in self.base().children.iter() {
            soonest = soonest_redraw(soonest, child.next_redraw());
        }
        soonest
    }

    /// The rect (logical px) to repaint when this widget is flagged
    /// [`needs_paint`](Base::needs_paint) — used by [`collect_damage`] in place of
    /// `bounds`. Overlay widgets that paint **outside** their own bounds (a tooltip
    /// bubble, a command-palette panel) override this to report where they actually
    /// draw, so a redraw covers the popover rather than the (often unrelated) layout
    /// box. Default: the widget's own `bounds`.
    fn damage_bounds(&self) -> Rectangle {
        self.base().bounds
    }

    /// The opaque drag-source id if this widget is draggable (see
    /// [`Base::drag_source`]). Default reads the base; widgets needing dynamic
    /// behavior may override. Walked by [`drag::source_at`](crate::drag::source_at).
    fn as_drag_source(&self) -> Option<DragItemId> {
        self.base().drag_source
    }

    /// The opaque drop-target id if this widget accepts drops (see
    /// [`Base::drop_target`]). Default reads the base. Walked by
    /// [`drag::resolve_at`](crate::drag::resolve_at).
    fn as_drop_target(&self) -> Option<DragItemId> {
        self.base().drop_target
    }

    /// The opaque hint-target id if this widget is a leader/vimium pick target (see
    /// [`Base::hint_target`]). Default reads the base. Walked by
    /// [`hint::collect_hint_targets`](crate::hint::collect_hint_targets).
    fn as_hint_target(&self) -> Option<crate::hint::HintTargetId> {
        self.base().hint_target
    }
}

/// Combine two "seconds until next redraw" requests, keeping the sooner one
/// (`None` means "no request").
pub fn soonest_redraw(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Route `ev` to `children` last-added first (top z-order wins), stopping at the
/// first that consumes it. This is the default [`Component::event`] behavior,
/// exposed so containers that *wrap* event handling — e.g. detecting a toggle
/// after delegating to a header (see [`ItemGroup`](crate::widgets::ItemGroup),
/// [`DockFrame`](crate::widgets::DockFrame)) — reuse it instead of re-rolling the
/// reverse loop.
pub(crate) fn route_event(children: &mut [Box<dyn Component>], ev: &Event) -> Handled {
    for child in children.iter_mut().rev() {
        if child.event(ev) == Handled::Yes {
            return Handled::Yes;
        }
    }
    Handled::No
}

/// Paint a child, unless it is hidden via `style.hidden` (taffy `display: none`).
/// A `display: none` subtree is collapsed to zero size at the top-left by layout,
/// so painting it would stamp its (stale, overlapping) contents there — every
/// container skips hidden children instead, matching the web. Containers with
/// bespoke paint loops (e.g. [`Pane`](crate::widgets::Pane),
/// [`DockFrame`](crate::widgets::DockFrame)) reuse this so a collapsed body/group
/// never bleeds onto the rest of the tree.
pub(crate) fn paint_child(c: &dyn Component, cx: &mut PaintCx) {
    if c.base().style.hidden {
        return;
    }
    c.paint(cx);
}

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

    /// Whether `r` lies fully outside the viewport (plus a halo margin for
    /// glow/shadow spill) and can be skipped. Off-screen content emits no draw
    /// command, so scrolling a tall page or maximizing the window doesn't pay to
    /// paint / shape / upload what isn't visible. The default viewport is
    /// "infinite" (headless), where nothing is ever culled; overlays draw
    /// on-screen, so they're never culled either.
    fn culled(&self, r: Rectangle) -> bool {
        let vp = self.viewport;
        r.loc.y + r.size.h < -CULL_MARGIN
            || r.loc.y > vp.h + CULL_MARGIN
            || r.loc.x + r.size.w < -CULL_MARGIN
            || r.loc.x > vp.w + CULL_MARGIN
    }

    /// Run `f` with draws routed to the scene's **overlay layer** (painted on
    /// top of everything). Used by popovers/dropdowns for correct z-order.
    pub fn with_overlay(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.begin_overlay();
        f(self);
        self.scene.end_overlay();
    }

    /// Run `f` with all its draws **clipped** to `rect` (logical px). Content that
    /// falls outside is scissored away by the renderer — the primitive a scrolling
    /// viewport uses so partial rows/glyphs are cut at the panel edge instead of
    /// spilling out. Nested clips intersect with their parent.
    pub fn with_clip(&mut self, rect: Rectangle, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.push(DrawCommand::PushClip(rect));
        f(self);
        self.scene.push(DrawCommand::PopClip);
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
        if self.culled(rect) {
            return;
        }
        self.scene.push(DrawCommand::Rect(RectCmd {
            rect,
            fill,
            border,
            radius,
            glow: self.scaled_glow(glow),
            shadow: None,
        }));
    }

    /// Queue a soft **drop shadow** for `rect` (corner `radius`): a dark, blurred,
    /// offset halo drawn *behind* it that lifts the shape off the background. Call
    /// this **before** painting the shape's fill, so the shape occludes the
    /// shadow's center and only its fringe shows. Unlike [`glow`](PaintCx::rect),
    /// the shadow darkens (composites a dark color with alpha), so it reads on
    /// dark themes — and it's independent of the glow/border tokens, so it shows
    /// even when both are off (e.g. a floating Modal at `border_width == 0`).
    pub fn drop_shadow(&mut self, rect: Rectangle, radius: f32, shadow: Shadow) {
        if shadow.color.a == 0 || shadow.radius <= 0.0 {
            return;
        }
        self.scene.push(DrawCommand::Rect(RectCmd {
            rect,
            fill: Color::TRANSPARENT,
            border: None,
            radius,
            glow: None,
            shadow: Some(shadow),
        }));
    }

    /// Build a box border in `color` at the **theme's** [`border_width`](crate::theme::Theme::border_width),
    /// or `None` when borders are off (`border_width == 0`). Widgets should build
    /// their box border with this instead of hardcoding a stroke, so they all
    /// honor the token (and disappear together at width 0). The single chokepoint
    /// that keeps border width theme-driven across the widget set.
    pub fn border(&self, color: Color) -> Option<Border> {
        let w = self.theme.colors.border_width;
        (w > 0.0).then_some(Border { color, width: w })
    }

    /// Queue the **thin-outline focus indicator**: an accent-toned outline drawn *just outside*
    /// `rect` (a CSS-style `outline` with an offset gap), with a restrained halo. An alternative to
    /// the corner-bracket reticle ([`corner_brackets`](Self::corner_brackets)).
    ///
    /// This is **the** keyboard focus indicator for the whole widget set (Button, IconButton,
    /// Toggle, Checkbox, Input, Select, Tabs, Item, Row, RailCell, Toast, BadgeButton, ScrollRegion,
    /// …). Because it sits **outside** the widget box rather than on its edge, it is visible whether
    /// or not the widget draws its own border — borderless variants (e.g. Ghost/Link buttons) get
    /// the same clear ring — and it never merges into the widget's own border. It is purely drawn
    /// (it does not affect layout), so it can overlap neighbouring padding like a real focus outline.
    /// `radius` is the widget's own corner radius; the outline widens it by the offset to stay
    /// concentric. Width uses the [`focus_border_width`](crate::theme::Theme::focus_border_width)
    /// token (its own width, so it stays visible even when decorative borders are off), and the halo
    /// is scaled by `glow_size` inside [`rect`](Self::rect) — dropping to nothing when glow is
    /// `none`. The glow is kept low so it reads as a focus cue, not an alarm. Pair with the theme's
    /// [`effective_focus_ring`](crate::theme::Theme::effective_focus_ring) /
    /// [`focus_ring_tone`](crate::theme::Theme::focus_ring_tone) for the color. (The decorative
    /// corner-bracket reticle is a different primitive — [`bracket_frame`](Self::bracket_frame).)
    pub fn focus_ring(&mut self, rect: Rectangle, color: Color, radius: f32) {
        // Offset gap (logical px) between the widget edge and the outline — like CSS `outline-offset`.
        const OFFSET: f32 = 2.0;
        let o = OFFSET as f64;
        let outset = Rectangle::new(
            Point::new(rect.loc.x - o, rect.loc.y - o),
            Size::new(rect.size.w + 2.0 * o, rect.size.h + 2.0 * o),
        );
        let border = Border {
            color,
            width: self.theme.focus_border_width,
        };
        // `rect` runs the glow through `scaled_glow`, so this halo tracks `glow_size`
        // (and vanishes at `none`) exactly like every other glow in the library.
        let glow = Some(Glow {
            color,
            radius: 3.0,
            intensity: 0.3,
        });
        self.rect(outset, Color::TRANSPARENT, Some(border), radius + OFFSET, glow);
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
        if self.culled(rect) {
            return;
        }
        self.scene.push(DrawCommand::Text(TextCmd {
            rect,
            text: text.to_string(),
            color,
            size,
            align,
            bold,
            font: FontRole::Text,
        }));
    }

    /// Queue a single icon glyph centered in `rect`, shaped with the icon font
    /// ([`FontRole::Icon`]). `glyph` is the codepoint as a string; the renderer
    /// selects the embedded icon family. Used by [`Icon`](crate::widgets::Icon).
    pub fn icon(&mut self, rect: Rectangle, glyph: &str, color: Color, size: f32) {
        if self.culled(rect) {
            return;
        }
        self.scene.push(DrawCommand::Text(TextCmd {
            rect,
            text: glyph.to_string(),
            color,
            size,
            align: TextAlign::Center,
            bold: false,
            font: FontRole::Icon,
        }));
    }

    /// Queue a theme-tinted "press flash" overlay over `rect`. `amount` is the
    /// flash strength in `0.0..=1.0` (see [`Flash`](crate::effects::Flash));
    /// `radius` must match the widget's corner radius so the overlay follows a
    /// rounded shape instead of poking square corners past it.
    pub fn flash(&mut self, rect: Rectangle, amount: f32, radius: f32) {
        if amount <= 0.0 {
            return;
        }
        let a = (amount.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.rect(rect, self.theme.colors.foreground.with_alpha(a), None, radius, None);
    }

    /// Draw the **drag ghost** — the small labelled chip that follows the cursor
    /// during a drag. Theme-driven: an accent-filled rounded rect with the label in
    /// the background color, painted on the **overlay layer** so it sits above all
    /// chrome. `rect` is the chip's bounds (the app positions it at the cursor).
    ///
    /// When `swap` is true the chip gains an inset **double frame** — the same motif
    /// as [`swap_indicator`](Self::swap_indicator) — so the cursor-following chip tells
    /// the user this drag is an **exchange**, not a move (there is no OS "swap" cursor).
    pub fn drag_ghost(&mut self, rect: Rectangle, text: &str, swap: bool) {
        let (accent, bg, radius) = (self.theme.colors.accent, self.theme.colors.background, self.theme.colors.border_radius);
        let font = (rect.size.h as f32 * 0.55).clamp(10.0, 15.0);
        self.with_overlay(|cx| {
            cx.rect(rect, accent.with_alpha(217), None, radius, None);
            cx.rect(rect, Color::TRANSPARENT, Some(Border { color: accent, width: 1.5 }), radius, None);
            if swap {
                let inset = 3.0_f64;
                let inner = Rectangle::new(
                    Point::new(rect.loc.x + inset, rect.loc.y + inset),
                    Size::new((rect.size.w - inset * 2.0).max(0.0), (rect.size.h - inset * 2.0).max(0.0)),
                );
                cx.rect(
                    inner,
                    Color::TRANSPARENT,
                    Some(Border { color: bg.with_alpha(180), width: 1.0 }),
                    (radius - inset as f32).max(0.0),
                    None,
                );
            }
            cx.text(rect, text, bg, font, TextAlign::Center, false);
        });
    }

    /// Draw a **drop indicator** over a target's `bounds`: an accent insertion line
    /// for [`DropSide::Before`]/[`After`](DropSide), or an accent wash + outline for
    /// [`DropSide::Onto`]. Theme-driven (derives from `accent`); the app calls this
    /// over the bounds returned by [`resolve_at`](crate::drag::resolve_at).
    pub fn drop_indicator(&mut self, bounds: Rectangle, side: DropSide) {
        let accent = self.theme.colors.accent;
        match side {
            DropSide::Onto => {
                self.rect(bounds, accent.with_alpha(45), None, self.theme.colors.border_radius, None);
                self.rect(
                    bounds,
                    Color::TRANSPARENT,
                    Some(Border { color: accent, width: 1.5 }),
                    self.theme.colors.border_radius,
                    None,
                );
            }
            DropSide::Before | DropSide::After => {
                let thickness = 2.0;
                let y = if side == DropSide::Before {
                    bounds.loc.y - thickness / 2.0
                } else {
                    bounds.loc.y + bounds.size.h - thickness / 2.0
                };
                self.rect(
                    Rectangle::new(Point::new(bounds.loc.x, y), Size::new(bounds.size.w, thickness)),
                    accent,
                    None,
                    1.0,
                    None,
                );
            }
        }
    }

    /// Draw a **swap indicator** over a target's whole `bounds` — for an *exchange*
    /// gesture (Shift+drag) where the entire target is swapped with the source. There
    /// is no before/after for a swap, so this deliberately draws a whole-item **double
    /// accent frame** (+ faint wash) instead of [`drop_indicator`](Self::drop_indicator)'s
    /// insertion line or `Onto` wash — a distinct cue that the whole item is the target.
    /// Theme-driven (derives from `accent` / `radius` / `border_width`).
    pub fn swap_indicator(&mut self, bounds: Rectangle) {
        let accent = self.theme.colors.accent;
        let radius = self.theme.colors.border_radius;
        // Faint wash + bold outer frame.
        self.rect(bounds, accent.with_alpha(28), None, radius, None);
        let outer_w = (self.theme.colors.border_width * 2.0).max(2.5);
        self.rect(
            bounds,
            Color::TRANSPARENT,
            Some(Border { color: accent, width: outer_w }),
            radius,
            None,
        );
        // Inset second line → reads as a "double frame" (= swap the whole item),
        // visually separating it from the single-outline `Onto` look.
        let inset = 3.0_f64;
        let inner = Rectangle::new(
            Point::new(bounds.loc.x + inset, bounds.loc.y + inset),
            Size::new(
                (bounds.size.w - inset * 2.0).max(0.0),
                (bounds.size.h - inset * 2.0).max(0.0),
            ),
        );
        self.rect(
            inner,
            Color::TRANSPARENT,
            Some(Border { color: accent.with_alpha(120), width: 1.0 }),
            (radius - inset as f32).max(0.0),
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
            self.theme.colors.background.with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Draw the prominent **flat corner-bracket frame** used by container chrome
    /// ([`Pane`](crate::widgets::Pane), [`DockFrame`](crate::widgets::DockFrame)):
    /// a subtle continuous accent line tracing the full rounded perimeter, with
    /// bright thick accent **corners** (rounded arc + a short straight arm along
    /// each edge) layered on top — the Tron reticle. DRY: the frame is defined
    /// once here instead of per-widget.
    ///
    /// Width/radius come from `theme.colors.border_width`/`theme.colors.border_radius`: at
    /// `border_width == 0` the frame draws **nothing** (no border anywhere),
    /// consistent with every other widget's border gate. For a surface that
    /// carries its own width/radius (e.g. a self-themed sidebar shell), use
    /// [`bracket_frame_with`](Self::bracket_frame_with).
    pub fn bracket_frame(&mut self, rect: Rectangle) {
        let (radius, border_width) = (self.theme.colors.border_radius, self.theme.colors.border_width);
        self.bracket_frame_with(rect, border_width, radius);
    }

    /// [`bracket_frame`](Self::bracket_frame) with an explicit `border_width` and
    /// corner `radius`, independent of the theme — so one surface can size its own
    /// reticle (the per-widget `Pane::border_width`/radius overrides). The accent
    /// color still comes from the theme. `border_width <= 0.0` ⇒ no frame.
    pub fn bracket_frame_with(&mut self, rect: Rectangle, border_width: f32, radius: f32) {
        let accent = self.theme.colors.accent;
        // Borders off (`border_width == 0`) ⇒ no frame at all, like every other
        // widget. A container that needs definition without a border should carry
        // a fill, not a forced hairline.
        if border_width <= 0.0 {
            return;
        }
        let b = rect;

        // Subtle continuous accent line tracing the whole rounded perimeter — the
        // dimmed "midsection" that the bright corners sit on top of.
        self.rect(
            b,
            Color::TRANSPARENT,
            Some(Border {
                color: accent.with_alpha(BRACKET_DIM_ALPHA),
                width: border_width,
            }),
            radius,
            None,
        );

        // Bright thick corner brackets. The renderer draws the accent border as a
        // band just OUTSIDE the rect edge, carrying the theme corner radius — so
        // redrawing that same border clipped to a corner-sized box yields a bright
        // *rounded* corner plus a short straight arm along each edge, without
        // faking the rounding. `keep` is the bright span from each corner (rounded
        // arc + `BRACKET_ARM_LEN`); `m` grows the clip box outward so it also
        // captures the outer border band hugging the corner.
        let bracket_width = border_width + BRACKET_WIDTH_BOOST;
        let keep = f64::from(radius + BRACKET_ARM_LEN);
        let m = f64::from(bracket_width);
        let (x, y, w, h) = (b.loc.x, b.loc.y, b.size.w, b.size.h);
        let bright = Border { color: accent, width: bracket_width };

        let corners = [
            Point::new(x - m, y - m),               // top-left
            Point::new(x + w - keep, y - m),        // top-right
            Point::new(x - m, y + h - keep),        // bottom-left
            Point::new(x + w - keep, y + h - keep), // bottom-right
        ];
        for loc in corners {
            self.with_clip(Rectangle::new(loc, Size::new(keep + m, keep + m)), |cx| {
                cx.rect(b, Color::TRANSPARENT, Some(bright), radius, None);
            });
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
        // `GlowLevel` (theme/config `glow_size`) owns BOTH dimensions of glow:
        // `radius_scale` (halo size) and `strength_scale` (alpha). Apply both here —
        // the single chokepoint — so every widget's glow tracks the config uniformly
        // instead of baking a fixed intensity. `Medium` (default) is 1.0×, so this is
        // a no-op for the default theme; `none` drops glow entirely.
        let radius = self.theme.colors.glow_size.radius_scale();
        let strength = self.theme.colors.glow_size.strength_scale();
        glow.filter(|_| radius > 0.0).map(|g| Glow {
            radius: g.radius * radius,
            intensity: g.intensity * strength,
            ..g
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn mark_needs_paint_sets_the_flag_and_requests_a_frame() {
        let frames = Rc::new(Cell::new(0u32));
        let f = frames.clone();
        install_frame_request(move || f.set(f.get() + 1));

        let base = Base::new();
        assert!(base.needs_paint(), "a fresh widget needs its first paint");
        base.clear_needs_paint();
        assert!(!base.needs_paint(), "clearing drops the flag");

        base.mark_needs_paint();
        assert!(base.needs_paint(), "marking re-sets the repaint flag");
        assert_eq!(frames.get(), 1, "marking asks the host for exactly one frame");
    }
}
