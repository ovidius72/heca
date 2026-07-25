//! [`Overlay`] — the base overlay surface: scrim + panel chrome + viewport
//! positioning, shared by every overlay widget.
//!
//! **Blocking is a property of this layer, not a per-widget reimplementation**
//! (the T009 overlay rework): a *blocking* overlay paints a dimming scrim over
//! the whole viewport and swallows outside input (modal — [`Dialog`](super::Dialog));
//! a non-blocking one lets outside input fall through (light-dismiss popovers).
//! Positioning is a [property](OverlayPosition) of this layer: the default
//! [`Center`](OverlayPosition::Center) fills the viewport (`Pct(1.0)`²) and
//! **centers** its single panel child with real taffy layout, so every
//! descendant gets true bounds (hint picker + pointer hit-testing need them);
//! [`Anchored`](OverlayPosition::Anchored) instead hangs the panel off a trigger
//! rect (below/flip-above/clamp — [`place_anchored`]) for dropdown/popover
//! specializations. (The `Select`/`Tooltip`/`ContextMenu` widgets still own their
//! placement today; converting their panel *presentation* to compose an anchored
//! `Overlay` is the follow-up — the placement authority now lives here.)
//!
//! **Composition, not inheritance.** A specialized overlay widget ([`Dialog`](super::Dialog))
//! *composes* an `Overlay` as its subtree — the `Overlay` owns presentation
//! (scrim, shadow, panel fill, bracket reticle) and geometry
//! ([`overlay_occludes`](Component::overlay_occludes)); the specialization owns
//! its content and behaviour (focus trap, keyboard, dismissal policy) and
//! intercepts events *before* the `Overlay`'s own standalone handling runs.
//! Used directly (a host mounting an arbitrary — e.g. `realize`d — panel), the
//! `Overlay`'s own event handling provides the standard layer semantics:
//! nested-overlay-first routing, outside-click callback, blocking swallow.
//!
//! Paint goes through [`PaintCx::with_overlay`], so an overlay opened *inside*
//! this panel (a `Select` dropdown in a modal body) records a **deeper** scene
//! segment and composites above everything this overlay draws — see
//! [`Scene::overlay_segments`](crate::scene::Scene::overlay_segments).

use crate::builders::LayoutExt;
use crate::component::{paint_child, shift_subtree, Base, Component, Event, Handled, PaintCx};
use crate::focus::FocusManager;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, Shadow};
use crate::theme::FrameStyle;
use crate::style::{Align, Justify, Length};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// Gap kept between an overlay panel and the window edge, so a maxed-out panel's
/// border (and its glow) is never shaved off by the viewport boundary.
const VIEWPORT_MARGIN: f32 = 24.0;
/// Multiplier on the theme `shadow.blur` token — an overlay panel is large and
/// wants a wider, softer halo than the small-surface base token. Raised from 4.0
/// (2026-07-24): the panel read as barely lifted off the page.
const SHADOW_BLUR_MULT: f32 = 6.0;
/// Downward shadow offset lifting the panel off the scrim/page. Raised from 12.0
/// alongside the blur so the panel sits more clearly *above* what's behind it.
const SHADOW_DROP: f32 = 16.0;

/// Where an [`Overlay`] places its panel.
///
/// - [`Center`](OverlayPosition::Center) — the default: fill the viewport and
///   center the panel with real taffy layout (a modal [`Dialog`](super::Dialog)).
/// - [`Anchored`](OverlayPosition::Anchored) — dropdown/popover placement: the
///   panel is put **below** an anchor rect (a trigger), flipped **above** when
///   there is no room below, aligned to the anchor's left edge, and clamped into
///   the viewport so it never spills off-screen. This is the shared placement the
///   `Select` dropdown and `ContextMenu` each hand-roll today
///   ([`place_anchored`] is the one authority).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayPosition {
    /// Fill the viewport and center the panel (modal default).
    Center,
    /// Anchor the panel to a trigger rect (dropdown/popover): below, flipped
    /// above when no room, left-edge aligned, clamped into the viewport.
    Anchored {
        /// The trigger rect (viewport coordinates) the panel hangs off.
        anchor: Rectangle,
        /// Gap between the anchor edge and the panel.
        gap: f64,
    },
}

/// Default gap between an anchored panel and its trigger (logical px).
pub const DEFAULT_ANCHOR_GAP: f64 = 4.0;

/// Optional per-widget accents layered onto the shared overlay panel chrome by
/// [`paint_panel_chrome`] — a specialization's own identity (e.g. a
/// [`Select`](super::Select) dropdown's accent edge + neon halo). The drop shadow
/// and theme surface fill are not configurable: they are what makes every overlay
/// panel read as the same surface. Whether an **edge** is drawn at all is the
/// user's call, not the widget's — see
/// [`FrameStyle`](crate::theme::FrameStyle)/`overlay_frame`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelChrome {
    /// The widget's preferred edge (color + width) — used **only** when
    /// `overlay_frame` draws an edge (`Bordered`/`Bracketed`), and ignored under
    /// [`FrameStyle::None`](crate::theme::FrameStyle::None). `None` here falls back
    /// to the theme's neutral `border` color, so the base [`Overlay`] still gets an
    /// edge when the user asks for one.
    pub border: Option<Border>,
    /// Glow on the panel fill. `None` (the base [`Overlay`]) = no halo. **Not**
    /// governed by `overlay_frame`: the halo is the panel's neon identity, not a
    /// frame, so it survives `FrameStyle::None`.
    pub glow: Option<Glow>,
    /// How far above the page this surface sits — the depth its drop shadow
    /// expresses. See [`PanelElevation`].
    pub elevation: PanelElevation,
}

/// How high above the page an overlay surface sits, and therefore how much drop
/// shadow it casts.
///
/// The shadow's *shape* is shared — one definition, scaled — so surfaces at
/// different depths still read as the same material. A caller picks the semantic
/// depth; it never supplies a blur radius or an offset.
///
/// This exists because the panel shadow was tuned for surfaces that own the screen
/// (a dialog is hundreds of pixels across). Applied unscaled to a ~30px
/// [`Tooltip`](super::Tooltip) bubble, the same shadow is **larger than the surface
/// casting it** — user-verified 2026-07-25 as "too much". A transient hover bubble
/// is not at dialog depth, so it does not take the dialog's shadow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelElevation {
    /// A surface that owns the screen: [`Dialog`](super::Dialog), a
    /// [`Select`](super::Select) dropdown, [`ContextMenu`](super::ContextMenu),
    /// [`CommandPalette`](super::CommandPalette). Full shadow depth.
    #[default]
    Panel,
    /// A transient surface hovering just above its target — the
    /// [`Tooltip`](super::Tooltip) bubble. Same shadow shape at
    /// [`HOVER_SHADOW_SCALE`] of the depth: enough to lift it off the page, not
    /// enough to read as a panel.
    Hover,
}

/// Fraction of the [`Panel`](PanelElevation::Panel) shadow that a
/// [`Hover`](PanelElevation::Hover) surface casts. Blur and offset scale together,
/// so the shadow keeps its shape and only loses depth.
const HOVER_SHADOW_SCALE: f32 = 0.25;

#[heca_grid_ui_macros::props]
impl PanelElevation {
    /// Multiplier applied to both the shadow's blur and its drop offset.
    fn shadow_scale(self) -> f32 {
        match self {
            Self::Panel => 1.0,
            Self::Hover => HOVER_SHADOW_SCALE,
        }
    }
}

/// Paint the **shared overlay panel chrome** into `rect`: drop shadow (lifting the
/// panel off the page), the theme surface fill, the per-widget `chrome` accents, and
/// the panel's **edge** as the user configured it.
///
/// This is the single authority for what an overlay panel *looks like*, so the base
/// [`Overlay`] and the widgets that own their own panel (a `Select` dropdown, whose
/// option rows are placed children and therefore cannot be handed to an `Overlay`)
/// cannot drift apart. Call it inside a [`PaintCx::with_overlay`] block; it does not
/// open the overlay layer itself.
///
/// The edge follows [`Theme::overlay_frame`](crate::theme::FrameStyle) — the app's
/// `[appearance] overlay_border_style` — and it owns the **whole** edge, so the
/// three styles are genuinely distinct:
///
/// | `overlay_frame` | Edge | Corner reticle |
/// |---|---|---|
/// | `Bracketed` (default) | yes | yes |
/// | `Bordered` | yes | no |
/// | `None` | **no** | no |
///
/// A widget's own `chrome.border` is its preferred edge *color*; it is honoured when
/// the style draws an edge and **ignored** under `None` (otherwise `None` could not
/// remove a `Select`'s accent border, which is the bug this ownership rule fixes).
/// The fill and the glow are never suppressed.
///
/// The **drop shadow is not part of the frame policy** — it is depth, not an edge, so
/// `overlay_frame` never removes it. Its size comes from
/// [`chrome.elevation`](PanelChrome::elevation): full for a panel, a quarter of it for
/// a [`Hover`](PanelElevation::Hover) surface.
pub fn paint_panel_chrome(cx: &mut PaintCx, rect: Rectangle, chrome: PanelChrome) {
    let (surface, shadow, shadow_blur, radius, frame, border_color, border_width) = {
        let t = cx.theme();
        (
            t.colors.surface,
            t.shadow_color(),
            t.colors.shadow.blur,
            t.colors.border_radius,
            t.colors.overlay_frame,
            t.colors.border,
            t.colors.border_width,
        )
    };
    // One shadow shape for every overlay surface, scaled by how high it sits — so a
    // dialog and a tooltip read as the same material at different depths.
    let depth = chrome.elevation.shadow_scale();
    cx.drop_shadow(
        rect,
        radius,
        Shadow {
            color: shadow,
            radius: shadow_blur * SHADOW_BLUR_MULT * depth,
            dx: 0.0,
            dy: SHADOW_DROP * depth,
        },
    );
    // `overlay_frame` owns the panel's whole EDGE — so `None` really means no
    // edge, not "no brackets but keep the border". The widget's own accent border
    // is its preferred edge *color*, honoured only when the style draws an edge;
    // the fill and the glow are never suppressed (the glow is the panel's neon
    // identity, not a frame).
    let edge = match frame {
        // Bracketed / Bordered both draw an edge: the widget's accent border when
        // it has one, else the theme's neutral border.
        FrameStyle::Bracketed | FrameStyle::Bordered => chrome.border.or(Some(Border {
            color: border_color,
            width: border_width,
        })),
        FrameStyle::None => None,
    };
    cx.rect(rect, surface, edge, radius, chrome.glow);
    // The corner reticle is the extra that distinguishes Bracketed from Bordered.
    if frame == FrameStyle::Bracketed {
        cx.bracket_frame(rect);
    }
}

/// Which side of the anchor an [`Anchored`](OverlayPosition::Anchored) panel goes on.
///
/// [`Auto`](AnchorSide::Auto) is the usual choice — the placement picks below,
/// flipping above when there is no room. A caller that has **already** decided the
/// direction passes [`Below`](AnchorSide::Below) / [`Above`](AnchorSide::Above) so
/// the placement honours it instead of re-deciding: [`Select`](super::Select) does
/// this because its flip decision and its visible-row count are computed together
/// (the panel's height depends on the side), so the two must not disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum AnchorSide {
    /// Prefer below; flip above when there is no room below (and more above).
    #[default]
    Auto,
    /// Force below the anchor (still clamped into the viewport).
    Below,
    /// Force above the anchor (still clamped into the viewport).
    Above,
}

/// Place a `panel`-sized rect against an `anchor` rect for a dropdown/popover:
/// **below** the anchor by `gap`, **flipped above** when the panel would overflow
/// the viewport bottom and there is more room above, **left-edge aligned** to the
/// anchor, and finally **clamped** on both axes into the viewport so it never
/// spills off-screen.
///
/// This is the single authority for anchor-to-rect placement — the flip/clamp
/// logic that `select.rs` (`open_list`/`panel_top`) and `context_menu.rs`
/// (`layout`) each reimplement. An **infinite** viewport axis (before the first
/// paint has cached one) disables clamping/flipping on that axis: the panel is
/// simply placed below the anchor.
///
/// # Examples
/// ```
/// use heca_grid_ui::widgets::place_anchored;
/// use heca_core::layout::{Point, Rectangle, Size};
///
/// let anchor = Rectangle::new(Point::new(10.0, 100.0), Size::new(120.0, 30.0));
/// let panel = Size::new(120.0, 80.0);
/// let vp = Size::new(800.0, 600.0);
/// // Plenty of room below → placed just under the trigger.
/// let r = place_anchored(anchor, panel, vp, 4.0);
/// assert_eq!(r.loc.y, 100.0 + 30.0 + 4.0);
/// assert_eq!(r.loc.x, 10.0);
/// ```
pub fn place_anchored(anchor: Rectangle, panel: Size, viewport: Size, gap: f64) -> Rectangle {
    place_anchored_on(anchor, panel, viewport, gap, AnchorSide::Auto)
}

/// [`place_anchored`] with an explicit [`AnchorSide`] — the full form.
///
/// `side` controls only the **flip decision**; clamping into the viewport still
/// applies on both axes, so a forced side never puts the panel off-screen. Use
/// [`AnchorSide::Auto`] unless the caller has already chosen a direction (see
/// [`AnchorSide`]).
pub fn place_anchored_on(
    anchor: Rectangle,
    panel: Size,
    viewport: Size,
    gap: f64,
    side: AnchorSide,
) -> Rectangle {
    let below_y = anchor.loc.y + anchor.size.h + gap;
    let above_y = anchor.loc.y - gap - panel.h;

    let mut x = anchor.loc.x;
    let mut y = match side {
        AnchorSide::Below => below_y,
        AnchorSide::Above => above_y,
        AnchorSide::Auto => below_y,
    };

    if viewport.h.is_finite() {
        if side == AnchorSide::Auto {
            let fits_below = below_y + panel.h <= viewport.h;
            let fits_above = above_y >= 0.0;
            if !fits_below && fits_above {
                // No room below, room above → flip up.
                y = above_y;
            } else if !fits_below && !fits_above {
                // Neither side fits fully: take the side with more room, then clamp.
                let room_below = (viewport.h - below_y).max(0.0);
                let room_above = (anchor.loc.y - gap).max(0.0);
                y = if room_above > room_below { above_y } else { below_y };
            }
        }
        y = y.clamp(0.0, (viewport.h - panel.h).max(0.0));
    }
    if viewport.w.is_finite() {
        x = x.clamp(0.0, (viewport.w - panel.w).max(0.0));
    }
    Rectangle::new(Point::new(x, y), panel)
}

/// Place a `panel`-sized rect against a **cursor point** for a context menu /
/// point-anchored popover: preferred **down-right** of the anchor by `inset`,
/// flipped **up-left** when the panel would overflow the viewport edge, then
/// clamped on both axes. When `centered` is true the panel is instead **centered
/// on** the anchor (a keyboard/RPC-opened menu with no pointer target).
///
/// This is the single authority for point-anchored placement — the logic
/// [`ContextMenu`](super::ContextMenu)'s `layout` reimplemented inline. As with
/// `ContextMenu` today, an **infinite** viewport (before the first paint caches
/// one) collapses the placement to the origin; callers cache a real viewport at
/// paint before relying on the result.
///
/// # Examples
/// ```
/// use heca_grid_ui::widgets::place_at_point;
/// use heca_core::layout::{Point, Size};
///
/// // Room down-right → top-left is offset from the cursor by the inset.
/// let r = place_at_point(Point::new(100.0, 100.0), Size::new(160.0, 80.0),
///                        Size::new(800.0, 600.0), 2.0, false);
/// assert_eq!(r.loc, Point::new(102.0, 102.0));
/// ```
pub fn place_at_point(
    anchor: Point,
    panel: Size,
    viewport: Size,
    inset: f64,
    centered: bool,
) -> Rectangle {
    // Mirror ContextMenu's viewport handling exactly: an infinite viewport falls
    // back to the panel's own size as the clamp bound (collapsing to the origin).
    let (vw, vh) = if viewport.w.is_finite() {
        (viewport.w, viewport.h)
    } else {
        (panel.w, panel.h)
    };
    let (mut x, mut y) = if centered {
        (anchor.x - panel.w / 2.0, anchor.y - panel.h / 2.0)
    } else {
        // Prefer down-right of the anchor; flip to up-left when it would overflow.
        let mut x = anchor.x + inset;
        if x + panel.w > vw {
            x = (anchor.x - panel.w - inset).max(0.0);
        }
        let mut y = anchor.y + inset;
        if y + panel.h > vh {
            y = (anchor.y - panel.h - inset).max(0.0);
        }
        (x, y)
    };
    x = x.clamp(0.0, (vw - panel.w).max(0.0));
    y = y.clamp(0.0, (vh - panel.h).max(0.0));
    Rectangle::new(Point::new(x, y), panel)
}

/// Which side of the anchor a [`place_beside`] panel sits on.
///
/// Unlike [`AnchorSide`] (a dropdown's vertical below/above decision) this is the
/// full four-sided vocabulary, and the side is always **explicit** — there is no
/// `Auto`, because "beside" has no natural default direction the way a dropdown
/// has "below". The placement still flips to the opposite side when the chosen one
/// does not fit.
///
/// Re-exported as [`TooltipSide`](super::TooltipSide) — the same type under the
/// name that reads better at a [`Tooltip`](super::Tooltip) call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum BesideSide {
    /// Above the anchor (flips to [`Bottom`](BesideSide::Bottom) when there is no room).
    #[default]
    Top,
    /// Below the anchor (flips to [`Top`](BesideSide::Top) when there is no room).
    Bottom,
    /// Left of the anchor (flips to [`Right`](BesideSide::Right) when there is no room).
    Left,
    /// Right of the anchor (flips to [`Left`](BesideSide::Left) when there is no room).
    Right,
}

impl BesideSide {
    /// The side this one flips to when it does not fit.
    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Whether a `panel`-sized rect fits on this side of `anchor` within `viewport`.
    fn fits(self, anchor: Rectangle, panel: Size, viewport: Size, gap: f64) -> bool {
        match self {
            Self::Top => anchor.loc.y - panel.h - gap >= 0.0,
            Self::Bottom => anchor.loc.y + anchor.size.h + panel.h + gap <= viewport.h,
            Self::Left => anchor.loc.x - panel.w - gap >= 0.0,
            Self::Right => anchor.loc.x + anchor.size.w + panel.w + gap <= viewport.w,
        }
    }
}

/// Place a `panel`-sized rect **beside** an `anchor` rect on any of the four
/// sides: offset from the chosen edge by `gap`, **centered** on the cross axis,
/// **flipped** to the opposite side when the chosen one has no room (and the
/// opposite does), and finally **clamped** on the cross axis so it never spills
/// off-screen.
///
/// This is the single authority for four-sided, centered placement — the rule
/// [`Tooltip`](super::Tooltip) used to hand-roll. It is a different rule in kind
/// from the other two authorities, which is why it is its own function:
///
/// | Authority | Anchor | Alignment | Flips |
/// |---|---|---|---|
/// | [`place_anchored_on`] | a rect | leading-edge aligned | below ↔ above |
/// | [`place_at_point`] | a point | corner-offset from the point | down-right ↔ up-left |
/// | `place_beside` | a rect | **centered** on the cross axis | all four sides |
///
/// # Not clamped on the main axis — deliberately
///
/// Only the **cross** axis (the one the side does not pin) is clamped. The flip
/// is the main axis's answer to "no room"; clamping it as well would slide the
/// panel *over* its anchor, which for a hover bubble means covering the very thing
/// it describes. When neither side fits, the chosen side is kept and the panel may
/// overflow — a smaller panel is the fix, not a moved one.
///
/// # The purity note
///
/// Unlike a dropdown's panel rect, this result **may** depend on the viewport (the
/// flip and the clamp both read it). That is safe **only because a `place_beside`
/// panel places no child components** — its content is drawn, not laid out into it.
/// If you place children with this rect, you inherit the detach bug documented on
/// [`place_anchored_on`]'s callers: layout and paint run at different moments, so a
/// viewport-dependent rect makes the two disagree. Draw-only, or don't use it.
///
/// An **infinite** viewport axis (before the first paint has cached one) disables
/// flipping and clamping against that axis.
///
/// # Examples
/// ```
/// use heca_grid_ui::widgets::{place_beside, BesideSide};
/// use heca_core::layout::{Point, Rectangle, Size};
///
/// let anchor = Rectangle::new(Point::new(100.0, 100.0), Size::new(40.0, 20.0));
/// let panel = Size::new(80.0, 24.0);
/// let vp = Size::new(800.0, 600.0);
///
/// // Room above → sits on top, centered over the anchor.
/// let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Top);
/// assert_eq!(r.loc.y, 100.0 - 24.0 - 6.0);
/// assert_eq!(r.loc.x, 100.0 + (40.0 - 80.0) / 2.0);
/// ```
pub fn place_beside(
    anchor: Rectangle,
    panel: Size,
    viewport: Size,
    gap: f64,
    side: BesideSide,
) -> Rectangle {
    // Keep the preferred side unless it does not fit and the opposite one does.
    let opposite = side.opposite();
    let flip =
        !side.fits(anchor, panel, viewport, gap) && opposite.fits(anchor, panel, viewport, gap);
    let side = if flip { opposite } else { side };

    let (mut x, mut y) = match side {
        BesideSide::Top => (
            anchor.loc.x + (anchor.size.w - panel.w) / 2.0,
            anchor.loc.y - panel.h - gap,
        ),
        BesideSide::Bottom => (
            anchor.loc.x + (anchor.size.w - panel.w) / 2.0,
            anchor.loc.y + anchor.size.h + gap,
        ),
        BesideSide::Left => (
            anchor.loc.x - panel.w - gap,
            anchor.loc.y + (anchor.size.h - panel.h) / 2.0,
        ),
        BesideSide::Right => (
            anchor.loc.x + anchor.size.w + gap,
            anchor.loc.y + (anchor.size.h - panel.h) / 2.0,
        ),
    };
    // Clamp the cross axis only — see "Not clamped on the main axis" above.
    match side {
        BesideSide::Top | BesideSide::Bottom if viewport.w.is_finite() => {
            x = x.clamp(0.0, (viewport.w - panel.w).max(0.0));
        }
        BesideSide::Left | BesideSide::Right if viewport.h.is_finite() => {
            y = y.clamp(0.0, (viewport.h - panel.h).max(0.0));
        }
        _ => {}
    }
    Rectangle::new(Point::new(x, y), panel)
}

/// The base overlay surface: a viewport-filling, centering layer that paints a
/// panel (its single child) with the shared overlay chrome — optional scrim,
/// drop shadow, theme surface fill, and the bracket reticle.
///
/// Build with [`Overlay::new`], hand it the panel via [`panel`](Overlay::panel)
/// (or [`panel_boxed`](Overlay::panel_boxed) for a mapper-produced
/// `Box<dyn Component>`), and drive visibility through
/// [`open_signal`](Overlay::open_signal). [`blocking`](Overlay::blocking)
/// selects the layer policy: blocking (default) = scrim + swallow outside
/// input; non-blocking = outside input falls through (light dismiss).
pub struct Overlay {
    base: Base,
    open: Signal<bool>,
    /// Blocking layer policy: scrim + swallow outside input (modal). `false` ⇒
    /// no scrim; outside input falls through after the outside-click callback.
    blocking: bool,
    /// Fired when a press lands outside the panel — the standalone dismissal
    /// hook (a composing widget usually implements its own policy instead).
    on_outside_click: Option<Box<dyn Fn()>>,
    /// How the panel is placed: centered (default) or anchored to a trigger rect.
    position: OverlayPosition,
    /// Explicit panel size, applied to the panel child's style. `None` (default)
    /// leaves the panel to size itself from its content.
    panel_size: Option<(Length, Length)>,
    /// Last-seen viewport, cached during paint (scrim rect + anchored placement).
    viewport: Cell<Size>,
}

impl Overlay {
    /// A new (closed) blocking overlay with an empty panel slot.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Fill the viewport and center the panel on both axes — real taffy
        // centering, so every descendant gets true bounds.
        base.style.layout.width = Length::Pct(1.0);
        base.style.layout.height = Length::Pct(1.0);
        base.style.layout.justify = Justify::Center;
        base.style.layout.align = Align::Center;
        // Breathing room between the panel and the window edge. It doubles as the
        // inset for the viewport cap in `apply_panel_size`: the panel's `Pct(1.0)`
        // max resolves against this padded content box, so even a huge panel keeps
        // this margin and its border/glow is never shaved by the window edge.
        base.style.layout.padding = VIEWPORT_MARGIN;
        Self {
            base,
            open: signal(false),
            blocking: true,
            on_outside_click: None,
            position: OverlayPosition::Center,
            panel_size: None,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
        }
    }

    /// Set the **panel** — the single child this layer centers and decorates.
    /// The caller owns the panel's internal layout (padding, gaps, children);
    /// the overlay owns the chrome around it. Replaces any previous panel.
    pub fn panel(mut self, panel: impl Component + 'static) -> Self {
        self.base.children.clear();
        self.base.children.push(Box::new(panel));
        self.apply_panel_size();
        self
    }

    /// Like [`panel`](Overlay::panel) but takes an already-boxed component —
    /// for a panel produced by a mapper returning `Box<dyn Component>` (e.g.
    /// `heca`'s `realize(ViewNode)`).
    pub fn panel_boxed(mut self, panel: Box<dyn Component>) -> Self {
        self.base.children.clear();
        self.base.children.push(panel);
        self.apply_panel_size();
        self
    }

    /// Give the panel an explicit size instead of letting it hug its content.
    ///
    /// The main reason to want this: **a [`ScrollRegion`](super::ScrollRegion)
    /// only scrolls when its parent bounds it.** A panel that sizes to its content
    /// simply grows with a long body, so nothing ever overflows and no scrollbar
    /// appears. Give the panel a height and the body can scroll inside it.
    ///
    /// `Length::Auto` on an axis means "as before" (hug the content). A
    /// [`Pct`](Length::Pct) resolves against the **viewport**, because the
    /// `Overlay` itself fills it — so `Pct(0.8)` is 80% of the viewport, not 80%
    /// of anything the caller laid out.
    ///
    /// Call order does not matter: this is stored on the overlay and re-applied
    /// whenever the panel is (re)set by [`panel`](Overlay::panel) /
    /// [`panel_boxed`](Overlay::panel_boxed).
    ///
    /// ```ignore
    /// // A modal that is 60% of the viewport wide and 70% tall, whose body scrolls.
    /// Overlay::new()
    ///     .panel_size(Length::Pct(0.6), Length::Pct(0.7))
    ///     .panel(Flex::column().child(ScrollRegion::new().child(long_content)))
    /// ```
    pub fn panel_size(mut self, width: Length, height: Length) -> Self {
        self.panel_size = Some((width, height));
        self.apply_panel_size();
        self
    }

    /// Push the configured panel size onto the panel child's style, and **always**
    /// cap the panel at the viewport.
    ///
    /// The cap is unconditional, not part of `panel_size`: the `Overlay` fills the
    /// viewport, so `Pct(1.0)` here *is* the window. Without it a fixed `Px` panel
    /// (or a big content-sized one) draws larger than the window and gets cut off
    /// by the screen edge on both sides — a dialog must never be bigger than the
    /// thing it is centered in.
    fn apply_panel_size(&mut self) {
        let Some(panel) = self.base.children.first_mut() else {
            return;
        };
        let style = &mut panel.base_mut().style.layout;
        style.max_width = Some(Length::Pct(1.0));
        style.max_height = Some(Length::Pct(1.0));
        if let Some((w, h)) = self.panel_size {
            style.width = w;
            style.height = h;
        }
    }

    /// Layer policy: `true` (default) = modal — dimming scrim + outside input
    /// swallowed; `false` = light layer — no scrim, outside input falls through.
    #[heca_grid_ui_macros::prop]
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    /// Set the panel placement (default [`OverlayPosition::Center`]).
    pub fn position(mut self, position: OverlayPosition) -> Self {
        self.position = position;
        self
    }

    /// Anchor the panel to a trigger `rect` (dropdown/popover placement): below,
    /// flipped above when no room, left-edge aligned, clamped into the viewport —
    /// see [`place_anchored`]. Uses [`DEFAULT_ANCHOR_GAP`]; pair with a
    /// non-[`blocking`](Overlay::blocking) layer for a light-dismiss popover.
    pub fn anchored(mut self, rect: Rectangle) -> Self {
        self.position = OverlayPosition::Anchored {
            anchor: rect,
            gap: DEFAULT_ANCHOR_GAP,
        };
        self
    }

    /// Update the anchor rect of an [`Anchored`](OverlayPosition::Anchored)
    /// overlay in place (a host re-anchoring to a moved trigger). No-op in
    /// [`Center`](OverlayPosition::Center) mode.
    pub fn set_anchor(&mut self, rect: Rectangle) {
        if let OverlayPosition::Anchored { anchor, .. } = &mut self.position
            && *anchor != rect
        {
            *anchor = rect;
            self.place_panel();
        }
    }

    /// Place the panel for an anchored overlay by baking the placement offset into
    /// the panel child's bounds (the same subtree-shift trick `Select`/`ScrollRegion`
    /// use). Idempotent: the target is absolute, so re-running never compounds.
    /// A no-op in [`Center`](OverlayPosition::Center) mode (taffy centers there).
    fn place_panel(&mut self) {
        let OverlayPosition::Anchored { anchor, gap } = self.position else {
            return;
        };
        let Some(child) = self.base.children.first_mut() else {
            return;
        };
        let current = child.base().bounds;
        if current.size == Size::new(0.0, 0.0) {
            return; // Not laid out yet — nothing to place.
        }
        let target = place_anchored(anchor, current.size, self.viewport.get(), gap);
        let dx = target.loc.x - current.loc.x;
        let dy = target.loc.y - current.loc.y;
        if dx != 0.0 || dy != 0.0 {
            shift_subtree(child.as_mut(), dx, dy);
            self.base.mark_needs_paint();
        }
    }

    /// Set the initial open state.
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// The open-state signal — the host (or composing widget) binds this.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// Called when a press lands **outside** the panel (standalone use; a
    /// composing widget usually intercepts the press and applies its own
    /// dismissal policy instead).
    pub fn on_outside_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_outside_click = Some(Box::new(f));
        self
    }

    /// The panel's laid-out bounds (valid after layout; zero before).
    pub fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .map(|c| c.base().bounds)
            .unwrap_or_else(|| Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)))
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Overlay {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so a host's overlay scan can route input here
    /// when the `Overlay` is mounted directly (a composing widget like `Dialog`
    /// is found first in pre-order and intercepts instead).
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// Layout just reset the panel to its taffy-computed position; in
    /// [`Anchored`](OverlayPosition::Anchored) mode, re-place it against the
    /// trigger rect (idempotent — see [`place_panel`](Overlay::place_panel)).
    /// [`Center`](OverlayPosition::Center) mode keeps taffy's centering untouched.
    fn on_layout(&mut self) {
        self.place_panel();
    }

    /// A **blocking** overlay occludes the whole viewport (its scrim owns every
    /// point); a non-blocking one occludes only the panel itself.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.is_open() && (self.blocking || self.panel_bounds().contains(pos))
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        let (background, scrim_a) = {
            let t = cx.theme();
            (t.colors.background, t.colors.interaction.scrim)
        };
        let panel = self.panel_bounds();

        cx.with_overlay(|cx| {
            // Scrim over the whole viewport — the visual half of the blocking
            // layer policy (the event half swallows outside input below).
            if self.blocking {
                let vp = self.viewport.get();
                let scrim = if vp.w.is_finite() {
                    Rectangle::new(Point::new(0.0, 0.0), vp)
                } else {
                    panel
                };
                cx.rect(scrim, background.with_alpha(scrim_a), None, 0.0, None);
            }

            // Lift the panel, fill it, stamp the shared bracket reticle (same
            // visual language as Pane / DockFrame) — the base layer takes the
            // chrome plain; specializations pass their own accents.
            paint_panel_chrome(cx, panel, PanelChrome::default());

            // The panel's real children on top of the fill. A nested overlay
            // painted in here records a DEEPER scene segment → composites above
            // everything this layer draws (Scene::overlay_segments).
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
    }

    /// **Standalone** layer semantics (a composing widget intercepts events
    /// before this runs and applies its own policy — see the module docs):
    /// nested-overlay-first routing, outside-click callback, blocking swallow.
    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        // Nested overlay first: an open overlay INSIDE the panel (a Select
        // dropdown) captures input over the whole layer before the panel's
        // ordinary children see it — mirroring the host's focus.rs overlay
        // scan. `offer_to_overlay` carries no manager state, so a fresh
        // FocusManager is just the scan.
        let panel_root = match self.base.children.first_mut() {
            Some(p) => p.as_mut(),
            None => return if self.blocking { Handled::Yes } else { Handled::No },
        };
        if FocusManager::new().offer_to_overlay(panel_root, ev) == Handled::Yes {
            return Handled::Yes;
        }
        let panel = self.panel_bounds();
        match ev {
            Event::PointerPressed { pos } => {
                if panel.contains(*pos) {
                    let panel_root = self.base.children[0].as_mut();
                    let _ = panel_root.event(ev);
                } else if let Some(f) = &self.on_outside_click {
                    f();
                }
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            Event::PointerMoved { .. } => {
                let panel_root = self.base.children[0].as_mut();
                let _ = panel_root.event(ev);
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            // A press has to be matched by its RELEASE inside the panel, or a
            // widget that grabbed the pointer never lets go — a `ScrollRegion`
            // thumb drag stayed stuck to the cursor because the release never
            // reached it (the overlay swallowed it as an unhandled event).
            Event::PointerReleased { .. } => {
                let panel_root = self.base.children[0].as_mut();
                let _ = panel_root.event(ev);
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            // The panel gets the wheel FIRST — a scrollable inside a modal (a long
            // dialog body) must scroll. The overlay scan above only offers to an
            // `overlay_active` descendant (a nested dropdown), and a `ScrollRegion`
            // is not one, so without this it never saw the wheel at all. Only if
            // the panel doesn't take it does the blocking layer swallow it, which
            // is what keeps the page behind a modal from scrolling.
            Event::Scroll { .. } => {
                let panel_root = self.base.children[0].as_mut();
                if panel_root.event(ev) == Handled::Yes {
                    return Handled::Yes;
                }
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Overlay {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::Parent;
    use crate::widgets::{Flex, Label};

    fn open_overlay() -> Overlay {
        Overlay::new()
            .panel(Flex::column().child(Label::new("hi")))
            .open(true)
    }

    #[test]
    fn closed_overlay_is_inert() {
        let mut o = Overlay::new().panel(Flex::column());
        assert!(!o.overlay_active());
        assert!(!o.focusable());
        assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(1.0, 1.0) }),
            Handled::No
        );
    }

    #[test]
    fn blocking_overlay_occludes_everywhere_and_swallows_outside_input() {
        let mut o = open_overlay();
        assert!(o.overlay_occludes(Point::new(-500.0, -500.0)), "scrim owns every point");
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(-500.0, -500.0) }),
            Handled::Yes,
            "modal swallows the outside press"
        );
        assert_eq!(o.event(&Event::Scroll { delta_x: 0.0, delta_y: 1.0 }), Handled::Yes);
    }

    #[test]
    fn non_blocking_overlay_occludes_only_its_panel_and_lets_outside_fall_through() {
        use std::cell::Cell;
        use std::rc::Rc;
        let dismissed = Rc::new(Cell::new(false));
        let d = dismissed.clone();
        let mut o = Overlay::new()
            .blocking(false)
            .panel(Flex::column().child(Label::new("hi")))
            .open(true)
            .on_outside_click(move || d.set(true));
        // Give the panel real bounds (as layout would).
        o.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(100.0, 100.0), Size::new(50.0, 20.0));
        assert!(o.overlay_occludes(Point::new(110.0, 110.0)), "panel point occludes");
        assert!(!o.overlay_occludes(Point::new(0.0, 0.0)), "outside point does not");
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(0.0, 0.0) }),
            Handled::No,
            "light layer lets the outside press fall through"
        );
        assert!(dismissed.get(), "outside press fired the dismissal hook");
    }

    // ── Panel sizing (.panel_size) ──

    /// The size lands on the panel child whichever order the builders are called
    /// in — `panel()` clears and re-pushes the child, so the overlay has to keep
    /// the size and re-apply it.
    #[test]
    fn panel_size_applies_regardless_of_builder_order() {
        let want_w = Length::Pct(0.6);
        let want_h = Length::Px(420.0);

        // size first, then panel
        let a = Overlay::new()
            .panel_size(want_w, want_h)
            .panel(Flex::column().child(Label::new("body")));
        let style = &a.base.children[0].base().style.layout;
        assert_eq!(style.width, want_w);
        assert_eq!(style.height, want_h);

        // panel first, then size
        let b = Overlay::new()
            .panel(Flex::column().child(Label::new("body")))
            .panel_size(want_w, want_h);
        let style = &b.base.children[0].base().style.layout;
        assert_eq!(style.width, want_w);
        assert_eq!(style.height, want_h);
    }

    /// **Regression guard.** A panel must never be larger than the viewport it is
    /// centered in: an oversized one gets cut off by the window edge on *both*
    /// sides (a dialog bigger than the window — user-reported). The cap is
    /// unconditional, so it also protects a content-sized panel, not just a
    /// `panel_size`d one.
    #[test]
    fn panel_never_exceeds_the_viewport() {
        use crate::widgets::ScrollRegion;
        // Ask for a panel far bigger than the viewport we lay out in.
        let mut o = Overlay::new()
            .panel_size(Length::Px(4000.0), Length::Px(3000.0))
            .panel(ScrollRegion::new().child(Label::new("tall")))
            .open(true);
        crate::LayoutEngine::new().compute(&mut o, Size::new(800.0, 600.0));
        let panel = o.panel_bounds();
        assert!(panel.size.w <= 800.0, "panel width capped, got {}", panel.size.w);
        assert!(panel.size.h <= 600.0, "panel height capped, got {}", panel.size.h);
    }

    /// Unset (the default) leaves the panel hugging its own content — the sizing
    /// API must not silently impose a size on every existing overlay.
    #[test]
    fn panel_size_is_opt_in() {
        let o = Overlay::new().panel(Flex::column().child(Label::new("body")));
        let style = &o.base.children[0].base().style.layout;
        assert_eq!(style.width, Length::Auto, "untouched by default");
        assert_eq!(style.height, Length::Auto);
    }

    // ── Shared panel chrome + the `overlay_frame` edge policy ──

    /// Paint the shared chrome with a given `overlay_frame` and report how many
    /// **bordered** draw commands it emitted. Counting bordered commands (rather
    /// than asserting exact widths) keeps the test about the edge *policy* and
    /// robust to how the reticle happens to be drawn.
    fn chrome_edge_count(frame: FrameStyle, chrome: PanelChrome) -> usize {
        use crate::scene::DrawCommand;
        use crate::Scene;
        let mut theme = crate::theme::Theme::default();
        theme.colors.overlay_frame = frame;
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 100.0));
            paint_panel_chrome(&mut cx, rect, chrome);
        }
        scene
            .iter()
            .filter(|cmd| matches!(cmd, DrawCommand::Rect(r) if r.border.is_some()))
            .count()
    }

    /// **Regression guard.** `overlay_frame` owns the panel's whole edge, so `None`
    /// must remove the border too — not merely the corner reticle. The first version
    /// drew the widget's `chrome.border` unconditionally, so selecting "none" left a
    /// `Select`'s accent border on screen (user-reported).
    #[test]
    fn overlay_frame_none_removes_the_edge_entirely() {
        let accent = PanelChrome {
            border: Some(Border {
                color: crate::color::Color::rgb(1, 2, 3),
                width: 2.0,
            }),
            glow: None,
            elevation: PanelElevation::Panel,
        };

        let none = chrome_edge_count(FrameStyle::None, accent);
        assert_eq!(
            none, 0,
            "None must draw NO edge, even when the widget supplied one"
        );

        let bordered = chrome_edge_count(FrameStyle::Bordered, accent);
        assert_eq!(bordered, 1, "Bordered draws exactly the panel edge");

        // Bracketed = the same edge plus the corner reticle, so strictly more.
        let bracketed = chrome_edge_count(FrameStyle::Bracketed, accent);
        assert!(
            bracketed > bordered,
            "Bracketed adds the reticle on top of the edge ({bracketed} vs {bordered})"
        );
    }

    /// A widget that supplies no border of its own still gets an edge when the user
    /// asked for one (the theme's neutral border colour) — and still none under
    /// `FrameStyle::None`.
    #[test]
    fn overlay_frame_falls_back_to_the_theme_border_when_the_widget_has_none() {
        let plain = PanelChrome::default();
        assert_eq!(
            chrome_edge_count(FrameStyle::Bordered, plain),
            1,
            "theme border fills in for the base Overlay"
        );
        assert_eq!(
            chrome_edge_count(FrameStyle::None, plain),
            0,
            "…but None still means no edge"
        );
    }

    // ── Anchor-to-rect placement (place_anchored) ──

    /// A trigger with plenty of room below → panel sits just under it, left-aligned.
    #[test]
    fn anchored_places_below_with_room() {
        let anchor = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        assert_eq!(r.loc.x, 40.0, "left-edge aligned to the anchor");
        assert_eq!(r.loc.y, 100.0 + 30.0 + 4.0, "gap below the anchor bottom");
    }

    /// No room below but room above → the panel flips up above the trigger.
    #[test]
    fn anchored_flips_above_when_no_room_below() {
        // Anchor near the viewport bottom: 80px panel won't fit in the 40px below.
        let anchor = Rectangle::new(Point::new(40.0, 560.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        assert_eq!(r.loc.y, 560.0 - 4.0 - 80.0, "flipped to sit above the anchor");
    }

    /// Neither side fully fits → take the side with more room, then clamp on-screen.
    #[test]
    fn anchored_picks_more_room_when_neither_side_fits() {
        // Tall panel (500) in a short viewport (600); anchor low → more room above.
        let anchor = Rectangle::new(Point::new(0.0, 450.0), Size::new(100.0, 30.0));
        let panel = Size::new(100.0, 500.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        // Room above (450-4=446) > room below (600-484=116) → flip up, then clamp ≥ 0.
        assert_eq!(r.loc.y, 0.0, "clamped to the top after choosing the roomier side");
    }

    /// The panel is clamped so it never spills past the right/bottom viewport edge.
    #[test]
    fn anchored_clamps_into_viewport() {
        let anchor = Rectangle::new(Point::new(760.0, 20.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        assert_eq!(r.loc.x, 800.0 - 120.0, "right edge clamped into the viewport");
        assert!(r.loc.x >= 0.0);
    }

    /// A negative/off-left anchor is clamped back to x = 0.
    #[test]
    fn anchored_clamps_left_edge() {
        let anchor = Rectangle::new(Point::new(-30.0, 20.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        assert_eq!(r.loc.x, 0.0, "left edge clamped to the viewport origin");
    }

    /// An infinite viewport (before the first paint caches one) disables clamping
    /// and flipping — the panel is simply placed below the anchor.
    #[test]
    fn anchored_infinite_viewport_places_below_without_clamp() {
        let anchor = Rectangle::new(Point::new(900.0, 50.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let inf = Size::new(f64::INFINITY, f64::INFINITY);
        let r = place_anchored(anchor, panel, inf, 4.0);
        assert_eq!(r.loc.x, 900.0, "no clamp with an infinite viewport");
        assert_eq!(r.loc.y, 50.0 + 30.0 + 4.0, "placed below the anchor");
    }

    /// A forced side is honoured even when the automatic rule would flip: the
    /// caller (a `Select` that sized its list for that side) owns the decision.
    #[test]
    fn anchored_forced_side_is_honoured_over_the_auto_flip() {
        // Anchor near the bottom: Auto would flip up, but Below is forced.
        let anchor = Rectangle::new(Point::new(40.0, 560.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let vp = Size::new(800.0, 600.0);
        let auto = place_anchored_on(anchor, panel, vp, 4.0, AnchorSide::Auto);
        assert_eq!(auto.loc.y, 560.0 - 4.0 - 80.0, "auto flips up");

        let forced = place_anchored_on(anchor, panel, vp, 4.0, AnchorSide::Below);
        // Below would start at 594 and overflow, so it clamps to the bottom edge —
        // but it never flips to the other side.
        assert_eq!(forced.loc.y, 600.0 - 80.0, "forced Below clamps, does not flip");

        // And a forced Above near the TOP clamps instead of flipping down.
        let top_anchor = Rectangle::new(Point::new(40.0, 10.0), Size::new(120.0, 30.0));
        let up = place_anchored_on(top_anchor, panel, vp, 4.0, AnchorSide::Above);
        assert_eq!(up.loc.y, 0.0, "forced Above clamps to the top");
    }

    // ── Point-anchored placement (place_at_point) ──

    /// Room down-right of the cursor → top-left offset from the anchor by the inset.
    #[test]
    fn at_point_places_down_right_with_room() {
        let r = place_at_point(
            Point::new(100.0, 100.0),
            Size::new(160.0, 80.0),
            Size::new(800.0, 600.0),
            2.0,
            false,
        );
        assert_eq!(r.loc, Point::new(102.0, 102.0));
    }

    /// Cursor near the right/bottom edge → the panel flips up-left of the anchor.
    #[test]
    fn at_point_flips_up_left_near_edges() {
        // Anchor at (780, 580); a 160x80 panel can't go right/down in an 800x600 vp.
        let r = place_at_point(
            Point::new(780.0, 580.0),
            Size::new(160.0, 80.0),
            Size::new(800.0, 600.0),
            2.0,
            false,
        );
        // Flipped: x = 780-160-2 = 618, y = 580-80-2 = 498, both within the viewport.
        assert_eq!(r.loc, Point::new(618.0, 498.0));
    }

    /// Centered mode puts the panel *center* on the anchor (keyboard/RPC-opened menu).
    #[test]
    fn at_point_centered_puts_center_on_anchor() {
        let anchor = Point::new(1000.0, 1000.0);
        let panel = Size::new(160.0, 80.0);
        let r = place_at_point(anchor, panel, Size::new(2000.0, 2000.0), 2.0, true);
        assert!((r.loc.x + panel.w / 2.0 - anchor.x).abs() < 1e-9);
        assert!((r.loc.y + panel.h / 2.0 - anchor.y).abs() < 1e-9);
    }

    /// Paint the shared chrome at `elevation` and report the drop shadow it emitted.
    fn chrome_shadow(elevation: PanelElevation) -> Shadow {
        use crate::scene::DrawCommand;
        use crate::Scene;
        let theme = crate::theme::Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 100.0));
            paint_panel_chrome(
                &mut cx,
                rect,
                PanelChrome {
                    elevation,
                    ..PanelChrome::default()
                },
            );
        }
        // A drop shadow is recorded as a transparent `RectCmd` carrying `shadow`.
        scene
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Rect(r) => r.shadow,
                _ => None,
            })
            .expect("panel chrome always casts a shadow")
    }

    /// **User-verified regression guard (2026-07-25).** The panel shadow is tuned
    /// for surfaces hundreds of px across; unscaled on a ~30px tooltip bubble it is
    /// larger than the bubble itself ("the shadow is too much"). A `Hover` surface
    /// casts a fraction of it.
    #[test]
    fn hover_elevation_casts_a_shallower_shadow_than_a_panel() {
        let panel = chrome_shadow(PanelElevation::Panel);
        let hover = chrome_shadow(PanelElevation::Hover);
        assert!(
            hover.radius < panel.radius,
            "hover blur {} should be under the panel's {}",
            hover.radius,
            panel.radius
        );
    }

    /// Blur and drop scale by the *same* factor, so the shadow keeps its shape and
    /// only loses depth — a hover bubble must not get a differently-shaped shadow.
    #[test]
    fn hover_elevation_scales_blur_and_drop_together() {
        let panel = chrome_shadow(PanelElevation::Panel);
        let hover = chrome_shadow(PanelElevation::Hover);
        assert!(
            ((hover.radius / panel.radius) - (hover.dy / panel.dy)).abs() < 1e-6,
            "blur ratio {} != drop ratio {}",
            hover.radius / panel.radius,
            hover.dy / panel.dy
        );
    }

    /// Elevation is depth, not an edge: it must not disturb the frame policy.
    #[test]
    fn hover_elevation_leaves_the_edge_policy_untouched() {
        let hover = PanelChrome {
            elevation: PanelElevation::Hover,
            ..PanelChrome::default()
        };
        assert_eq!(
            chrome_edge_count(FrameStyle::None, hover),
            0,
            "None still means no edge at any elevation"
        );
    }

    // ── Four-sided, centered placement (place_beside) ──

    /// The anchor used by the `beside_*` tests: a 40×20 target in the middle of an
    /// 800×600 viewport, with a 80×24 panel — wider than the anchor, so a centered
    /// result has a *negative* offset and can't be confused with edge alignment.
    fn beside_case() -> (Rectangle, Size, Size) {
        (
            Rectangle::new(Point::new(400.0, 300.0), Size::new(40.0, 20.0)),
            Size::new(80.0, 24.0),
            Size::new(800.0, 600.0),
        )
    }

    /// The defining difference from `place_anchored_on`: a vertical side centers
    /// the panel **horizontally** on the anchor rather than aligning its left edge.
    /// The panel is deliberately wider than the anchor, so a centered result has a
    /// negative offset and cannot be confused with edge alignment.
    #[test]
    fn beside_top_centers_the_panel_horizontally_on_the_anchor() {
        let (anchor, panel, vp) = beside_case();
        let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Top);
        let anchor_mid = anchor.loc.x + anchor.size.w / 2.0;
        assert!(
            (r.loc.x + panel.w / 2.0 - anchor_mid).abs() < 1e-9,
            "panel centre {} should sit on the anchor centre {anchor_mid}",
            r.loc.x + panel.w / 2.0
        );
    }

    /// A horizontal side centers the panel **vertically** — the same rule on the
    /// other axis.
    #[test]
    fn beside_left_centers_the_panel_vertically_on_the_anchor() {
        let (anchor, panel, vp) = beside_case();
        let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Left);
        let anchor_mid = anchor.loc.y + anchor.size.h / 2.0;
        assert!(
            (r.loc.y + panel.h / 2.0 - anchor_mid).abs() < 1e-9,
            "panel centre {} should sit on the anchor centre {anchor_mid}",
            r.loc.y + panel.h / 2.0
        );
    }

    /// Gap shared by the `beside_flips_*` cases.
    const BESIDE_GAP: f64 = 6.0;

    /// Place the shared 80×24 panel beside a 40×20 anchor pinned at `(x, y)`,
    /// inside the shared 800×600 viewport. One line of arrangement so each flip
    /// case below is a single assertion.
    fn beside_at(x: f64, y: f64, side: BesideSide) -> Rectangle {
        let (_, panel, vp) = beside_case();
        let anchor = Rectangle::new(Point::new(x, y), Size::new(40.0, 20.0));
        place_beside(anchor, panel, vp, BESIDE_GAP, side)
    }

    /// An anchor flush against the viewport top leaves no room above, so a `Top`
    /// preference lands below it instead.
    #[test]
    fn beside_top_flips_below_when_the_anchor_hugs_the_viewport_top() {
        let r = beside_at(400.0, 0.0, BesideSide::Top);
        assert_eq!(r.loc.y, 20.0 + BESIDE_GAP);
    }

    /// …and the mirror case: no room below flips a `Bottom` preference above.
    #[test]
    fn beside_bottom_flips_above_when_the_anchor_hugs_the_viewport_bottom() {
        let r = beside_at(400.0, 580.0, BesideSide::Bottom);
        assert_eq!(r.loc.y, 580.0 - 24.0 - BESIDE_GAP);
    }

    /// The horizontal axis flips by the same rule: no room left → placed right.
    #[test]
    fn beside_left_flips_right_when_the_anchor_hugs_the_viewport_left() {
        let r = beside_at(0.0, 300.0, BesideSide::Left);
        assert_eq!(r.loc.x, 40.0 + BESIDE_GAP);
    }

    /// …and no room right → placed left.
    #[test]
    fn beside_right_flips_left_when_the_anchor_hugs_the_viewport_right() {
        let r = beside_at(760.0, 300.0, BesideSide::Right);
        assert_eq!(r.loc.x, 760.0 - 80.0 - BESIDE_GAP);
    }

    /// Only the **cross** axis is clamped. An anchor near the left edge slides the
    /// panel right so it stays on screen, without touching the side it sits on.
    #[test]
    fn beside_clamps_only_the_cross_axis() {
        let (_, panel, vp) = beside_case();
        // Anchor hugging the left edge: centring would put x at 5 - 40 = -35.
        let anchor = Rectangle::new(Point::new(5.0, 300.0), Size::new(40.0, 20.0));
        let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Top);
        assert_eq!(r.loc.x, 0.0, "cross axis clamped into the viewport");
        assert_eq!(r.loc.y, 300.0 - panel.h - 6.0, "main axis untouched by the clamp");
    }

    /// **Documents the deliberate rule.** When neither side fits, the preferred side
    /// is kept and the panel is allowed to overflow — clamping the main axis would
    /// slide the bubble *over* the target it describes. A smaller panel is the fix.
    #[test]
    fn beside_keeps_the_preferred_side_when_neither_fits() {
        // A viewport barely taller than the anchor: no room above or below.
        let anchor = Rectangle::new(Point::new(10.0, 2.0), Size::new(40.0, 20.0));
        let vp = Size::new(800.0, 26.0);
        let panel = Size::new(80.0, 24.0);
        let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Top);
        assert_eq!(
            r.loc.y,
            2.0 - 24.0 - 6.0,
            "preferred side kept; the panel overflows rather than covering the anchor"
        );
    }

    /// Before the first paint caches a viewport, an infinite axis disables both the
    /// flip and the clamp against it — the panel simply goes on the preferred side.
    #[test]
    fn beside_infinite_viewport_keeps_the_preferred_side_unclamped() {
        let (anchor, panel, _) = beside_case();
        let vp = Size::new(f64::INFINITY, f64::INFINITY);
        let r = place_beside(anchor, panel, vp, 6.0, BesideSide::Bottom);
        assert_eq!(r.loc.y, 300.0 + 20.0 + 6.0, "preferred side kept, no flip");
        assert_eq!(
            r.loc.x,
            400.0 + (40.0 - 80.0) / 2.0,
            "centred and left negative — an infinite axis disables the clamp"
        );
    }

    /// End-to-end: an anchored overlay bakes the placement into the panel child's
    /// bounds on layout (so paint/hit-testing follow), and it is idempotent.
    #[test]
    fn anchored_overlay_places_panel_child_on_layout() {
        let anchor = Rectangle::new(Point::new(40.0, 100.0), Size::new(120.0, 30.0));
        let mut o = Overlay::new()
            .blocking(false)
            .anchored(anchor)
            .panel(Flex::column().child(Label::new("hi")))
            .open(true);
        // Simulate a layout pass: taffy placed the panel somewhere with a real size.
        o.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(300.0, 300.0), Size::new(120.0, 80.0));
        o.viewport.set(Size::new(800.0, 600.0));
        o.on_layout();
        let placed = o.panel_bounds();
        assert_eq!(placed.loc, Point::new(40.0, 134.0), "panel anchored below trigger");
        // Idempotent: a second on_layout must not compound the offset.
        o.on_layout();
        assert_eq!(o.panel_bounds().loc, placed.loc, "re-placing is idempotent");
    }
}
