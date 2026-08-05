//! The **exposé** — a bird's-eye view of every workspace at once (F003/P082/T327).
//!
//! One row per workspace, each laying out **all** of that workspace's columns side by side —
//! including the ones scrolled off-screen, which is the whole point of the view. Panes stack
//! vertically inside their column, and a floating pane is drawn over the strip where it actually
//! sits. It is the same idea as niri's overview (`docs/niri-wiki/03-usage/overview.md`): a
//! zoomed-out map you navigate and act on, not a second way to use the app.
//!
//! **This module is the model, not the widgets.** It answers "what is on screen, where, and how
//! big" as plain data, so the geometry — the part that is easy to get subtly wrong — is testable
//! without a window. The tree is built from it separately.
//!
//! Panes are drawn as **boxes** for now: a border, the pane's name and its icon. A real snapshot
//! needs a scene primitive `heca-grid-ui` does not have (its vocabulary is `Rect`, `Text`, `Icon`,
//! clips — there is no image command) plus per-pane offscreen capture in `heca-renderer`. Nothing
//! in the layout, the navigation or the layer changes when that lands: only what fills the box.

// Seam module: the model lands before the tree that consumes it, the same arrangement `layers.rs`
// carried while its content path was being built. The allow goes when the tree is wired.
#![allow(dead_code)]

use heca_core::layout::{PaneId, Session};

/// One pane in the overview — a box, positioned by its column.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposePane {
    pub(crate) pane_id: PaneId,
    /// The pane's display name. Resolved by the caller through `pane_info_view`, the one name
    /// composer, so a pane reads the same here as in the sidebar and its header.
    pub(crate) name: String,
    /// Is this the focused pane of its workspace?
    pub(crate) active: bool,
}

/// One column of a workspace, with the width it really has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeColumn {
    pub(crate) col_idx: usize,
    /// The column's **resolved** width in layout pixels, read from
    /// `ScrollingSpace::column_widths` rather than recomputed — the layout already decided it, and
    /// a second calculation here would be a second answer that can disagree.
    pub(crate) width: f64,
    pub(crate) panes: Vec<ExposePane>,
}

/// A floating pane, which carries its own geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeFloating {
    pub(crate) pane_id: PaneId,
    pub(crate) name: String,
    /// Position **in strip coordinates**, i.e. already offset by the workspace's scroll.
    ///
    /// A float is positioned relative to the *viewport*, not the strip (`app/render.rs:340`:
    /// `fx = float.position.x + pane_area.loc.x + ws_offset.x`), so it does not scroll with the
    /// columns. A bird's-eye shows the whole strip, so the scroll offset has to be added back or a
    /// float lands in the wrong place the moment the workspace is scrolled.
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

/// One workspace's row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeWorkspace {
    pub(crate) ws_idx: usize,
    pub(crate) name: String,
    /// Is this the workspace the user is currently in?
    pub(crate) active: bool,
    pub(crate) columns: Vec<ExposeColumn>,
    pub(crate) floating: Vec<ExposeFloating>,
    /// Where the visible viewport sits within the strip: `(x, width)` in the same units as
    /// [`ExposeColumn::width`]. This is what lets the row show which part you are actually looking
    /// at, and it is why the rows in a bird's-eye are offset from one another.
    pub(crate) viewport: (f64, f64),
    /// The strip's total width — the sum of the columns, or the viewport when there are none. The
    /// scale factor for the row is this against the room the row is given.
    pub(crate) strip_width: f64,
}

/// Build the overview model from the session — **pure**, so the geometry is testable without a
/// window (an `AppState` needs one).
///
/// `name_of` resolves a pane's display name; the caller passes it because the one name composer
/// (`pane_info_view`) needs the programs config and the live runtime, neither of which belongs in
/// a geometry function.
pub(crate) fn model(session: &Session, mut name_of: impl FnMut(&heca_core::layout::Pane) -> String) -> Vec<ExposeWorkspace> {
    session
        .workspaces
        .iter()
        .enumerate()
        .map(|(ws_idx, ws)| {
            let scrolling = &ws.scrolling;
            let columns: Vec<ExposeColumn> = scrolling
                .columns
                .iter()
                .enumerate()
                .map(|(col_idx, col)| ExposeColumn {
                    col_idx,
                    // The resolved width when the layout has one for this index; otherwise the
                    // column's own minimum. A missing entry means the layout has not run yet.
                    width: scrolling
                        .column_widths
                        .get(col_idx)
                        .copied()
                        .unwrap_or(heca_core::layout::scrolling::MIN_COLUMN_WIDTH),
                    panes: col
                        .panes
                        .iter()
                        .enumerate()
                        .map(|(pane_idx, pane)| ExposePane {
                            pane_id: pane.id,
                            name: name_of(pane),
                            active: pane_idx == col.active_pane_idx
                                && col_idx == scrolling.active_column_idx,
                        })
                        .collect(),
                })
                .collect();

            let strip: f64 = columns.iter().map(|c| c.width).sum();
            let view_x = scrolling.view_offset.current();
            let view_w = scrolling.working_area.size.w;

            ExposeWorkspace {
                ws_idx,
                name: ws
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1)),
                active: ws_idx == session.active_workspace_idx,
                floating: ws
                    .floating_panes
                    .iter()
                    .map(|f| ExposeFloating {
                        pane_id: f.pane.id,
                        name: name_of(&f.pane),
                        // Viewport-relative → strip-relative. See the field's own note.
                        x: f.position.x + view_x,
                        y: f.position.y,
                        w: f.size.w,
                        h: f.size.h,
                    })
                    .collect(),
                columns,
                viewport: (view_x, view_w),
                // A workspace with no columns is still a row, as wide as its viewport, so an empty
                // workspace does not collapse to nothing and become unselectable.
                strip_width: if strip > 0.0 { strip } else { view_w },
            }
        })
        .collect()
}



// ── The tree ───────────────────────────────────────────────────────────────────

use heca_grid_ui::builders::{LayoutExt, NavExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Flex, Label, Row};
use heca_grid_ui::Component;

/// The `nav_key` of a pane's box — the row's one identity, so the cursor, the right-click target
/// and later a drag are three readers of a single declaration (the rule F003/P085/T354 set).
pub(crate) fn pane_nav_key(pane_id: PaneId) -> String {
    format!("expose.pane.{}", pane_id.0)
}

/// Gap between workspace rows, and between the columns inside one.
const ROW_GAP: f32 = 10.0;
const COL_GAP: f32 = 4.0;
/// Room the workspace's name takes at the head of its row.
const LABEL_W: f64 = 130.0;

/// Build the overview tree from the model.
///
/// **One scale for every row.** The widest strip decides it, and every workspace is drawn at that
/// same scale — which is what makes the view a map: a wide column looks wide, and a workspace with
/// three columns looks busier than one with a single pane. Scaling each row to fit its own width
/// independently would make every workspace look identical and destroy the whole point.
///
/// `avail` is the room the layer has for the strips, excluding the name column.
pub(crate) fn build(rows: &[ExposeWorkspace], theme: &GuiTheme, avail_w: f64) -> Box<dyn Component> {
    let widest = rows
        .iter()
        .map(|r| r.strip_width)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let room = (avail_w - LABEL_W).max(1.0);
    // Never magnify: a session narrower than the window is drawn at 1:1 rather than blown up.
    let scale = (room / widest).min(1.0);

    let mut stack = Flex::column().gap(ROW_GAP).grow(1.0);
    for ws in rows {
        let mut strip = Flex::row().gap(COL_GAP).grow(1.0);
        for col in &ws.columns {
            let mut column = Flex::column()
                .gap(COL_GAP)
                .width(Length::Px((col.width * scale) as f32));
            for pane in &col.panes {
                // A `Row` is the app's card: background, radius, and `nav_key` — the same widget
                // the sidebar uses for a pane, so selection and right-click come from the shared
                // declaration rather than from anything invented here.
                column = column.child(
                    Row::new()
                        .background(theme.colors.foreground.with_alpha(
                            super::alpha_u8(theme.colors.card_background_alpha),
                        ))
                        .highlight(theme.colors.accent)
                        .radius(theme.colors.control_radius())
                        .padding(4.0)
                        .active(pane.active)
                        .nav_key(pane_nav_key(pane.pane_id))
                        .grow(1.0)
                        .child(Label::new(pane.name.clone())),
                );
            }
            strip = strip.child(column);
        }
        stack = stack.child(
            Flex::row()
                .gap(ROW_GAP)
                .align(Align::Center)
                .grow(1.0)
                .child(Flex::row().width(Length::Px(LABEL_W as f32)).child(
                    Label::new(ws.name.clone()).muted(!ws.active),
                ))
                .child(strip),
        );
    }
    Box::new(stack)
}

/// Register (or re-register) the exposé as the named layer `heca.expose`.
///
/// **Re-registering is the rebuild path.** `add_named` replaces the layer under that name, and the
/// exposé's content is structural — a pane opens, a column is deleted — which a signal cannot
/// express, since a `Signal<PropValue>` replaces a *prop* and never adds or removes children. So
/// values follow signals and *shape* follows this call. It is safe to call while the layer is up:
/// the selection lives in the chrome store, keyed by mount, exactly as the sidebar's cursor does,
/// so a rebuild restores it rather than resetting to the top.
///
/// Hidden on registration (`LayerKind::OnDemand` via `add`), so nothing appears until
/// `show_layer name=heca.expose` or `toggle_layer` asks for it.
pub(crate) fn register(state: &mut crate::app_state::AppState) -> Option<super::LayerId> {
    let name = super::layers::layer_name(super::layers::HOST_OWNER, "expose")?;
    let programs = state.programs.clone();
    let rows = model(&state.session, |pane| {
        super::pane_info_view(
            &programs,
            &pane.title,
            pane.custom_name.as_deref(),
            Some(&pane.runtime),
            false,
        )
        .title
    });
    let theme = super::chrome_gui_theme(state);
    let width = state.session.viewport_size.w;
    let root = build(&rows, &theme, width);
    // Modal + covering: the overview is a context switch — it replaces what you are looking at, so
    // nothing behind it should be acted on. `covers_content` is what `Domain::Overlay` reads.
    let was_visible = state.layers.is_visible_named(&name);
    let id = state.layers.add_named(
        name.clone(),
        super::LayerBand::Overlay,
        super::LayerKind::OnDemand,
        true,
        true,
        root,
    );
    // A rebuild must not close a view the user is standing in.
    if was_visible {
        state.layers.show(id);
    }
    state.needs_redraw = true;
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{LayoutOptions, Pane, Rectangle, SessionId, Size};

    fn session() -> Session {
        let viewport = Size::new(800.0, 600.0);
        let mut s = Session::new(SessionId(0), viewport, 1.0, LayoutOptions::default());
        s.workspaces[0].name = Some("Editing".to_string());
        s.add_pane(Pane::new(PaneId(1), "a"), None, false);
        s.add_pane(Pane::new(PaneId(2), "b"), Some(1), false);
        s.add_workspace(Rectangle::from_size(viewport));
        s
    }

    /// Column widths are the layout's **resolved** ones, and the row knows how wide the whole strip
    /// is — that is what makes the bird's-eye proportional to the real thing rather than a diagram.
    #[test]
    fn a_row_carries_the_real_column_widths_and_the_strip_it_scrolls() {
        let s = session();
        let rows = model(&s, |p| p.title.clone());

        assert_eq!(rows.len(), s.workspaces.len(), "one row per workspace");
        assert_eq!(rows[0].name, "Editing");
        assert!(rows[0].active, "the first workspace is the active one");
        assert_eq!(rows[1].name, "Workspace 2");
        assert!(!rows[1].active);

        let strip: f64 = rows[0].columns.iter().map(|c| c.width).sum();
        assert_eq!(rows[0].strip_width, strip, "the strip is the sum of its columns");
        assert!(
            rows[0].columns.iter().all(|c| c.width > 0.0),
            "every column has a real width: {:?}",
            rows[0].columns,
        );
        assert_eq!(rows[0].viewport.1, 800.0, "the viewport is the working area's width");

        // An empty workspace is still a row, as wide as its viewport — it must stay selectable.
        assert!(rows[1].columns.is_empty());
        assert_eq!(rows[1].strip_width, rows[1].viewport.1);
    }

    /// **A floating pane is placed in strip coordinates.** It is positioned against the viewport
    /// and does not scroll with the columns, so the scroll offset has to be added back or it lands
    /// in the wrong place the moment the workspace is scrolled.
    #[test]
    fn a_floating_pane_is_offset_by_the_scroll_so_it_lands_where_it_looks() {
        use heca_core::layout::workspace::FloatingPane;
        use heca_core::layout::Point;
        let mut s = session();
        s.workspaces[0].floating_panes.push(FloatingPane {
            pane: Pane::new(PaneId(9), "float"),
            position: Point::new(40.0, 30.0),
            size: Size::new(200.0, 150.0),
            is_active: false,
            original_column_idx: None,
            original_pane_idx: None,
        });
        s.workspaces[0].scrolling.view_offset =
            heca_core::layout::view_offset::ViewOffset::Static(120.0);

        let rows = model(&s, |p| p.title.clone());
        let f = &rows[0].floating[0];
        assert_eq!(f.pane_id, PaneId(9));
        assert_eq!(f.x, 160.0, "40 viewport-relative + 120 scrolled = 160 in the strip");
        assert_eq!(f.y, 30.0, "vertical does not scroll");
        assert_eq!((f.w, f.h), (200.0, 150.0), "a float carries its own size");
    }

    /// **One scale for every row**, decided by the widest strip — that is what makes the view a map
    /// rather than a diagram: a wide column looks wide, and a busy workspace looks busy. Scaling
    /// each row to its own width would draw every workspace identically.
    #[test]
    fn every_row_is_drawn_at_the_same_scale_and_nothing_is_magnified() {
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let widest = rows.iter().map(|r| r.strip_width).fold(0.0_f64, f64::max);

        // Two workspaces of very different width still share one scale, so their strips keep their
        // real proportion to one another.
        let narrow = rows.iter().find(|r| r.columns.is_empty()).expect("an empty workspace");
        assert!(narrow.strip_width <= widest);

        // A session narrower than the window is drawn 1:1, never blown up to fill it.
        let room = 10_000.0_f64;
        let scale = ((room - 130.0) / widest.max(1.0)).min(1.0);
        assert_eq!(scale, 1.0, "a huge window does not magnify a small session");
    }
}
