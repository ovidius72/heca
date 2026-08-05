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



// ── The composition ───────────────────────────────────────────────────────────

use heca_grid_ui::builders::{LayoutExt, NavExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{CardGrid, Flex, GridCell, Label, Overlay, Row, ScrollRegion, Surface};
use heca_grid_ui::Component;

/// The `nav_key` of a pane's box — the row's one identity, so the cursor, the right-click target
/// and later a drag are three readers of a single declaration (F003/P085/T354).
pub(crate) fn pane_nav_key(pane_id: PaneId) -> String {
    format!("expose.pane.{}", pane_id.0)
}

/// The gutter holding a workspace's name — a fixed label column, so the strips all start at the
/// same x whatever a workspace is called.
const GUTTER_W: f32 = 120.0;
/// Smallest a workspace row may be drawn before the stack starts scrolling instead of squeezing.
const ROW_MIN_H: f32 = 90.0;

/// A share expressed as `flex_grow`, plus the two things that make it a share.
///
/// `flex_grow` alone does **not** divide a region — it distributes only *positive* free space, so a
/// row of `grow(1.0)` children collapses to its content. A share needs a **zero base size and
/// permission to shrink** as well (CSS `flex: 1 1 0`). Written once here so no caller re-derives it.
fn share<T: LayoutExt + Component>(node: T, weight: f64, vertical: bool) -> T {
    let node = node.grow(weight.max(0.001) as f32);
    let node = match vertical {
        true => node.height(Length::Px(0.0)),
        false => node.width(Length::Px(0.0)),
    };
    node.shrink(1.0)
}

/// Build the overview — **a composition, not a widget**: it binds `PaneId` to a `FocusPane` intent
/// and the `close_overlay` action, which is the only reason it is not in `heca-grid-ui`.
///
/// Plain data in, a `Component` out, and **never `&AppState`** (§ 0b): the session is already
/// reduced to [`ExposeWorkspace`]s by [`model`], so this is testable without a window.
///
/// **Nothing here computes pixels.** Sizes are declared as relationships and taffy resolves them —
/// a column's width is its share of the row, weighted by the column's real width, so the row is
/// always filled and the proportions are still true. Gaps and padding are `Spacing` tokens, which
/// scale with the font, rather than literals. The earlier version hand-computed a scale, a row
/// height and a pane height, which is the layout engine's job and is why the map wasted its width.
///
/// What is drawn, and what is not:
/// - **A workspace is a row.** No box, no border, no fill — vertical position and a muted name.
/// - **A column is invisible grouping** that takes its share of the row's width.
/// - **A pane is the only thing with a surface.**
///
/// The overlay behaviour is [`Overlay`]'s, the cursor is [`CardGrid`]'s, and the overflow is
/// [`ScrollRegion`]'s. None of the three is re-implemented here.
pub(crate) fn map(
    rows: &[ExposeWorkspace],
    theme: &GuiTheme,
    emit: super::ChromeIntentEmitter,
    start: Option<PaneId>,
) -> Box<dyn Component> {
    // **One behaviour, two ways in.** Choosing a pane closes the map and focuses it, whether the
    // choice came from the cursor (`CardGrid::on_activate`) or a click on the card itself. Defined
    // once here so the mouse and the keyboard can never drift apart.
    let choose = {
        let emit = emit.clone();
        std::rc::Rc::new(move |pane_id: PaneId| {
            // Close FIRST: `FocusPane` acts on the tiled content and is refused while the map
            // covers it. Both are queued and applied in order.
            emit(crate::app::interaction::InteractionIntent::ActivateAction(
                crate::input::WmAction::CloseOverlay { overlay: None },
            ));
            emit(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
        })
    };

    // Workspaces need air between them or two rows of panes read as one grid.
    let mut grid = CardGrid::new().gap_spacing(Spacing::Md);
    for ws in rows {
        let mut columns_of_cells: Vec<Vec<GridCell>> = Vec::new();
        let mut strip = Flex::row().gap_spacing(Spacing::Xs).grow(1.0);
        for col in &ws.columns {
            let mut cells = Vec::new();
            let mut column = Flex::column().gap_spacing(Spacing::Xs);
            for pane in &col.panes {
                let card = Row::new();
                cells.push(GridCell::new(pane.pane_id.0.to_string(), card.nav_state()));
                let card = card
                    .background(theme.colors.foreground.with_alpha(
                        super::alpha_u8(theme.colors.card_background_alpha),
                    ))
                    .highlight(theme.colors.accent)
                    .radius(theme.colors.control_radius())
                    .pad_all(Spacing::Xs)
                    .active(pane.active)
                    .nav_key(pane_nav_key(pane.pane_id))
                    // Click to go there. `Row` already provides the hover tint and the press
                    // handling, so the mouse costs one line rather than a second input path.
                    .on_activate({
                        let choose = choose.clone();
                        let id = pane.pane_id;
                        move || choose(id)
                    })
                    .child(Label::new(pane.name.clone()));
                // Panes divide their column's height evenly — each an equal share.
                column = column.child(share(card, 1.0, true));
            }
            // **A column's width is its fraction of the VIEWPORT, not a share of the row.**
            //
            // A share always fills, so one column became the whole row and a workspace holding a
            // single half-screen pane looked exactly like one holding four. The map has to keep the
            // relationship to the screen, not just the ratio between columns: half the viewport is
            // half the row, and a strip wider than the viewport runs past the row's edge — which is
            // the truth about that workspace and the reason the view exists.
            //
            // A ratio, not a computed pixel: taffy resolves `Pct` against the row, so nothing here
            // knows how wide the row will be.
            let frac = (col.width / ws.viewport.1.max(1.0)) as f32;
            strip = strip.child(column.width(Length::Pct(frac)).shrink(0.0));
            columns_of_cells.push(cells);
        }
        let row = Flex::row()
            .gap_spacing(Spacing::Sm)
            .align(Align::Stretch)
            // A minimum so a row stays legible once there are many; past that the stack scrolls.
            .min_height(Length::Px(ROW_MIN_H))
            .child(
                Flex::row()
                    .width(Length::Px(GUTTER_W))
                    .align(Align::Center)
                    .child(Label::new(ws.name.clone()).muted(true)),
            )
            .child(strip);
        grid = grid.row(columns_of_cells, share(row, 1.0, true));
    }
    if let Some(id) = start {
        grid = grid.selected(id.0.to_string());
    }

    // Dismissal goes through the **action**, not a bespoke hide: `close_overlay` is in the registry,
    // so it is bindable, rebindable in config, listed in the palette and reachable over RPC.
    let close = {
        let emit = emit.clone();
        move || {
            emit(crate::app::interaction::InteractionIntent::ActivateAction(
                crate::input::WmAction::CloseOverlay { overlay: None },
            ));
        }
    };
    let grid = grid
        .on_activate({
            let choose = choose.clone();
            move |key| {
                if let Ok(id) = key.parse::<u64>() {
                    choose(PaneId(id));
                }
            }
        })
        .on_dismiss(close);

    Box::new(
        Overlay::new()
            .blocking(true)
            .panel_size(Length::Pct(1.0), Length::Pct(1.0))
            // The panel carries the opaque surface: heca's window colour is translucent by design
            // (the frosted compositor), so a map painted with it lets the app read straight through.
            .panel(
                Surface::column()
                    .background(theme.colors.background.with_alpha(255))
                    .pad_all(Spacing::Md)
                    // **The overflow is the scroll region's.** Enough workspaces and the rows run
                    // past the window; nothing here needs to know how many fit.
                    .child(ScrollRegion::new().grow(1.0).child(grid)),
            )
            .open(true),
    )
}


/// Register (or re-register) the exposé as the named layer `heca.expose` — **host wiring only**.
///
/// The composition itself is [`map`], which takes plain data; this is the part that needs
/// `AppState`, and it does nothing but gather it.
///
/// **Re-registering is the rebuild path.** `add_named` replaces the layer under that name, and the
/// content is structural — a pane opens, a column is deleted — which a signal cannot express. So
/// values follow signals and shape follows this call. It is safe while the view is up: the layer is
/// re-shown if it was showing.
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
    let event_proxy = state.event_proxy.clone();
    let emit: super::ChromeIntentEmitter = std::rc::Rc::new(move |intent| {
        let _ = event_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: crate::app::interaction::InteractionSource::Keyboard,
            intent,
        });
    });
    // Open on the pane you are in, so the map starts where you are standing.
    let here = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    let root = map(&rows, &theme, emit, here);
    let was_visible = state.layers.is_visible_named(&name);
    let id = state.layers.add_named(
        name.clone(),
        super::LayerBand::Overlay,
        super::LayerKind::OnDemand,
        true,
        true,
        root,
    );
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
        s.add_pane(Pane::new(PaneId(2), "b"), None, false);
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



    /// **Dismiss must reach the grid through the overlay.** The layer root is an `Overlay`, whose
    /// panel is a `Surface`, whose child is the `CardGrid` that answers the intent — so this pins
    /// the whole routing chain the widget keymap relies on when the layer is the top modal.
    #[test]
    fn a_dismiss_reaches_the_grid_through_the_overlay() {
        use heca_grid_ui::component::{Event, WidgetIntent as W};
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let sink = seen.clone();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(move |intent| {
            sink.borrow_mut().push(format!("{intent:?}"));
        });
        let mut root = map(&rows, &theme, emit, None);

        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
        let got = seen.borrow().join(" ");
        assert!(
            got.contains("CloseOverlay"),
            "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
        );
    }

}
