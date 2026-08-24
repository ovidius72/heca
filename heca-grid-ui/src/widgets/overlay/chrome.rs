//! **What an overlay panel looks like** — the drop shadow that lifts it off the page, the theme
//! surface fill, a widget's own accents, and the edge the user asked for.
//!
//! One authority, because the base [`Overlay`](super::Overlay) and the widgets that own their own
//! panel — a [`Select`](crate::widgets::Select) dropdown, whose option rows are *placed* children —
//! must not drift apart.

use crate::component::PaintCx;
use crate::scene::{Border, Glow, Shadow};
use crate::theme::FrameStyle;
use heca_core::layout::Rectangle;

/// Multiplier on the theme `shadow.blur` token — an overlay panel is large and
/// wants a wider, softer halo than the small-surface base token. Raised from 4.0
/// (2026-07-24): the panel read as barely lifted off the page.
const SHADOW_BLUR_MULT: f32 = 6.0;
/// Downward shadow offset lifting the panel off the scrim/page. Raised from 12.0
/// alongside the blur so the panel sits more clearly *above* what's behind it.
const SHADOW_DROP: f32 = 16.0;

/// Optional per-widget accents layered onto the shared overlay panel chrome by
/// [`paint_panel_chrome`] — a specialization's own identity (e.g. a
/// [`Select`](crate::widgets::Select) dropdown's accent edge + neon halo). The drop shadow
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
/// [`Tooltip`](crate::widgets::Tooltip) bubble, the same shadow is **larger than the surface
/// casting it** — user-verified 2026-07-25 as "too much". A transient hover bubble
/// is not at dialog depth, so it does not take the dialog's shadow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelElevation {
    /// A surface that owns the screen: [`Dialog`](crate::widgets::Dialog), a
    /// [`Select`](crate::widgets::Select) dropdown, [`ContextMenu`](crate::widgets::ContextMenu),
    /// [`CommandPalette`](crate::widgets::CommandPalette). Full shadow depth.
    #[default]
    Panel,
    /// A transient surface hovering just above its target — the
    /// [`Tooltip`](crate::widgets::Tooltip) bubble. Same shadow shape at
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
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{Point, Size};

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
}
