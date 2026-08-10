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
    /// columns. A bird's-eye shows the whole strip, so the viewport's own strip position has to be
    /// added back or a float lands in the wrong place the moment the workspace is scrolled. That
    /// position is `view_pos()` — `column_x(active) + view_offset` — **not** `view_offset`, which
    /// is measured from the active column.
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
    /// The workspace's viewport height — the other half of its shape, so a row can be drawn as the
    /// screen scaled rather than as a bar of arbitrary height.
    pub(crate) viewport_h: f64,
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
            // **`view_offset` is relative to the ACTIVE COLUMN, not to the strip.** The strip
            // position of the visible area is `column_x(active) + view_offset`, which the layout
            // already spells as `view_pos()`. Read raw, the offset put the workspace rectangle at
            // the wrong place in every scrolled workspace — the map centred on a window that was
            // never where it said, and the pane you were on sat off the right edge.
            let view_x = scrolling.view_pos();
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
                        // (`view_x` is `view_pos()`, the viewport's own place in the strip.)
                        y: f.position.y,
                        w: f.size.w,
                        h: f.size.h,
                    })
                    .collect(),
                columns,
                viewport: (view_x, view_w),
                viewport_h: scrolling.working_area.size.h,
                // A workspace with no columns is still a row, as wide as its viewport, so an empty
                // workspace does not collapse to nothing and become unselectable.
                strip_width: if strip > 0.0 { strip } else { view_w },
            }
        })
        .collect()
}



// ── The composition ───────────────────────────────────────────────────────────

use heca_grid_ui::builders::{LayoutExt, ComponentExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    CardGrid, Flex, GridCell, Label, Overlay, RevealAlign, Row, ScrollRegion, Surface,
};
use heca_grid_ui::Component;

/// The `nav_key` of a pane's box — the row's one identity, so the cursor, the right-click target
/// and later a drag are three readers of a single declaration (F003/P085/T354).
pub(crate) fn pane_nav_key(pane_id: PaneId) -> String {
    format!("expose.pane.{}", pane_id.0)
}

/// How long the map takes to settle when the cursor moves, as a time constant in seconds.
///
/// The view **glides** rather than cutting: a cut between two positions of the same picture is
/// read as a different picture, and the whole value of a map is knowing where you just came from.
/// Short enough (~95% inside 250ms) that it never feels like waiting.
const SCROLL_EASE: f32 = 0.09;
/// How long the map takes to dissolve when it is dismissed, in seconds.
const FADE_OUT: f32 = 0.14;

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
    geometry: &heca_core::layout::LayoutOptions,
) -> Box<dyn Component> {
    // **The map's scale and spacing are the layout's, not this module's.** `overview_scale` and
    // `overview_gap` are `LayoutOptions` fields the user sets in config; hardcoding a zoom here
    // would be a second answer to a question the layout already answers, and an unreachable one.
    let zoom = geometry.overview_scale;
    let gap_frac = geometry.overview_gap;
    // **One behaviour, two ways in.** Choosing a pane closes the map and focuses it, whether the
    // choice came from the cursor (`CardGrid::on_activate`) or a click on the card itself. Defined
    // once here so the mouse and the keyboard can never drift apart.
    let choose = {
        let emit = emit.clone();
        std::rc::Rc::new(move |pane_id: PaneId| {
            // **Choosing a card is activate-and-leave**, the same three-part act the sidebar
            // performs when you pick a pane from it (`providers/workspaces/mod.rs:466`):
            //
            //   focus the pane → hand the keyboard back → put the surface away.
            //
            // The middle step is the one that is invisible until it is missing. `focus_pane`
            // changes which pane is *active* and deliberately nothing else — it does not release
            // container focus, because whether the keyboard should follow is a property of what
            // the user asked for, not of a pane being focused (a peek focuses without leaving).
            // Without it the map focused the right pane and the keyboard stayed where it was, so
            // choosing a card looked like it had done nothing at all.
            emit(crate::app::interaction::InteractionIntent::FocusPaneThenAction {
                pane_id,
                action: Box::new(crate::input::WmAction::UnfocusDock),
            });
            emit(crate::app::interaction::InteractionIntent::ActivateAction(
                crate::input::WmAction::CloseOverlay { overlay: None },
            ));
        })
    };

    // **niri's model, and it is not a scroll.** `monitor.rs::workspaces_render_geo` positions the
    // whole stack by translation:
    //
    // ```text
    // static_offset = (view_size - ws_size) / 2      // the ACTIVE workspace rectangle, centred
    // first_ws_y    = -active_idx * ws_height_with_gap
    // y[i]          = first_ws_y + i * ws_height_with_gap + static_offset
    // ```
    //
    // Three things follow, and each is one this view got wrong before:
    //
    // - What is centred is the **workspace rectangle** — the screen-sized window onto that
    //   workspace — not the focused pane. Where the columns sit inside it is that workspace's own
    //   scroll offset, and whether the focused column is centred there is `center_focused_column`,
    //   a normal layout setting that applies outside the map too. So the map contains no centring
    //   logic of its own and the setting shows straight through it.
    // - **Nothing clamps.** With the first workspace active it is dead centre and the backdrop
    //   above it is empty. That is `overscroll` here.
    // - The gap is `view_height * 0.1 * zoom` — a tenth of a screen — so it scales with the map
    //   instead of being a fixed token that grows wrong at other zooms.
    // niri's gap: `view_height * 0.1 * zoom`, a tenth of a screen at this zoom. Not a `Spacing`
    // token — a token is tuned to text and would read as a hairline at zoom 0.25 and a canyon at
    // 0.75, while a tenth of a screen stays the same picture at every zoom.
    let gap = rows
        .iter()
        .map(|ws| ws.viewport_h * gap_frac * zoom)
        .fold(0.0_f64, f64::max) as f32;
    let mut grid = CardGrid::new().gap(gap).min_width(Length::Pct(1.0));
    for ws in rows {
        let mut columns_of_cells: Vec<Vec<GridCell>> = Vec::new();
        // The two axes want opposite things and each is a separate property: `grow` fills the
        // row's **height** (the region's main axis), `align_self` opts out of stretching across its
        // **width** so the strip measures its own columns and the region has real slack to
        // position into. Take the grow away and the cards collapse to a 7px sliver; take the
        // align_self away and the strip fills the region and there is nothing left to centre.
        let mut strip = Flex::row().grow(1.0).align_self(Align::Start);
        for col in &ws.columns {
            let mut cells = Vec::new();
            // Panes in a column want visible air between them: at this zoom the real gap scales
            // down to a hairline, and three stacked panes read as one striped box rather than
            // three. A token, so it tracks the font and the size setting like every other gap.
            let mut column = Flex::column().gap_spacing(Spacing::Sm);
            for pane in &col.panes {
                let card = Row::new();
                // The cursor follows the mouse: `CardGrid` reads this and moves onto the card you
                // point at, so hovering centres it exactly as arrowing onto it does.
                cells.push(
                    GridCell::new(pane.pane_id.0.to_string(), card.nav_state())
                        .hovered(card.hovered()),
                );
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
            // **A column is its real width times the zoom**, like everything else here — not a
            // share of the row. A share always fills, so one column became the whole row and a
            // workspace holding a single half-screen pane looked exactly like one holding four.
            // The relationship the map has to keep is to the *screen*: half the viewport is half
            // the workspace rectangle, and a strip scrolled past one screen really is wider.
            strip = strip.child(column.width(Length::Px((col.width * zoom) as f32)).shrink(0.0));
            columns_of_cells.push(cells);
        }
        // The workspace rectangle: the screen at this zoom. It is what a row is a picture of, and
        // what gets centred — so the map is declared, not computed, by naming it as the window this
        // row wants brought into view. Its x is the workspace's own scroll, which is exactly what
        // makes the columns you are looking at the ones in the middle.
        let row_h = (ws.viewport_h * zoom) as f32;
        // **No name gutter.** A fixed label column pushed every strip inwards, so a row stopped
        // lining up with the screen it is a picture of; niri's overview labels nothing either.
        let row = Flex::row()
            .align(Align::Stretch)
            .height(Length::Px(row_h))
            // **What gets centred is the card the cursor is on** — the one you are moving through
            // the map, not the one the session happens to have focused. Declaring the workspace's
            // visible rectangle instead centred a window positioned by the session's own scroll,
            // which is why the map kept centring the pane you had left rather than the one you
            // were on (Antonio, 2026-08-05).
            //
            // No scrollbar: a map positioned by where the cursor is has no "how far along am I" to
            // report — the picture already says it — and a bar across it reads as a divider
            // between things that are not divided.
            .child(
                ScrollRegion::new()
                    .horizontal()
                    .reveal_align(RevealAlign::Center)
                    .overscroll(true)
                    .scrollbars(false)
                    .smooth_scroll(SCROLL_EASE)
                    .grow(1.0)
                    .child(strip),
            );
        grid = grid.row(columns_of_cells, row);
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
        // **Every move is reported to the host.** The map is rebuilt from scratch each time it
        // opens, so it has no memory of its own — where the highlight was is something only
        // `AppState` can still know a moment later.
        .on_move({
            let emit = emit.clone();
            move |key| {
                if let Ok(id) = key.parse::<u64>() {
                    emit(crate::app::interaction::InteractionIntent::ExposeCursor {
                        pane_id: PaneId(id),
                    });
                }
            }
        })
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
            // **The panel is the frost's tint, not a lid.** The host stamps the blurred frame
            // under this layer (`LayerBackdrop::Frosted`), so the surface here is the background
            // colour at the theme's scrim strength: enough to hold the cards' contrast, thin
            // enough that the session reads behind them as a blurred wash. Painted opaque it hid
            // the blur completely; left at heca's own window colour — translucent by design, for
            // the frosted compositor — the app read straight through it, sharp.
            .panel(
                Surface::column()
                    .background(theme.colors.background.with_alpha(theme.colors.interaction.scrim))
                    .pad_all(Spacing::Md)
                    // **The overflow is the scroll region's.** Enough workspaces and the rows run
                    // past the window; nothing here needs to know how many fit. The region also
                    // follows the cursor, and `Center` is what makes it a map rather than a list:
                    // the pane you are on holds the middle of the screen while the workspaces
                    // around it move past.
                    .child(
                        ScrollRegion::new()
                            .reveal_align(RevealAlign::Center)
                            .overscroll(true)
                            .scrollbars(false)
                            .smooth_scroll(SCROLL_EASE)
                            .grow(1.0)
                            .child(grid),
                    ),
            )
            .open(true),
    )
}


/// Remember where the map's highlight is, against the workspace the pane lives in.
///
/// Per workspace rather than one global slot: the map's vertical axis *is* the workspace list, so
/// walking away from a row and back is the ordinary way to move around it — and a single slot would
/// answer "the last pane I touched anywhere", which is never the pane that row was showing.
pub(crate) fn record_expose_cursor(state: &mut crate::app_state::AppState, pane_id: PaneId) {
    let Some(ws) = crate::app::focus::find_pane_workspace(&state.session, pane_id) else {
        return;
    };
    while state.expose_cursor_per_ws.len() <= ws {
        state.expose_cursor_per_ws.push(None);
    }
    state.expose_cursor_per_ws[ws] = Some(pane_id);
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
    // **Open where the map was left, else on the pane you are standing in.** The remembered
    // highlight wins because it is the more specific answer: it is where *this surface* was when
    // you last used it. With nothing remembered — the first open of a session — the pane you came
    // from is the only sensible place for the map to start.
    let here = state
        .expose_cursor_per_ws
        .get(state.session.active_workspace_idx)
        .copied()
        .flatten()
        .filter(|id| crate::app::focus::find_pane_workspace(&state.session, *id).is_some())
        .or_else(|| {
            state
                .session
                .active_workspace()
                .and_then(|ws| ws.active_pane())
                .map(|p| p.id)
        });
    let root = map(&rows, &theme, emit, here, &state.session.options);
    let was_visible = state.layers.is_visible_named(&name);
    let id = state.layers.add_named(
        name.clone(),
        super::LayerBand::Overlay,
        super::LayerKind::OnDemand,
        // Modal: it takes the keyboard while it is up.
        true,
        // **But it does not cover the content.** `covers_content` is what refuses actions on panes
        // the user cannot see — and in the map you can see them; that is what it is. Declaring
        // coverage here made the surface refuse every act on the pane it exists to let you choose:
        // `FocusPane` was blocked in `Domain::Overlay`, so choosing a card did nothing however the
        // intents were arranged. A dialog covers. A map does not.
        false,
        root,
    );
    // The map floats **over** the session, so the session has to still be there underneath — but
    // legibly out of focus. A flat fill made it a different screen; the blur makes it a lens.
    state.layers.set_backdrop(id, super::LayerBackdrop::Frosted);
    // **Choosing a pane does not make the map vanish.** It dissolves while the app comes back into
    // focus behind it, so the eye follows one picture becoming another instead of being cut to a
    // different screen. A full-screen surface disappearing between two frames reads as a glitch.
    state.layers.set_fade_out(id, FADE_OUT);
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

    /// **The card the cursor is on holds the middle of the window, on both axes.**
    ///
    /// The one you are moving through the map — not the pane the session happens to have focused,
    /// and not the workspace's visible rectangle, both of which were tried and both of which
    /// centred somewhere you were not (Antonio, 2026-08-05).
    ///
    /// Measured off the real layout, because two faults hid behind green tests on the way here:
    /// the strip stretched to the region's full width so there was no slack to position into, and
    /// the strip carrying its wrapper's `grow` left the cards 7px tall.
    #[test]
    fn the_card_under_the_cursor_is_centred_on_both_axes() {
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        // Open on the FIRST pane — the leftmost card of the top row, the case that has to travel
        // furthest and the one that sat in the corner.
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default());
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);
        let mut scene = heca_grid_ui::Scene::new();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme).with_viewport(vp));

        fn cursor_card(n: &dyn Component) -> Option<heca_grid_ui::Rectangle> {
            n.base()
                .children
                .iter()
                .find_map(|c| cursor_card(c.as_ref()))
                .or_else(|| n.wants_visible().then(|| n.base().bounds))
        }
        let card = cursor_card(root.as_ref()).expect("the cursor is on a card");

        let mid = |start: f64, len: f64| start + len / 2.0;
        assert!(
            (mid(card.loc.x, card.size.w) - vp.w / 2.0).abs() < 2.0,
            "centred across the window: {card:?} in {vp:?}",
        );
        assert!(
            (mid(card.loc.y, card.size.h) - vp.h / 2.0).abs() < 2.0,
            "and down it: {card:?} in {vp:?}",
        );
        // A card is a pane-shaped box, not a sliver: the row is the screen at the configured zoom.
        let zoom = LayoutOptions::default().overview_scale;
        assert!(
            (card.size.h - rows[0].viewport_h * zoom).abs() < 24.0,
            "the card fills its row's height ({} at zoom {zoom}): {card:?}",
            rows[0].viewport_h,
        );
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

        // **Stand on a later column**, so `column_x(active)` is not zero and reading `view_offset`
        // raw gives a different answer from `view_pos()`. With the active column at 0 the two
        // coincide and the bug hides — which is exactly how it survived until it was on screen.
        s.workspaces[0].scrolling.active_column_idx = 1;
        let anchor = s.workspaces[0].scrolling.column_x(1);
        assert!(anchor > 0.0, "the second column starts somewhere other than 0: {anchor}");

        let rows = model(&s, |p| p.title.clone());
        let f = &rows[0].floating[0];
        assert_eq!(f.pane_id, PaneId(9));
        assert_eq!(
            f.x,
            40.0 + anchor + 120.0,
            "40 viewport-relative + the viewport's own strip position (column_x(active) + 120)",
        );
        assert_eq!(
            rows[0].viewport.0,
            anchor + 120.0,
            "and the row's visible area is at view_pos(), not at the bare view_offset",
        );
        assert_eq!(f.y, 30.0, "vertical does not scroll");
        assert_eq!((f.w, f.h), (200.0, 150.0), "a float carries its own size");
    }



    /// **Every card in the map is a pane, and nothing else has a surface.** The workspace name was
    /// a fixed gutter column that pushed every strip inwards, so a row stopped lining up with the
    /// screen it is a picture of; niri's overview labels nothing either. This pins the labels that
    /// remain to exactly the pane names.
    #[test]
    fn a_row_carries_no_workspace_label() {
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        let mut root = map(&rows, &theme, emit, None, &LayoutOptions::default());

        // What is *drawn*, not what the tree holds — the question is whether a workspace name ever
        // reaches the screen.
        let viewport = heca_grid_ui::Size::new(1200.0, 800.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), viewport);
        let mut scene = heca_grid_ui::Scene::new();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme).with_viewport(viewport));
        let seen: Vec<String> = scene
            .iter()
            .filter_map(|c| match c {
                heca_grid_ui::DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !seen.iter().any(|t| t == "Editing" || t.starts_with("Workspace ")),
            "no workspace name is drawn; the map showed: {seen:?}",
        );
        assert!(seen.iter().any(|t| t == "a"), "the pane names are still there: {seen:?}");

        // **The whole map lives in the scene's OVERLAY layer**, because `Overlay` paints through
        // `PaintCx::with_overlay`. A host deciding whether to flush the layer pass by asking
        // "did the base layer draw anything" gets `no` and skips it, and `prefix+Tab` shows
        // nothing at all — which is exactly what happened. The renderer asks the layer registry
        // whether a layer is up instead; this pins the fact that made the other question wrong.
        assert!(
            scene.base_layer().is_empty(),
            "the map draws nothing into the base layer — do not gate its flush on that",
        );
        assert!(scene.has_overlay(), "it is all in the overlay layer");
    }

    /// **Activate must reach the grid through the overlay, and choose the card the cursor is on.**
    ///
    /// Enter and Space resolve to `WidgetIntent::Activate` (`[keys.widgets]`), which has to travel
    /// Overlay → Surface → ScrollRegion → CardGrid before anything happens. Every widget on that
    /// path can swallow an intent, and one of them swallowing this is indistinguishable from a
    /// wrong keybinding when you are looking at the app — so it is pinned here.
    #[test]
    fn an_activate_reaches_the_grid_and_chooses_the_card_under_the_cursor() {
        use heca_grid_ui::component::{Event, WidgetIntent as W};
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let sink = seen.clone();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(move |intent| {
            sink.borrow_mut().push(format!("{intent:?}"));
        });
        let mut root = map(&rows, &theme, emit, Some(PaneId(2)), &LayoutOptions::default());

        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Activate));
        let got = seen.borrow().join(" ");
        // Activate-and-leave, the same three parts the sidebar performs: focus the pane, hand the
        // keyboard back, put the surface away. The middle one is invisible until it is missing —
        // without it the right pane is focused and the keyboard stays on the dock, so choosing a
        // card looks like it did nothing.
        assert!(
            got.contains("FocusPaneThenAction") && got.contains("PaneId(2)"),
            "it focuses the card the cursor was on: {got:?}",
        );
        assert!(
            got.contains("UnfocusDock"),
            "and hands the keyboard back to the pane: {got:?}",
        );
        assert!(got.contains("CloseOverlay"), "and closes the map: {got:?}");
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
        let mut root = map(&rows, &theme, emit, None, &LayoutOptions::default());
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
        let got = seen.borrow().join(" ");
        assert!(
            got.contains("CloseOverlay"),
            "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
        );
    }

}
