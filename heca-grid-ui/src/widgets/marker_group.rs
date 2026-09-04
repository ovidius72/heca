//! [`MarkerGroup`] — a vertical group of rows fronted by a left **marker bar**
//! that brightens to the theme accent when the group is active, and reads as a
//! **grip** (the group's own grab/target surface) when hovered.
//!
//! Domain-neutral and presentation-only: a host composes child rows into the
//! group and flips [`active`](MarkerGroup::active) to indicate the group holds
//! the current selection (e.g. a sidebar *column* whose bar lights when it holds
//! the focused pane, a list *section*, …). The widget encodes **no** app concept
//! — never name it for workspace/column/pane.
//!
//! ## The bar as the group's surface (grip + target)
//! A group's only *own* surface is its left **gutter** — the rows fill the rest.
//! That gutter is the seam for the group's universal capabilities, applied by the
//! host, **not** built in here:
//! - **Drag**: mark the group `.draggable(payload)` ([`ComponentExt`](crate::builders::ComponentExt)).
//!   A pointer press in the gutter resolves to the *group* (move the whole group);
//!   a press on a child row resolves to the *row* — innermost-first hit-testing
//!   makes the gutter the group's drag handle for free.
//! - **Pick/hint**: wrap the group in [`KeyHint`](super::KeyHint); its keycap is an
//!   overlay anchored at the **top of the bar**, so it can render larger than the
//!   thin gutter.
//!
//! To make the gutter a comfortable, discoverable handle, it is widened to a
//! [`GRIP_W`]-wide hit zone (the visible bar stays thin) and **brightens +
//! thickens on hover**. (The OS grab *cursor* is a windowing concern the host
//! sets from this hover state — `heca-grid-ui` only emits the visual.) All bar
//! styling is read from the [`Theme`](crate::theme::Theme) at paint.

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, Event, Handled, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Glow;
use crate::style::Direction;
use heca_core::layout::{Point, Rectangle, Size};

/// Width of the left **grip gutter** (logical px): the group's own grab/target
/// hit zone + keycap anchor. Children start after it; the visible bar is drawn at
/// its left edge (thinner than the gutter — the extra is invisible grab padding).
const GRIP_W: f64 = 12.0;
/// Resting visible bar width (logical px). Matches [`Row`](super::Row)'s active bar.
const BAR_W: f64 = 3.0;
/// Thickened bar width on hover (the "grab here" affordance).
const BAR_W_HOVER: f64 = 5.0;
/// Vertical inset of the bar from the group's top/bottom (logical px), so the
/// rounded bar caps don't touch the edges.
const BAR_INSET: f64 = 2.0;
/// Base glow falloff radius (logical px) of the active bar. **Config-driven:**
/// `cx.rect` scales this by `theme.colors.glow_size` and drops the glow entirely at
/// `GlowLevel::None` (see [`PaintCx::rect`](crate::component::PaintCx::rect)).
const BAR_GLOW_RADIUS: f32 = 8.0;
/// Base glow strength of the active bar, **scaled at paint by `theme.colors.intensity`**
/// (`Off` ⇒ no glow). Config-driven, not a fixed look.
const BAR_GLOW_INTENSITY: f32 = 0.16;

/// A vertical group of rows with a left marker bar. Flip
/// [`active`](MarkerGroup::active) to brighten the bar to the theme accent; the
/// bar also brightens/thickens while its grip gutter is hovered.
pub struct MarkerGroup {
    base: Base,
    /// Active state: brightens the bar to the accent (+ glow). Bind reactively
    /// via [`state`](MarkerGroup::state).
    active: Signal<bool>,
    /// Sidebar-nav cursor state: a full-opacity bar **without** the active glow,
    /// so the cursor reads distinctly from the active column.
    nav: Signal<bool>,
    /// Pointer is over the left grip gutter (drives the hover affordance).
    hovered: Signal<bool>,
}

#[heca_grid_ui_macros::props]
impl MarkerGroup {
    /// A new (empty) group. Add rows with `.child(...)`.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        Self {
            base,
            active: signal(false),
            nav: signal(false),
            hovered: signal(false),
        }
    }

    /// Set the active state (brightens the bar).
    #[heca_grid_ui_macros::prop]
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// The active-state signal — bind UI to it reactively (the host writes it when
    /// the group's selection changes; the bar repaints from it).
    pub fn state(&self) -> Signal<bool> {
        self.active
    }

    /// Set the sidebar-nav **cursor** state (full-opacity bar, no active glow).
    #[heca_grid_ui_macros::prop]
    pub fn nav_selected(self, on: bool) -> Self {
        self.nav.set(on);
        self
    }

    /// The nav-cursor signal — bind UI to it reactively.
    pub fn nav_state(&self) -> Signal<bool> {
        self.nav
    }

    /// `true` while the pointer is over the left grip gutter. The host reads this
    /// to set the OS grab cursor (a windowing concern outside this crate).
    pub fn hovered(&self) -> Signal<bool> {
        self.hovered
    }

    /// Is `pos` within the left grip gutter (the group's own grab/target surface)?
    fn in_grip(&self, pos: Point) -> bool {
        let b = self.base.bounds;
        b.contains(pos) && (pos.x - b.loc.x) <= GRIP_W
    }
}

impl Default for MarkerGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MarkerGroup {
    /// The navigation cursor is "the current one" for this list, so an enclosing scroll region
    /// keeps it in view — the keyboard half of scrolling, without the host wiring it per list.
    fn wants_visible(&self) -> bool {
        self.nav.get_untracked() || self.base.focused_by_keyboard()
    }

    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Reserve the left grip gutter so child rows never overlap it. The rest of
    /// the layout is the base column style.
    fn taffy_style(&self) -> taffy::Style {
        use taffy::prelude::length;
        let mut s = self.base.style.layout.to_taffy();
        s.padding.left = length(GRIP_W as f32);
        s
    }

    /// Capture returning `No`: the grip's hover is tracked whatever the rows do with the move,
    /// and the rows still receive it — they own everything outside the gutter.
    ///
    /// This is a **sub-region** hover, not the widget's own: the grip is a gutter this widget
    /// paints, not a child, so [`Base::hovered`](crate::component::Base::hovered) (which is true
    /// anywhere over the group) cannot answer it. The move already arrives only when the pointer
    /// is over this group.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if let Some(p) = ev.pointer()
            && matches!(ev, Event::PointerMove(_) | Event::PointerLeave(_))
        {
            let in_grip = matches!(ev, Event::PointerMove(_)) && self.in_grip(p.pos);
            if self.hovered.get_untracked() != in_grip {
                self.hovered.set(in_grip);
            }
        }
        Handled::No
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        cx.paint_base(&self.base);

        // Left marker bar, spanning the group's height (inset for rounded caps).
        // Theme-driven: accent + glow when active; brighter + thicker when the
        // grip is hovered; a dimmed accent at rest.
        let active = self.active.get_untracked();
        let nav = self.nav.get_untracked();
        let hovered = self.hovered.get_untracked();
        // All effect parameters come from the theme (→ config.toml): glow color,
        // plus its strength via `glow_size` — `GlowLevel` is the sole owner of
        // glow (presence + radius + strength); `intensity` no longer feeds glow
        // (it owns scanlines only). Nothing about the *look* is fixed here.
        let (accent, glow_c, glow_strength) = {
            let t = cx.theme();
            (t.colors.accent, t.colors.glow, t.colors.glow_size.strength_scale())
        };
        let b = self.base.bounds;
        let bar_w = if hovered { BAR_W_HOVER } else { BAR_W };
        let bar = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + BAR_INSET),
            Size::new(bar_w, (b.size.h - 2.0 * BAR_INSET).max(0.0)),
        );
        let (color, glow) = if active {
            let g = Glow {
                color: glow_c,
                radius: BAR_GLOW_RADIUS,
                intensity: BAR_GLOW_INTENSITY * glow_strength,
            };
            (accent, Some(g))
        } else if nav {
            // Nav cursor: full-opacity accent, but no glow — a "lit but flat" bar,
            // clearly the cursor yet distinct from the active column's glowing bar.
            (accent, None)
        } else if hovered {
            (accent.with_alpha(cx.theme().colors.interaction.thumb_hover), None)
        } else {
            (accent.with_alpha(cx.theme().colors.interaction.thumb_rest), None)
        };
        // Radius from the theme token (small-control radius) — not a hardcoded
        // width/2 literal — so `border_radius` in config / `prefix+Shift+r` reflows
        // the bar. The SDF path clamps radius to half the bar width, so default themes
        // still render a pill; a sharp theme (border_radius 0) makes it square.
        cx.rect(bar, color, None, cx.theme().colors.control_radius(), glow);

        // Children paint on top of (right of) the gutter.
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for MarkerGroup {}
impl Parent for MarkerGroup {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Component;

    fn sized(g: MarkerGroup, w: f64, h: f64) -> MarkerGroup {
        let mut g = g;
        g.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(w, h));
        g
    }

    #[test]
    fn defaults_inactive_and_unhovered() {
        let g = MarkerGroup::new();
        assert!(!g.state().get_untracked(), "a fresh group is inactive");
        assert!(!g.hovered().get_untracked(), "a fresh group is not hovered");
    }

    #[test]
    fn active_sets_the_signal() {
        let g = MarkerGroup::new().active(true);
        assert!(g.state().get_untracked());
    }

    #[test]
    fn state_handle_drives_the_widget() {
        // The host binds the returned signal; setting it flips the group's state
        // without rebuilding the widget.
        let g = MarkerGroup::new();
        g.state().set(true);
        assert!(
            g.state().get_untracked(),
            "writing the state handle activates the group"
        );
    }

    #[test]
    fn reserves_left_grip_gutter() {
        let g = MarkerGroup::new();
        let s = g.taffy_style();
        assert_eq!(s.padding.left, taffy::prelude::length(GRIP_W as f32));
    }

    #[test]
    fn hover_tracks_only_the_left_grip() {
        let mut g = sized(MarkerGroup::new(), 100.0, 40.0);
        crate::component::dispatch(&mut g, &Event::pointer_moved(Point::new(5.0, 20.0))); // x < GRIP_W → gutter
        assert!(
            g.hovered().get_untracked(),
            "pointer in the grip gutter hovers the bar"
        );
        crate::component::dispatch(&mut g, &Event::pointer_moved(Point::new(60.0, 20.0))); // over content
        assert!(
            !g.hovered().get_untracked(),
            "pointer over content does not hover the bar"
        );
    }

    #[test]
    fn hosts_children() {
        let g = MarkerGroup::new()
            .child(crate::widgets::Label::new("a"))
            .child(crate::widgets::Label::new("b"));
        assert_eq!(g.base().children.len(), 2);
    }
}
