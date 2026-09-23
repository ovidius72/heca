//! **Where an overlay panel goes** — hung off a trigger rect, dropped at a point, or set beside a
//! target, and always clamped into the viewport.
//!
//! Free functions rather than methods on [`Overlay`](super::Overlay), because the widgets that
//! place their own panel ([`Select`](crate::widgets::Select),
//! [`ContextMenu`](crate::widgets::ContextMenu)) must share the one flip/clamp rule instead of
//! hand-rolling a second one.

use heca_core::layout::{Point, Rectangle, Size};

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

/// Which side of the anchor an [`Anchored`](OverlayPosition::Anchored) panel goes on.
///
/// [`Auto`](AnchorSide::Auto) is the usual choice — the placement picks below,
/// flipping above when there is no room. A caller that has **already** decided the
/// direction passes [`Below`](AnchorSide::Below) / [`Above`](AnchorSide::Above) so
/// the placement honours it instead of re-deciding: [`Select`](crate::widgets::Select) does
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
                y = if room_above > room_below {
                    above_y
                } else {
                    below_y
                };
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
/// [`ContextMenu`](crate::widgets::ContextMenu)'s `layout` reimplemented inline. As with
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
/// Re-exported as [`TooltipSide`](crate::widgets::TooltipSide) — the same type under the
/// name that reads better at a [`Tooltip`](crate::widgets::Tooltip) call site.
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
/// [`Tooltip`](crate::widgets::Tooltip) used to hand-roll. It is a different rule in kind
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            r.loc.y,
            560.0 - 4.0 - 80.0,
            "flipped to sit above the anchor"
        );
    }

    /// Neither side fully fits → take the side with more room, then clamp on-screen.
    #[test]
    fn anchored_picks_more_room_when_neither_side_fits() {
        // Tall panel (500) in a short viewport (600); anchor low → more room above.
        let anchor = Rectangle::new(Point::new(0.0, 450.0), Size::new(100.0, 30.0));
        let panel = Size::new(100.0, 500.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        // Room above (450-4=446) > room below (600-484=116) → flip up, then clamp ≥ 0.
        assert_eq!(
            r.loc.y, 0.0,
            "clamped to the top after choosing the roomier side"
        );
    }

    /// The panel is clamped so it never spills past the right/bottom viewport edge.
    #[test]
    fn anchored_clamps_into_viewport() {
        let anchor = Rectangle::new(Point::new(760.0, 20.0), Size::new(120.0, 30.0));
        let panel = Size::new(120.0, 80.0);
        let r = place_anchored(anchor, panel, Size::new(800.0, 600.0), 4.0);
        assert_eq!(
            r.loc.x,
            800.0 - 120.0,
            "right edge clamped into the viewport"
        );
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
        assert_eq!(
            forced.loc.y,
            600.0 - 80.0,
            "forced Below clamps, does not flip"
        );

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
        assert_eq!(
            r.loc.y,
            300.0 - panel.h - 6.0,
            "main axis untouched by the clamp"
        );
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
}
