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
    /// The pane's **resolved** height in layout pixels, read from `Column::pane_sizes` for the same
    /// reason [`ExposeColumn::width`] is read from `column_widths`: the layout already decided it,
    /// and a second calculation here would be a second answer that can disagree.
    ///
    /// This is what makes a stack of unequal panes look unequal. The map divided every column
    /// evenly before, so a pane the user had dragged to twice its neighbour's height — its
    /// `preferred_height` — was drawn as its twin, and the picture disagreed with the screen it is
    /// a picture of.
    pub(crate) height: f64,
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
    /// Is this the workspace's active pane? A float carries the flag itself
    /// (`FloatingPane::is_active`) because it is not in a column and so has no
    /// `active_pane_idx` to be compared against.
    pub(crate) active: bool,
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
                            // The layout's resolved height, which already accounts for
                            // `preferred_height`. A missing entry means the layout has not run
                            // yet, and equal weights are exactly the even split it falls back to.
                            height: col
                                .pane_sizes
                                .get(pane_idx)
                                .map(|s| s.h)
                                .filter(|h| *h > 0.0)
                                .unwrap_or(1.0),
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
                        active: f.is_active,
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
use heca_grid_ui::style::{Align, Length, Spacing, Track};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    CardGrid, Flex, Grid, GridCell, Label, Overlay, RevealAlign, Row, ScrollRegion, Surface,
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
/// How long the map takes to zoom out to its map size when it opens, in seconds.
///
/// niri's own overview animation is in this range; long enough to read as one picture pulling back
/// and short enough that `prefix+Tab` never feels like waiting.
const ZOOM_IN: f32 = 0.2;


/// Dispatch a registry action by name with integer arguments — the one door the map's delete keys
/// go through, so `x`, `X` and `d` are ordinary actions rather than a private path.
type DispatchAction = std::rc::Rc<dyn Fn(&str, &[(&str, i64)])>;

/// What `x` / `X` / `d` do to the card they are pressed on: delete the pane, its column, or the
/// whole workspace.
///
/// One handler for both the tiled cards and the floating ones, because a float answers `x` the same
/// way — it is a pane. `X` and `d` name the column and workspace the card sits in, which the card
/// knows because it was built inside them; nothing is looked up and no cursor is consulted.
///
/// **Which typed characters the map's cards answer to**, one list per thing that can be deleted.
///
/// Plain data, so [`map`] stays a composition that takes plain data and needs no `AppState` (§ 0b)
/// — the lists are read from the built keymaps by [`register`] and handed in. Empty lists are a
/// working state, not a bug: a user who unbinds them gets a map with no delete keys.
#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct ExposeDeleteKeys {
    /// `delete_pane` — the card the cursor is on.
    pub(crate) pane: Vec<String>,
    /// `delete_column` — the column that card sits in.
    pub(crate) column: Vec<String>,
    /// `delete_workspace` — the workspace that column sits in.
    pub(crate) workspace: Vec<String>,
}

/// The surface name the map registers under, and the one a `[[keys.surface]]` entry addresses. One
/// constant so the layer, the config entry and the key lookup cannot drift apart.
pub(crate) const SURFACE: &str = "expose";

/// Written as a `fn` returning the closure so the two call sites share one definition rather than
/// growing a second, drifting copy. The acts are the ordinary registry actions — so a rebind, the
/// palette and RPC all still reach the same three.
///
/// **The letters come from `[[keys.surface]] heca.expose`**, not from this file (F003/P082/T416).
/// They used to be literals here, which made them the one part of the map a user could not rebind
/// while the workspaces dock's identical `x` sat in `keybindings.default.toml`. The *handler* stays
/// on the card, because the card is what knows which pane, column and workspace it is — the host
/// resolves which letters, the widget resolves what they act on.
fn delete_keys(
    keys: ExposeDeleteKeys,
    delete: DispatchAction,
    cursor_to: std::rc::Rc<dyn Fn(PaneId)>,
    pane_id: PaneId,
    next: Option<PaneId>,
    ws_idx: usize,
    col_idx: usize,
) -> impl FnMut(&mut heca_grid_ui::event::EventCx<'_>) + 'static {
    use heca_grid_ui::event::Event;
    move |cx| {
        // **The typed character, not the key** — because `x` and `X` are the same *key*.
        // `Event::Key` deliberately carries no modifiers (a chord is resolved in the keymap, so a
        // widget sees `Char('x')` with or without Shift); the case lives in the text the platform
        // committed, which is exactly what `TextInput` is for — `Shift+2` is `Char('2')` as a key
        // and `"@"` as text. Reading the key made `X` do what `x` does (Antonio, driving,
        // 2026-08-11).
        let Event::TextInput(typed) = cx.event() else {
            return;
        };
        let typed = typed.as_str();
        let matches = |bound: &[String]| bound.iter().any(|k| k == typed);
        let dispatched = if matches(&keys.pane) {
            // **Hand the cursor on before the card goes.** The neighbour was resolved while this
            // row still had both of them; after the delete there is nothing left to ask. Sent
            // first so the rebuild the delete triggers already finds the cursor moved — the events
            // are queued and processed in order.
            if let Some(next) = next {
                cursor_to(next);
            }
            delete("close_pane_by_id", &[("pane_id", pane_id.0 as i64)]);
            true
        } else if matches(&keys.column) {
            // A column is named by the workspace holding it, so both indices go in one intent.
            //
            // The shipped default is `r`, not `X`: `x` and `X` are the same *key* and only differ
            // as typed text, so the pair read as one gesture with a modifier that the key layer
            // cannot see. Three plain letters, one per thing — pane, column, workspace — is what a
            // user can actually keep in their head (Antonio, 2026-08-11).
            delete(
                "delete_column",
                &[("ws_idx", ws_idx as i64), ("col_idx", col_idx as i64)],
            );
            true
        } else if matches(&keys.workspace) {
            delete("delete_workspace", &[("ws_idx", ws_idx as i64)]);
            true
        } else {
            false
        };
        // Claim only what was acted on, so every other key still bubbles to the grid for the nav.
        if dispatched {
            cx.stop_propagation();
        }
    }
}

/// **How far to zoom out so the whole session is on screen at once** (F003/P082/T419).
///
/// An exposé shows everything — that is what the name means — so the zoom is *computed* from the
/// session's own extents rather than read from config. `overview_scale` becomes the **maximum**
/// (so a single pane is not blown up to fill the screen) and `overview_min_card_width` the floor
/// (so thirty panes stay legible and the map scrolls instead of shrinking to slivers).
///
/// **One zoom for the whole map, not one per row** — Antonio, 2026-08-12. It keeps the workspaces
/// comparable in size so the map reads as one picture; fitting each row on its own makes a
/// two-pane workspace's cards huge beside a ten-pane one.
///
/// Pure, and takes the model rather than an `AppState`, so the arithmetic is testable without a
/// window (§ 0b).
fn fit_zoom(
    rows: &[ExposeWorkspace],
    geometry: &heca_core::layout::LayoutOptions,
    available: (f64, f64),
    font_px: f64,
) -> f64 {
    let max_scale = geometry.overview_scale;
    if rows.is_empty() {
        return max_scale;
    }
    // **The room the MAP has, not the room a pane has.** The two are different and the difference
    // is the whole of this bug: the exposé is a full-window overlay, drawn over the sidebars and
    // the bars, while `viewport` is the session's content area with those taken out. Fitting
    // against the latter drew the map at 0.17 where 0.26 fitted — `screen_w=664` on a 995px window
    // (Antonio, driving, with the trace, 2026-08-12), which is the wide empty margin down both
    // sides of every screenshot of it.
    // **Take the chrome out of the room before dividing by the content.** The zoom scales the
    // *panes*; it does not scale the panel's padding or the gaps between cards, which are `Spacing`
    // tokens sized from the font. Dividing the whole window by the sum of the column widths
    // therefore promises a fit that the gaps then break — the widest row ran off the right edge and
    // the last row off the bottom the moment the fit stopped being conservative (Antonio, driving,
    // 2026-08-12).
    // **Every box between the window and the cards, measured.** Four corrections to this
    // arithmetic in a row each left a row off the bottom, because the term that was missing sat
    // *outside* this function: `Overlay` keeps `VIEWPORT_MARGIN` between its panel and the window
    // edge, so a `Pct(1.0)` panel is the window minus 24 on every side. The laid-out chain says it
    // plainly — `1280x800` root → `1232x752` panel → `1210x730` content — 70px of chrome on the
    // vertical, of which this knew about 22 (Antonio, driving, with the geometry trace,
    // 2026-08-12).
    let margin = heca_grid_ui::widgets::overlay::VIEWPORT_MARGIN as f64;
    let pad = Spacing::Md.scale() as f64 * font_px;
    let chrome = 2.0 * (margin + pad);
    let col_gap = Spacing::Sm.scale() as f64 * font_px;
    // The widest row pays for a gap between each pair of its columns.
    let gaps_w = rows
        .iter()
        .map(|ws| (ws.columns.len().saturating_sub(1)) as f64 * col_gap)
        .fold(0.0_f64, f64::max);
    let (avail_w, avail_h) = (
        (available.0 - chrome - gaps_w).max(1.0),
        (available.1 - chrome).max(1.0),
    );

    // **Horizontally: the widest row must fit.** `strip_width` is the sum of the columns, already
    // resolved by the layout. A float can stick out past the end of the strip, and a card off the
    // right edge is exactly what this exists to stop, so its own extent counts too.
    let widest = rows
        .iter()
        .map(|ws| {
            ws.floating
                .iter()
                .map(|f| f.x + f.w)
                .fold(ws.strip_width, f64::max)
        })
        .fold(1.0_f64, f64::max);
    let fit_w = avail_w / widest;

    // **Vertically: every row's own height, summed — not one row's times N.**
    //
    // A row is `ws.viewport_h * zoom` tall, and `viewport_h` is **per workspace**: they are not all
    // the same, which is the whole of why this kept clipping. Taking the first row's height and
    // multiplying made the stack come out at exactly `avail_h` on paper while the real one was
    // taller, so the last row fell off the bottom however the rest of the arithmetic was corrected
    // (Antonio, driving, with the trace, 2026-08-12). The gap matches what `map` actually uses: the
    // **tallest** row's, once between each pair.
    let n = rows.len() as f64;
    let sum_h: f64 = rows.iter().map(|ws| ws.viewport_h.max(1.0)).sum();
    let tallest = rows.iter().map(|ws| ws.viewport_h).fold(1.0_f64, f64::max);
    let stack = sum_h + (n - 1.0).max(0.0) * tallest * geometry.overview_gap;
    let fit_h = avail_h / stack.max(1.0);

    // **The floor is a card width, not a scale.** Columns differ in width, so one scale leaves this
    // session legible and that one a set of slivers; the question actually being asked is "can I
    // still tell what that pane is?".
    let narrowest = rows
        .iter()
        .flat_map(|ws| ws.columns.iter().map(|c| c.width))
        .filter(|w| *w > 0.0)
        .fold(f64::INFINITY, f64::min);
    let floor = match narrowest.is_finite() {
        true => geometry.overview_min_card_width / narrowest,
        // No columns anywhere — nothing to keep legible, so nothing to hold the zoom up.
        false => 0.0,
    };

    // ⚠️ `min` then `max`, never `f64::clamp`: it **panics** when `min > max`, and a small
    // `overview_zoom` beside a large `overview_min_card_width` is exactly that. Legibility wins
    // there — a map you cannot read is worse than one that scrolls.
    let zoom = fit_w.min(fit_h).min(max_scale).max(floor);
    // **Which of the four bounds is binding, in the app's own words.** Read off screenshots this
    // was mis-diagnosed three times running (2026-08-12); the numbers say it in one line.
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "[heca] expose fit: zoom={zoom:.4} (fit_w={fit_w:.4} fit_h={fit_h:.4} \
             ceiling={max_scale:.4} floor={floor:.4}) avail={avail_w:.0}x{avail_h:.0} \
             widest={widest:.0} rows={n}",
        );
        // The model's own numbers. The fit divides by these, so when the drawn strip is a fifth of
        // the room it has, the question is whether the arithmetic is wrong or whether the session
        // really is that shape — and only these say which.
        for ws in rows {
            eprintln!(
                "[heca]   ws{} strip={:.0} viewport={:.0}x{:.0} cols={:?} floats={}",
                ws.ws_idx,
                ws.strip_width,
                ws.viewport.1,
                ws.viewport_h,
                ws.columns.iter().map(|c| c.width as i64).collect::<Vec<_>>(),
                ws.floating.len(),
            );
        }
    }
    zoom
}

/// **Does the map still run past the room it has at `zoom`?**
///
/// Only ever true when the floor ([`overview_min_card_width`]) held the zoom above the fit — the
/// deliberate "I would rather scroll than squint" case. Expressed as the same comparison
/// [`fit_zoom`] makes, so the two can never disagree about whether it fitted: the fit is the
/// largest zoom that does *not* overflow, so overflowing means the zoom in use is bigger than it.
///
/// [`overview_min_card_width`]: heca_core::layout::LayoutOptions::overview_min_card_width
fn overflows(
    rows: &[ExposeWorkspace],
    geometry: &heca_core::layout::LayoutOptions,
    available: (f64, f64),
    font_px: f64,
    zoom: f64,
) -> bool {
    // A floor of zero can never lift the zoom above the fit, so this is the ordinary answer.
    let unfloored = heca_core::layout::LayoutOptions {
        overview_min_card_width: 0.0,
        ..geometry.clone()
    };
    zoom > fit_zoom(rows, &unfloored, available, font_px) + 1e-9
}

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
    keys: &ExposeDeleteKeys,
    // The room the map has to draw in — the window, since it is a full-window overlay. Passed in
    // rather than read from `AppState`, which this composition must never take (§ 0b).
    available: (f64, f64),
) -> Box<dyn Component> {
    // **The map's scale and spacing are the layout's, not this module's.** `overview_scale` and
    // `overview_gap` are `LayoutOptions` fields the user sets in config; hardcoding a zoom here
    // would be a second answer to a question the layout already answers, and an unreachable one.
    let gap_frac = geometry.overview_gap;
    let zoom = fit_zoom(rows, geometry, available, theme.font_size as f64);
    // **One behaviour, two ways in.** Choosing a pane closes the map and focuses it, whether the
    // choice came from the cursor (`CardGrid::on_activate`) or a click on the card itself. Defined
    // once here so the mouse and the keyboard can never drift apart.
    // **Deleting is the card's own key handler, dispatching the actions that already exist.** No
    // new `WidgetIntent`, no new `ActionPolicy` arm, no host-side key match: a handler on the
    // widget, exactly as a click is (AGENTS § 0c). The action carries the confirm with it —
    // `close`, `delete_column` and `delete_workspace` all declare a `ConfirmSpec`
    // (`actions::builtin_confirm_specs`), so the central destructive gate raises the same prompt
    // the sidebar and a header button raise, and this surface neither asks for it nor can skip it.
    //
    // Antonio, 2026-08-11: *"we have an actionRegistry and we MUST always reuse what we have… the
    // only thing i expect is a developer to handle the on_key_up on each pane in the overlay"*.
    let delete: DispatchAction = {
        let emit = emit.clone();
        std::rc::Rc::new(move |action: &str, args: &[(&str, i64)]| {
            let intent = args.iter().fold(heca_view::Intent::new(action), |i, (k, v)| {
                i.arg(*k, heca_view::PropValue::Int(*v))
            });
            emit(crate::app::interaction::InteractionIntent::View(intent));
        })
    };
    // Moving the map's highlight is its own intent — the same one `CardGrid::on_move` sends when
    // you arrow around — so handing the cursor on before a delete goes through the one door that
    // already exists rather than a second way to move it.
    let cursor_to: std::rc::Rc<dyn Fn(PaneId)> = {
        let emit = emit.clone();
        std::rc::Rc::new(move |pane_id: PaneId| {
            emit(crate::app::interaction::InteractionIntent::ExposeCursor { pane_id });
        })
    };
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
    // **Nothing asks to be revealed while the whole session is on screen.**
    //
    // Two reasons, and the second is the one that cost a day. A reveal exists to bring into view
    // something you cannot see, and when everything fits there is no such thing — so following the
    // cursor can only move the picture for no gain. And the region's centring is *driven* by that
    // request: given a target it puts **that** in the middle of the window, which with several rows
    // shoves the others off the edges. The map fitted (727 in a 730 box) and was still pushed 277px
    // up, hiding the first row, because the fourth row was asking to be centred (Antonio, driving,
    // with the geometry trace, 2026-08-12). With no target the region centres the **content**,
    // which is the whole picture, which is what an exposé is.
    //
    // Past the floor the map really does overflow, and then the cursor is worth following again —
    // so the gate is exactly "did it fit", not a flag anyone has to remember.
    let reveal = match overflows(rows, geometry, available, theme.font_size as f64, zoom) {
        // Overflowing: follow the keyboard cursor, never the pointer (`CardGrid::reveal_state`).
        true => grid.reveal_state(),
        // Fitting: nothing is ever off screen, so nothing ever asks.
        false => heca_grid_ui::reactive::signal(false),
    };
    for ws in rows {
        // **Where the cursor goes when a card is deleted: the next one in this row.**
        //
        // Computed here, before anything is deleted, because afterwards the pane is gone and its
        // neighbour can no longer be found from it. The order is the one the eye reads — down each
        // column, then the next column, then the floats — so "next" is the card to the right or
        // below, and the last card falls back to the one before it. Nothing left in the row means
        // nothing to say, and the map's ordinary fallback takes over.
        let order: Vec<PaneId> = ws
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.pane_id))
            .chain(ws.floating.iter().map(|f| f.pane_id))
            .collect();
        let after = |id: PaneId| -> Option<PaneId> {
            let at = order.iter().position(|p| *p == id)?;
            order.get(at + 1).or_else(|| at.checked_sub(1).and_then(|p| order.get(p))).copied()
        };

        let mut columns_of_cells: Vec<Vec<GridCell>> = Vec::new();
        // Columns want visible air between them for the same reason the panes inside one do: at
        // this zoom the real gap scales down to nothing and two neighbouring columns read as a
        // single wide box. A token, so it tracks the font and the size setting rather than being a
        // number that is right at one zoom.
        let mut strip = Flex::row().gap_spacing(Spacing::Sm);
        for col in &ws.columns {
            let mut cells = Vec::new();
            // Panes in a column want visible air between them: at this zoom the real gap scales
            // down to a hairline, and three stacked panes read as one striped box rather than
            // three. A token, so it tracks the font and the size setting like every other gap.
            let mut column = Flex::column().gap_spacing(Spacing::Sm);
            for pane in &col.panes {
                // **The cursor follows the mouse, but the scroll does not.** `CardGrid` reads the
                // hover signal and moves the cursor onto the card you point at; `reveal_when` is
                // what stops that move also scrolling the strip to centre it, which slid the card
                // out from under the pointer and left the mouse in empty space (Antonio, driving,
                // with screenshots, 2026-08-12).
                let card = Row::new().reveal_when(reveal);
                cells.push(
                    GridCell::new(pane.pane_id.0.to_string(), card.nav_state())
                        .hovered(card.hovered()),
                );
                let mut card = card
                    .background(theme.colors.foreground.with_alpha(
                        super::alpha_u8(theme.colors.card_background_alpha),
                    ))
                    .highlight(theme.colors.accent)
                    .radius(theme.colors.control_radius())
                    .pad_all(Spacing::Xs)
                    .active(pane.active)
                    .nav_key(pane_nav_key(pane.pane_id));
                // **The card the cursor is on holds the keyboard**, so its own handlers are what a
                // key reaches — and what it does not take bubbles up to the `CardGrid` for the nav
                // keys, exactly as a browser's listbox option does (AGENTS § 0c). The cursor signal
                // *is* the focus signal, so there is nothing to keep in step.
                card.base_mut().focused = card.nav_state();
                let card = card
                    .on_text_input(delete_keys(
                        keys.clone(),
                        delete.clone(),
                        cursor_to.clone(),
                        pane.pane_id,
                        after(pane.pane_id),
                        ws.ws_idx,
                        col.col_idx,
                    ))
                    // Click to go there. `Row` already provides the hover tint and the press
                    // handling, so the mouse costs one line rather than a second input path.
                    .on_activate({
                        let choose = choose.clone();
                        let id = pane.pane_id;
                        move || choose(id)
                    })
                    .child(Label::new(pane.name.clone()));
                // **A pane's share of its column is its real height**, so a stack the user has
                // dragged out of balance is drawn out of balance. The weights are the resolved
                // heights and the column is the screen at this zoom, so the ratios come out true
                // without this module scaling anything itself.
                column = column.child(share(card, pane.height, true));
            }
            // **A column is its real width times the zoom**, like everything else here — not a
            // share of the row. A share always fills, so one column became the whole row and a
            // workspace holding a single half-screen pane looked exactly like one holding four.
            // The relationship the map has to keep is to the *screen*: half the viewport is half
            // the workspace rectangle, and a strip scrolled past one screen really is wider.
            strip = strip.child(column.width(Length::Px((col.width * zoom) as f32)).shrink(0.0));
            columns_of_cells.push(cells);
        }
        // **A floating pane is drawn over the strip, where it actually sits.** It belongs to no
        // column, so it cannot be a child of one: it is placed at its own rect, in strip
        // coordinates the model already resolved (`ExposeFloating`), scaled by the same zoom as
        // everything else here.
        //
        // The stacking is `Grid`'s, not this module's: two children in the **same cell** overlap,
        // which is what a CSS grid does and what a plugin would reach for. The offset is a margin
        // on a fixed-size box — `docs/widgets.md` → "Placing a widget at an app-chosen rect" — so
        // nothing here measures or positions anything itself. Only wrapped when a workspace
        // actually has floats, so the ordinary row keeps exactly the tree it had.
        //
        // The two axes want opposite things and each is a separate property: `grow` fills the
        // row's **height** (the region's main axis), `align_self` opts out of stretching across its
        // **width** so the strip measures its own columns and the region has real slack to
        // position into. Take the grow away and the cards collapse to a 7px sliver; take the
        // align_self away and the strip fills the region and there is nothing left to centre.
        // Whichever node ends up as the region's child carries both — the strip, or the `Grid`.
        let strip: Box<dyn Component> = if ws.floating.is_empty() {
            Box::new(strip.grow(1.0).align_self(Align::Start))
        } else {
            // ⚠️ **The strip must STRETCH inside its cell, so it must not carry `align_self`
            // here.** That property means "do not stretch me", which is right in the scroll region
            // (it leaves slack to centre into) and fatal in a grid cell: the strip then takes its
            // content height, and a pane's height is a *share* — a zero base plus grow — so every
            // card in the row collapsed to its own padding, a 7px sliver where a pane should be
            // (Antonio, driving, 2026-08-11). Stretching is the grid's job in the region and the
            // cell's job for the strip; the same two properties, one level further out.
            //
            // The tracks are written out for the same reason they would be in CSS: one row taking
            // the full height, one column measuring the strip. Taffy infers the same thing from
            // the implicit track, so this is a statement of intent rather than the fix.
            let mut stacked = Grid::new()
                .columns([Track::Auto])
                .rows([Track::Fr(1.0)])
                .grow(1.0)
                .align_self(Align::Start)
                .cell(strip, 1, 1, 1, 1);
            // **A float is a card like any other, so the cursor must reach it.** The floats become
            // one more column of cells at the end of the row: `l` walks off the last tiled column
            // onto them, and the map centres them like anything else. Drawn over the strip and
            // *navigated* after it — a float belongs to no column, so it cannot be a cell in one,
            // and leaving it out of the grid is what made pane focus work in a row without floats
            // and stop in a row with one (Antonio, driving, 2026-08-11).
            let mut float_cells = Vec::new();
            for float in &ws.floating {
                // Same gate as the tiled cards: a float is a card, and hovering one must not
                // scroll the strip either.
                let card = Row::new().reveal_when(reveal);
                float_cells.push(
                    GridCell::new(float.pane_id.0.to_string(), card.nav_state())
                        .hovered(card.hovered()),
                );
                let mut card = card
                    .background(theme.colors.foreground.with_alpha(super::alpha_u8(
                        theme.colors.card_background_alpha,
                    )))
                    .highlight(theme.colors.accent)
                    .radius(theme.colors.control_radius())
                    .pad_all(Spacing::Xs)
                    .active(float.active)
                    .nav_key(pane_nav_key(float.pane_id));
                card.base_mut().focused = card.nav_state();
                // A float has no column, so `X` names the last one — it is still the workspace's
                // column the user is looking at. `x` and `d` mean exactly what they do elsewhere.
                let card = card
                    .on_text_input(delete_keys(
                        keys.clone(),
                        delete.clone(),
                        cursor_to.clone(),
                        float.pane_id,
                        after(float.pane_id),
                        ws.ws_idx,
                        ws.columns.len().saturating_sub(1),
                    ))
                    .on_activate({
                        let choose = choose.clone();
                        let id = float.pane_id;
                        move || choose(id)
                    })
                    .child(Label::new(float.name.clone()))
                    // Its own rect at this zoom: the size it really has, at the place it really is.
                    .width(Length::Px((float.w * zoom) as f32))
                    .height(Length::Px((float.h * zoom) as f32))
                    .margin_left((float.x * zoom) as f32)
                    .margin_top((float.y * zoom) as f32);
                stacked = stacked.cell(card, 1, 1, 1, 1);
            }
            columns_of_cells.push(float_cells);
            Box::new(stacked)
        };

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
                    .child_boxed(strip),
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
    // …and which row that is, so a rebuild restores the cursor where it stands rather than at the
    // active workspace's remembered one. See `AppState::expose_cursor_ws`.
    state.expose_cursor_ws = Some(ws);
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
    // **The map's own id, so its intents say the map made them.** Stamped `Keyboard` before, which
    // was indistinguishable from `prefix+j` typed at the session behind the map — and once the
    // active context started refusing the app's bindings, that sameness would have refused the
    // map's own deletes with them (F003/P082/T416). A re-registration keeps the layer's id, so the
    // one already registered under this name is the one to name.
    let id = state.layers.slot_for_name(&name);
    let emit = super::layer_emitter(&state.event_proxy, id);
    // **Open where the map was left, else on the pane you are standing in.** The remembered
    // highlight wins because it is the more specific answer: it is where *this surface* was when
    // you last used it. With nothing remembered — the first open of a session — the pane you came
    // from is the only sensible place for the map to start.
    //
    // **While the map is already up, "where it was left" is the row the cursor is on** — not the
    // active workspace's memory. A rebuild happens whenever the session changes underneath, and
    // reading the active workspace's slot then moved the highlight to the active row: delete a
    // pane while standing in another row and the map jumped workspace (Antonio, driving,
    // 2026-08-11).
    let already_up = state.layers.is_visible_named(&name);
    let row = state
        .expose_cursor_ws
        .filter(|_| already_up)
        .unwrap_or(state.session.active_workspace_idx);
    let here = state
        .expose_cursor_per_ws
        .get(row)
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
    // **The cards' delete letters, from `[[keys.surface]] heca.expose`.** Read through
    // `ActionShortcuts`, which is built from the *resolved* keymaps at load and at every
    // `prefix+Shift+r` — so a rebind reaches the map without this path knowing anything about the
    // config file, and without a second reader of it.
    let keys = ExposeDeleteKeys {
        pane: state.action_shortcuts.in_surface(&name, "delete_pane").to_vec(),
        column: state.action_shortcuts.in_surface(&name, "delete_column").to_vec(),
        workspace: state
            .action_shortcuts
            .in_surface(&name, "delete_workspace")
            .to_vec(),
    };
    // **The room the map has is the window**, not the session's content area: the overlay is drawn
    // over the sidebars and the bars. Read here because `map` takes plain data and no `AppState`.
    let available = {
        let phys = state.window.inner_size();
        let s = state.scale_factor;
        (phys.width as f64 / s, phys.height as f64 / s)
    };
    let root = map(&rows, &theme, emit, here, &state.session.options, &keys, available);
    let was_visible = state.layers.is_visible_named(&name);
    let id = state.layers.add_named(
        id,
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
    // ⚠️ **The dissolve is NOT delayed behind the zoom.** Tried 2026-08-11 so the shrink would
    // play before the surface went; the layer is retired when the *fade* finishes, so waiting left
    // the cards on screen after the map had gone (Antonio, driving). If the two are ever to be
    // sequenced, the layer's lifetime has to follow the whole exit, not the fade alone.
    state.layers.set_fade_out(id, FADE_OUT);
    // **It opens by pulling back, the way niri's overview does** — the same session seen from
    // further away, rather than a different picture arriving. Antonio: *"The animation in niri is
    // zoom-in/out not fade."*
    //
    // How far back is `[settings] overview_zoom_from`, not a constant here: `1 / overview_zoom`
    // would start the cards at exactly life size, which is the truest reading and overshoots — at
    // 2× the outer rows begin off-screen and rush in. The setting defaults to a gentler 1.3, and a
    // user who wants the literal reading sets 2.0.
    state
        .layers
        .set_zoom(id, ZOOM_IN, state.session.options.overview_zoom_from as f32);
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

    /// **The other half of the rule.** Once the strip is wider than the window there really is
    /// something off-screen, and the map has to bring the cursor card to the middle — the case the
    /// centring was written for. Guarded separately so "centre the content when it fits" can never
    /// be satisfied by centring the content always.
    #[test]
    fn an_overflowing_strip_still_centres_the_cursor() {
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        // Wide enough that even at the floor the strip cannot fit: 40 columns of a quarter-screen.
        let mut rows = fitting(40, 1900.0, 1200.0);
        rows[0].columns = rows[0]
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ExposeColumn {
                col_idx: i,
                width: c.width,
                panes: vec![ExposePane {
                    pane_id: PaneId(i as u64 + 1),
                    name: format!("p{i}"),
                    active: i == 0,
                    height: 1200.0,
                }],
            })
            .collect();
        let last = PaneId(rows[0].columns.len() as u64);

        // A floor is what makes a strip overflow now that the fit always succeeds — which is
        // exactly the setting's purpose: someone who would rather scroll than squint.
        let opts = LayoutOptions { overview_min_card_width: 300.0, ..LayoutOptions::default() };
        let mut root = map(&rows, &theme, emit, Some(last), &opts, &shipped_keys(), (1900.0, 1200.0));
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);

        let strip = strip_bounds(root.as_ref()).expect("the cards are laid out");
        assert!(
            strip.size.w > vp.w,
            "the fixture must actually overflow, or this proves nothing: {strip:?}",
        );
        let card = card_of(root.as_ref(), &pane_nav_key(last)).expect("the cursor's card");
        assert!(
            (card.loc.x + card.size.w / 2.0 - vp.w / 2.0).abs() < 2.0,
            "the last card is brought to the middle: {card:?} in {vp:?}",
        );
    }

    /// The width the map really draws at `zoom` — the columns **plus** the gaps between them and
    /// the panel's padding, which the zoom does not scale. Asserting against this rather than
    /// against a bare ratio is what catches a fit that promises what the chrome then breaks.
    fn drawn_w(rows: &[ExposeWorkspace], zoom: f64, font_px: f64) -> f64 {
        let pad = Spacing::Md.scale() as f64 * font_px
            + heca_grid_ui::widgets::overlay::VIEWPORT_MARGIN as f64;
        let gap = Spacing::Sm.scale() as f64 * font_px;
        rows.iter()
            .map(|ws| {
                ws.strip_width * zoom + (ws.columns.len().saturating_sub(1)) as f64 * gap
            })
            .fold(0.0_f64, f64::max)
            + 2.0 * pad
    }

    /// The height the map really draws at `zoom`: the rows, their gaps, and the panel's padding.
    fn drawn_h(rows: &[ExposeWorkspace], zoom: f64, gap_frac: f64, font_px: f64) -> f64 {
        let pad = Spacing::Md.scale() as f64 * font_px
            + heca_grid_ui::widgets::overlay::VIEWPORT_MARGIN as f64;
        let n = rows.len() as f64;
        let screen_h = rows[0].viewport_h;
        zoom * screen_h * (n + (n - 1.0) * gap_frac) + 2.0 * pad
    }

    /// One workspace `screen_w` wide holding `cols` equal columns, on a `screen_w`×`screen_h`
    /// screen. Four columns is exactly one screenful.
    fn fitting(cols: usize, screen_w: f64, screen_h: f64) -> Vec<ExposeWorkspace> {
        let width = screen_w / 4.0;
        vec![ExposeWorkspace {
            ws_idx: 0,
            name: "w".into(),
            active: true,
            columns: (0..cols)
                .map(|col_idx| ExposeColumn { col_idx, width, panes: Vec::new() })
                .collect(),
            floating: Vec::new(),
            viewport: (0.0, screen_w),
            viewport_h: screen_h,
            strip_width: width * cols as f64,
        }]
    }

    /// **An exposé shows everything at once** — that is what the name means. Four columns fill one
    /// screen, so the ceiling decides; sixteen are four screens wide, and the map zooms out until
    /// they are all on screen rather than running off the edge (Antonio, 2026-08-12, with
    /// screenshots of cards outside the view).
    #[test]
    fn the_map_zooms_out_until_the_widest_row_fits() {
        // The floor is measured on its own below; here it is lowered out of the way so this
        // asserts the FIT and nothing else. At the shipped 140px it would bind first and the test
        // would be green for the wrong reason.
        let opts = LayoutOptions { overview_min_card_width: 1.0, ..LayoutOptions::default() };
        let four = fit_zoom(&fitting(4, 1600.0, 1000.0), &opts, (1600.0, 1000.0), 14.0);
        assert_eq!(four, opts.overview_scale, "one screen wide — the ceiling decides");

        let wide = fitting(16, 1600.0, 1000.0);
        let sixteen = fit_zoom(&wide, &opts, (1600.0, 1000.0), 14.0);
        assert!(sixteen < opts.overview_scale, "four screens wide zooms out: {sixteen}");
        assert!(
            drawn_w(&wide, sixteen, 14.0) <= 1600.0 + 1e-6,
            "and what is DRAWN fits the window, gaps and padding included: {}",
            drawn_w(&wide, sixteen, 14.0),
        );
    }

    /// The ceiling still holds: a nearly empty session is **not** blown up to fill the window.
    #[test]
    fn a_small_session_is_capped_rather_than_magnified() {
        let opts = LayoutOptions::default();
        let one = fit_zoom(&fitting(1, 1600.0, 1000.0), &opts, (1600.0, 1000.0), 14.0);
        assert_eq!(
            one, opts.overview_scale,
            "a quarter-screen strip could fit at 4x; overview_zoom is the maximum",
        );
    }

    /// **The default is no floor at all** — the map fits the whole session, which is what an
    /// exposé is for. At 140px it bound on an ordinary three-workspace session and pushed two of
    /// the three rows off the bottom (Antonio, with a screenshot, 2026-08-12), so the shipped
    /// default is asserted here rather than left to whatever the constant happens to be.
    #[test]
    fn there_is_no_minimum_card_width_by_default() {
        let opts = LayoutOptions::default();
        assert_eq!(opts.overview_min_card_width, 0.0);
        let rows = fitting(200, 1600.0, 1000.0);
        let zoom = fit_zoom(&rows, &opts, (1600.0, 1000.0), 14.0);
        assert!(zoom > 0.0, "still a usable zoom, however small");
        assert!(
            drawn_w(&rows, zoom, 14.0) <= 1600.0 + 1e-6,
            "fitted however wide the strip is: {}",
            drawn_w(&rows, zoom, 14.0),
        );
    }

    /// And the floor, when a user asks for one: past it the cards stop shrinking and the map
    /// scrolls instead, for someone who would rather scroll than squint.
    #[test]
    fn cards_stop_shrinking_at_the_minimum_width() {
        let opts = LayoutOptions { overview_min_card_width: 140.0, ..LayoutOptions::default() };
        let rows = fitting(200, 1600.0, 1000.0);
        let column_w = rows[0].columns[0].width;
        let zoom = fit_zoom(&rows, &opts, (1600.0, 1000.0), 14.0);
        assert!(
            (zoom * column_w - opts.overview_min_card_width).abs() < 1e-9,
            "held at the floor ({column_w} × {zoom}), not fitted to the whole strip",
        );
    }

    /// **`min` then `max`, never `f64::clamp`** — which panics when `min > max`. A small
    /// `overview_zoom` beside a large `overview_min_card_width` is exactly that pair, and
    /// legibility is the one that wins.
    #[test]
    fn a_floor_above_the_ceiling_does_not_panic() {
        let opts = LayoutOptions {
            overview_scale: 0.05,
            overview_min_card_width: 900.0,
            ..LayoutOptions::default()
        };
        let zoom = fit_zoom(&fitting(4, 1600.0, 1000.0), &opts, (1600.0, 1000.0), 14.0);
        assert!(zoom > opts.overview_scale, "the floor wins: {zoom}");
    }

    /// Several workspaces share **one** zoom (Antonio's choice, 2026-08-12), and they have to fit
    /// stacked with their gaps — so the vertical fit is what decides once there are a few.
    #[test]
    fn the_rows_fit_stacked_with_their_gaps() {
        let opts = LayoutOptions { overview_min_card_width: 1.0, ..LayoutOptions::default() };
        let mut rows = fitting(1, 1600.0, 1000.0);
        for ws_idx in 1..4 {
            let mut next = rows[0].clone();
            next.ws_idx = ws_idx;
            rows.push(next);
        }
        let zoom = fit_zoom(&rows, &opts, (1600.0, 1000.0), 14.0);
        assert!(
            drawn_h(&rows, zoom, opts.overview_gap, 14.0) <= 1000.0 + 1e-6,
            "four rows and three gaps fit the height: {}",
            drawn_h(&rows, zoom, opts.overview_gap, 14.0),
        );
    }

    /// **Rows are not all the same height**, and taking the first one's and multiplying is what
    /// made the stack come out at exactly the available height on paper while the real one was
    /// taller — so the last row fell off the bottom however the rest of the arithmetic was
    /// corrected (Antonio, driving, with the trace, 2026-08-12). Every fixture above uses equal
    /// rows, which is why none of them could catch it.
    #[test]
    fn rows_of_different_heights_are_summed_not_multiplied() {
        let opts = LayoutOptions::default();
        let mut rows = fitting(1, 1600.0, 400.0);
        for (ws_idx, h) in [800.0, 1200.0].into_iter().enumerate() {
            let mut next = rows[0].clone();
            next.ws_idx = ws_idx + 1;
            next.viewport_h = h;
            rows.push(next);
        }
        let zoom = fit_zoom(&rows, &opts, (1600.0, 1000.0), 14.0);

        let pad = Spacing::Md.scale() as f64 * 14.0
            + heca_grid_ui::widgets::overlay::VIEWPORT_MARGIN as f64;
        let sum: f64 = rows.iter().map(|ws| ws.viewport_h).sum();
        let tallest = rows.iter().map(|ws| ws.viewport_h).fold(0.0_f64, f64::max);
        let drawn = zoom * (sum + 2.0 * tallest * opts.overview_gap) + 2.0 * pad;
        assert!(
            drawn <= 1000.0 + 1e-6,
            "the three unequal rows and their gaps fit the height: {drawn}",
        );
        // And it is the sum that binds, not three times the first (which would be far smaller).
        assert!(
            zoom < 1000.0 / (3.0 * rows[0].viewport_h),
            "a fit taken from the first row alone would have overshot: {zoom}",
        );
    }

    /// A float can stick out past the end of the strip, and a card off the right edge is exactly
    /// what this exists to stop — so its extent counts towards the fit too.
    #[test]
    fn a_float_past_the_end_of_the_strip_still_has_to_fit() {
        let opts = LayoutOptions::default();
        let mut rows = fitting(4, 1600.0, 1000.0);
        let strip = rows[0].strip_width;
        rows[0].floating.push(ExposeFloating {
            pane_id: PaneId(9),
            name: "f".into(),
            active: false,
            x: strip,
            y: 0.0,
            w: strip,
            h: 100.0,
        });
        let zoom = fit_zoom(&rows, &opts, (1600.0, 1000.0), 14.0);
        let without = fit_zoom(&fitting(4, 1600.0, 1000.0), &opts, (1600.0, 1000.0), 14.0);
        assert!(
            zoom < without,
            "the float doubles the row's extent, so the map zooms out further than the strip \
             alone would ({zoom} vs {without})",
        );
    }

    /// The delete letters **as the bundled defaults bind them**, so these tests exercise what a
    /// user actually gets. Held to the real file by
    /// [`the_shipped_defaults_bind_the_maps_delete_keys`], which fails if the two drift.
    fn shipped_keys() -> ExposeDeleteKeys {
        ExposeDeleteKeys {
            pane: vec!["x".into()],
            column: vec!["r".into()],
            workspace: vec!["d".into()],
        }
    }

    /// **The letters reach the cards** — the whole path, from the shipped file through the built
    /// keymaps to the list [`register`] hands the cards.
    ///
    /// This is the test that was missing, and the gap was not academic: the entry parsed, the
    /// keymap built, and the lookup still came back **empty**, because the index records the
    /// qualified id (`heca.expose.delete_pane`) while the lookup asked for the short name. The
    /// letters silently did nothing and `x` fell through to the host, which reported an action it
    /// had never heard of (Antonio, driving, 2026-08-12). Asserting the entry alone — which the
    /// test below does — could not see it: both ends were right and the join was wrong.
    #[test]
    fn the_shipped_delete_letters_reach_the_cards() {
        let mut index = crate::keymap::BindingIndex::new();
        crate::app::registry::build_component_keymaps(
            &heca_config::theme::Config::default(),
            &mut crate::app::conflicts::Conflicts::default(),
            &mut index,
        );
        let shortcuts = super::super::ActionShortcuts::from_index(
            &index,
            crate::shortcut::KeyStyle::default(),
        );
        let name = super::super::layers::layer_name(super::super::layers::HOST_OWNER, SURFACE)
            .expect("a valid layer name");
        let found = ExposeDeleteKeys {
            pane: shortcuts.in_surface(&name, "delete_pane").to_vec(),
            column: shortcuts.in_surface(&name, "delete_column").to_vec(),
            workspace: shortcuts.in_surface(&name, "delete_workspace").to_vec(),
        };
        assert_eq!(
            found,
            shipped_keys(),
            "the map's cards must actually receive the letters the defaults bind",
        );
    }

    /// **The letters are config, and this is the file they live in.** They were literals in this
    /// module until F003/P082/T416; the tests above would happily keep passing with a default file
    /// that bound nothing, and the map would then have no delete keys at all. So the shipped
    /// `[[keys.surface]]` entry is asserted directly.
    #[test]
    fn the_shipped_defaults_bind_the_maps_delete_keys() {
        let keys = heca_config::theme::KeysConfig::default();
        let name = super::super::layers::layer_name(super::super::layers::HOST_OWNER, SURFACE)
            .expect("a valid layer name");
        let entry = keys
            .surfaces()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("keybindings.default.toml must ship a surface layer for {name}"));
        for (action, expected) in [
            ("delete_pane", "x"),
            ("delete_column", "r"),
            ("delete_workspace", "d"),
        ] {
            let bound = entry
                .bindings
                .get(action)
                .unwrap_or_else(|| panic!("{name} must bind {action}"));
            assert_eq!(bound.keys(), vec![expected.to_string()], "{action}");
        }
    }

    fn session() -> Session {
        let viewport = Size::new(800.0, 600.0);
        let mut s = Session::new(SessionId(0), viewport, 1.0, LayoutOptions::default());
        s.workspaces[0].name = Some("Editing".to_string());
        s.add_pane(Pane::new(PaneId(1), "a"), None, false);
        s.add_pane(Pane::new(PaneId(2), "b"), None, false);
        s.add_workspace(Rectangle::from_size(viewport));
        s
    }

    /// **A session that fits is centred as a whole; only an overflowing one follows the cursor.**
    ///
    /// Changed 2026-08-12 (F003/P082/T419). The rule used to be "the cursor card holds the middle,
    /// always", which is right when there is something off-screen to follow — the case it was
    /// written for, where the map centred the pane you had *left* rather than the one you were on
    /// (Antonio, 2026-08-05) — and wrong when everything already fits: pulling one card to the
    /// middle can then only push its neighbours off the edges. The map showed a screen of empty
    /// space on the left with the last column clipped on the right (Antonio, with a screenshot,
    /// 2026-08-12). The overflow half of the rule is asserted by
    /// [`an_overflowing_strip_still_centres_the_cursor`].
    ///
    /// Measured off the real layout, because two faults hid behind green tests on the way here:
    /// the strip stretched to the region's full width so there was no slack to position into, and
    /// the strip carrying its wrapper's `grow` left the cards 7px tall.
    #[test]
    fn a_session_that_fits_is_centred_as_a_whole() {
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        // Open on the FIRST pane — the leftmost card of the top row, the case that has to travel
        // furthest and the one that sat in the corner.
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);
        let mut scene = heca_grid_ui::Scene::new();
        root.paint(&mut heca_grid_ui::PaintCx::new(&mut scene, &theme).with_viewport(vp));

        // Found by key, not by "who wants to be visible": **nothing** asks while the map fits,
        // which is the point — a request to be revealed is what drags the picture off-centre.
        let card = card_of(root.as_ref(), &pane_nav_key(PaneId(1)))
            .expect("the cursor's card is laid out");

        // The whole strip — not the cursor card — sits in the middle, so nothing is pushed off an
        // edge. Read off the row that holds the cards rather than the card itself.
        let strip = strip_bounds(root.as_ref()).expect("the cards are laid out");
        let mid = |start: f64, len: f64| start + len / 2.0;
        assert!(
            (mid(strip.loc.x, strip.size.w) - vp.w / 2.0).abs() < 2.0,
            "the strip is centred across the window: {strip:?} in {vp:?}",
        );
        assert!(
            strip.loc.x >= -1.0 && strip.loc.x + strip.size.w <= vp.w + 1.0,
            "and nothing hangs off either edge: {strip:?} in {vp:?}",
        );
        // A card is a pane-shaped box, not a sliver: the row is the screen at the **resolved**
        // zoom. Asked for rather than hardcoded to `overview_scale`, which is only the ceiling now
        // — a test holding the old constant would keep passing while the map fitted nothing.
        let zoom = fit_zoom(&rows, &LayoutOptions::default(), (1900.0, 1200.0), 14.0);
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

    /// **A stack the user dragged out of balance is drawn out of balance** (T327, item 3).
    ///
    /// The map divided every column evenly, so a pane the user had resized to twice its
    /// neighbour's height was drawn as its twin — the picture disagreeing with the screen it is a
    /// picture of. Measured off the real layout, not the model alone: the model carrying the right
    /// number is worth nothing if the share does not reach taffy.
    #[test]
    fn stacked_panes_are_drawn_at_the_heights_the_layout_gave_them() {
        let mut s = session();
        // Two panes stacked in one column, then the boundary between them dragged down so the
        // upper is decidedly the taller of the two.
        let ws = &mut s.workspaces[0];
        ws.scrolling.add_pane_to_column(0, None, Pane::new(PaneId(3), "c"), false);
        let (h, gaps) = (ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
        ws.scrolling.columns[0].move_pane_boundary(0, 120.0, h, gaps);

        let rows = model(&s, |p| p.title.clone());
        let stacked = &rows[0].columns[0].panes;
        assert_eq!(stacked.len(), 2, "two panes share the first column");
        assert!(
            stacked[0].height > stacked[1].height + 100.0,
            "the model carries the resolved heights, not an even split: {stacked:?}",
        );

        // …and the drawn cards keep that ratio.
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        heca_grid_ui::LayoutEngine::new()
            .compute(root.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));

        let top = card_of(root.as_ref(), &pane_nav_key(PaneId(1))).expect("the first pane's card");
        let bottom = card_of(root.as_ref(), &pane_nav_key(PaneId(3))).expect("the second's");
        let model_ratio = stacked[0].height / stacked[1].height;
        let drawn_ratio = top.size.h / bottom.size.h;
        assert!(
            (drawn_ratio - model_ratio).abs() < 0.15,
            "the cards keep the real ratio ({model_ratio:.2}), drawn {drawn_ratio:.2}: \
             {top:?} over {bottom:?}",
        );
    }

    /// The card a pane's `nav_key` names, wherever it sits in the tree.
    fn card_of(n: &dyn Component, key: &str) -> Option<heca_grid_ui::Rectangle> {
        if n.base().nav_key.as_deref() == Some(key) {
            return Some(n.base().bounds);
        }
        n.base().children.iter().find_map(|c| card_of(c.as_ref(), key))
    }

    /// The rectangle every card in the map falls inside — what has to sit in the middle of the
    /// window when the session fits, rather than any one card.
    fn strip_bounds(root: &dyn Component) -> Option<heca_grid_ui::Rectangle> {
        fn walk(n: &dyn Component, acc: &mut Option<heca_grid_ui::Rectangle>) {
            if n.base().nav_key.is_some() {
                let b = n.base().bounds;
                *acc = Some(match acc.take() {
                    None => b,
                    Some(a) => {
                        let x0 = a.loc.x.min(b.loc.x);
                        let y0 = a.loc.y.min(b.loc.y);
                        let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
                        let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
                        heca_grid_ui::Rectangle::new(
                            heca_grid_ui::Point::new(x0, y0),
                            heca_grid_ui::Size::new(x1 - x0, y1 - y0),
                        )
                    }
                });
            }
            for c in &n.base().children {
                walk(c.as_ref(), acc);
            }
        }
        let mut acc = None;
        walk(root, &mut acc);
        acc
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

    /// **…and it is drawn there** (T327, item 2). The model carried the strip coordinates from the
    /// start; nothing put a box at them, so a floating pane was simply missing from the map.
    ///
    /// Placement is `Grid`'s — the strip and the float share one cell and overlap, the way a CSS
    /// grid does — with the offset as a margin on a fixed-size box. So this asserts the *drawn*
    /// rect against the model, which is the only thing that can tell the two apart.
    #[test]
    fn a_floating_pane_is_drawn_over_the_strip_at_its_own_rect() {
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

        let rows = model(&s, |p| p.title.clone());
        let f = rows[0].floating[0].clone();
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        heca_grid_ui::LayoutEngine::new()
            .compute(root.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));

        let drawn = card_of(root.as_ref(), &pane_nav_key(PaneId(9)))
            .expect("the floating pane has a card in the map");
        let tiled = card_of(root.as_ref(), &pane_nav_key(PaneId(1)))
            .expect("and the tiled panes still have theirs");

        let zoom = fit_zoom(&rows, &LayoutOptions::default(), (1900.0, 1200.0), 14.0);
        assert!(
            (drawn.size.w - f.w * zoom).abs() < 1.0 && (drawn.size.h - f.h * zoom).abs() < 1.0,
            "the float is its real size at this zoom ({}×{} × {zoom}): {drawn:?}",
            f.w,
            f.h,
        );
        // Its offset from the strip's own origin is its strip position, scaled. Compared against
        // the first tiled card rather than the window, so the assertion does not depend on where
        // the row happens to be scrolled or centred.
        assert!(
            (drawn.loc.x - tiled.loc.x - f.x * zoom).abs() < 2.0,
            "offset across the strip by its scaled x ({} × {zoom}): {drawn:?} vs {tiled:?}",
            f.x,
        );
        assert!(
            (drawn.loc.y - tiled.loc.y - f.y * zoom).abs() < 2.0,
            "and down it by its scaled y ({} × {zoom}): {drawn:?} vs {tiled:?}",
            f.y,
        );

        // It overlaps the strip rather than displacing it — the tiled cards are exactly where they
        // would be with no float in the workspace at all.
        let without = {
            let s2 = session();
            let rows2 = model(&s2, |p| p.title.clone());
            let emit2: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
            let mut root2 = map(&rows2, &theme, emit2, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
            heca_grid_ui::LayoutEngine::new()
                .compute(root2.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));
            card_of(root2.as_ref(), &pane_nav_key(PaneId(1))).expect("its card")
        };
        // **Every side of it**, not just the axis the float moves along. Checking x and width
        // alone is how the first version of this shipped a row whose cards had collapsed to a 4px
        // sliver: the stacking wrapper broke the *height* chain — a grid's implicit track is
        // content-sized and a pane's height is a share of its column — and the test was looking
        // the other way (Antonio, driving, 2026-08-11).
        assert!(
            (tiled.loc.x - without.loc.x).abs() < 2.0
                && (tiled.loc.y - without.loc.y).abs() < 2.0
                && (tiled.size.w - without.size.w).abs() < 2.0
                && (tiled.size.h - without.size.h).abs() < 2.0,
            "a float changes nothing about the tiled cards: {tiled:?} vs {without:?}",
        );
        // And the card is still a pane-shaped box rather than a sliver.
        assert!(
            tiled.size.h > 100.0,
            "the tiled card keeps its height beside a float: {tiled:?}",
        );
    }

    /// **`x` / `X` / `d` are the card's own key handler, dispatching actions that already exist**
    /// (T327, item 4).
    ///
    /// The whole chain in one test, because every link of it was a design question: the card the
    /// cursor is on holds focus, so the key reaches *it*; it dispatches an ordinary registry action
    /// by name with the ids it already knows; and anything it does not take still bubbles to the
    /// grid, so the nav keys keep working. No `WidgetIntent`, no `ActionPolicy` arm, no host-side
    /// key match — and the destructive confirm comes from the action, not from here.
    #[test]
    fn the_delete_keys_dispatch_the_actions_the_registry_already_has() {
        use heca_grid_ui::event::{Event, GridKey, WidgetIntent};
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let emit: crate::chrome::ChromeIntentEmitter = {
            let seen = seen.clone();
            std::rc::Rc::new(move |intent| seen.borrow_mut().push(intent))
        };
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);

        let press = |root: &mut Box<dyn Component>, c: char| {
            // The way a host delivers a keystroke: the committed text, then the key. The delete
            // gesture reads the text, because `x` and `X` are the same key.
            heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput(c.to_string()));
            for pressed in [true, false] {
                heca_grid_ui::dispatch(
                    root.as_mut(),
                    &Event::Key { key: GridKey::Char(c.to_ascii_lowercase()), pressed },
                );
            }
        };
        let intents = |seen: &std::rc::Rc<std::cell::RefCell<Vec<_>>>| -> Vec<(String, Vec<(String, i64)>)> {
            seen.borrow()
                .iter()
                .filter_map(|i| match i {
                    crate::app::interaction::InteractionIntent::View(vi) => Some((
                        vi.action.clone(),
                        {
                            let mut args: Vec<(String, i64)> = vi
                                .args
                                .iter()
                                .filter_map(|(k, v)| match v {
                                    heca_view::PropValue::Int(n) => Some((k.clone(), *n)),
                                    _ => None,
                                })
                                .collect();
                            args.sort();
                            args
                        },
                    )),
                    _ => None,
                })
                .collect()
        };

        // The cursor opens on pane 1, so `x` closes pane 1 — not "the focused pane", not a lookup.
        press(&mut root, 'x');
        assert_eq!(
            intents(&seen),
            vec![("close_pane_by_id".to_string(), vec![("pane_id".to_string(), 1)])],
            "the card under the cursor deleted itself, by id",
        );

        // A key the card does not claim still bubbles to the grid: the cursor moves.
        seen.borrow_mut().clear();
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);
        press(&mut root, 'r');
        assert_eq!(
            intents(&seen),
            vec![(
                "delete_column".to_string(),
                vec![("col_idx".to_string(), 1), ("ws_idx".to_string(), 0)],
            )],
            "nav still reached the grid, and r names the column the cursor is now in",
        );

        // And `d` names the workspace that row is.
        seen.borrow_mut().clear();
        press(&mut root, 'd');
        assert_eq!(
            intents(&seen),
            vec![("delete_workspace".to_string(), vec![("ws_idx".to_string(), 0)])],
        );

        // An unrelated key is nobody's: it must not turn into an action.
        seen.borrow_mut().clear();
        press(&mut root, 'z');
        assert!(intents(&seen).is_empty(), "an unclaimed key dispatches nothing");

        // **Three plain letters, one per thing.** `X` was the column at first and read as the same
        // gesture as `x`: they are the same *key*, differing only as typed text, so nothing about
        // the pair said "a different target" (Antonio, 2026-08-11).
        seen.borrow_mut().clear();
        heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("r".to_string()));
        let after_upper = intents(&seen);
        seen.borrow_mut().clear();
        heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".to_string()));
        let after_lower = intents(&seen);
        assert_eq!(
            after_upper.first().map(|(a, _)| a.as_str()),
            Some("delete_column"),
            "r deletes the column",
        );
        assert_eq!(
            after_lower.first().map(|(a, _)| a.as_str()),
            Some("close_pane_by_id"),
            "…and x still deletes the pane",
        );
    }

    /// **The focused card follows the cursor, and only ONE card is ever focused.**
    ///
    /// `focus_path` takes the deepest widget carrying the flag, so a second card left holding it
    /// silently keeps the keyboard: the cursor moves, the grid reports the move handled, and every
    /// key still goes to the card it left behind (Antonio, driving, 2026-08-11 — the cursor stuck
    /// on one card while `ctrl+l` kept answering `Yes`).
    #[test]
    fn exactly_one_card_holds_the_keyboard_as_the_cursor_moves() {
        use heca_grid_ui::event::{Event, WidgetIntent};
        use heca_grid_ui::reactive::SignalGet;
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let emit: crate::chrome::ChromeIntentEmitter = std::rc::Rc::new(|_| {});
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);

        fn focused_keys(n: &dyn Component, out: &mut Vec<String>) {
            if n.base().focused.get_untracked()
                && let Some(k) = n.base().nav_key.as_deref()
            {
                out.push(k.to_string());
            }
            for c in n.base().children.iter() {
                focused_keys(c.as_ref(), out);
            }
        }

        let mut seen = Vec::new();
        for step in 0..4 {
            let mut focused = Vec::new();
            focused_keys(root.as_ref(), &mut focused);
            assert!(
                focused.len() <= 1,
                "step {step}: {} cards hold the keyboard at once: {focused:?}",
                focused.len(),
            );
            seen.push(focused.first().cloned());
            heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
            heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);
        }
        assert!(
            seen.iter().filter(|s| s.is_some()).collect::<std::collections::HashSet<_>>().len() > 1,
            "the focused card must CHANGE as the cursor moves, got {seen:?}",
        );
    }

    /// **Deleting a card hands the cursor to the next one in the SAME row** (T327, item 4).
    ///
    /// The map is rebuilt when the session changes under it, and the rebuilt tree has to be told
    /// where the highlight goes — after the delete the pane is gone and its neighbour can no longer
    /// be found from it. So the neighbour is resolved while both still exist and the cursor is
    /// handed over first, through the same `ExposeCursor` intent an arrow key sends.
    #[test]
    fn deleting_a_card_moves_the_cursor_to_its_neighbour_first() {
        use heca_grid_ui::event::Event;
        let s = session();
        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let emit: crate::chrome::ChromeIntentEmitter = {
            let seen = seen.clone();
            std::rc::Rc::new(move |i| seen.borrow_mut().push(i))
        };
        // Open on the FIRST pane; the row also holds pane 2, which must take the cursor.
        let mut root = map(&rows, &theme, emit, Some(PaneId(1)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        heca_grid_ui::LayoutEngine::new()
            .compute(root.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));

        heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".to_string()));

        let order: Vec<String> = seen
            .borrow()
            .iter()
            .map(|i| match i {
                crate::app::interaction::InteractionIntent::ExposeCursor { pane_id } => {
                    format!("cursor:{}", pane_id.0)
                }
                crate::app::interaction::InteractionIntent::View(vi) => {
                    format!("action:{}", vi.action)
                }
                _ => "other".to_string(),
            })
            .collect();
        assert_eq!(
            order,
            vec!["cursor:2".to_string(), "action:close_pane_by_id".to_string()],
            "the cursor is handed to the neighbour BEFORE the delete, or the rebuilt map has \
             nowhere to put it",
        );
    }

    /// **The cursor reaches a floating pane.** Drawing it is half the job: a float belongs to no
    /// column, so it was in no cell, and moving between panes worked in a row without floats and
    /// stopped in a row with one (Antonio, driving, 2026-08-11). The floats are one more column of
    /// cells at the end of the row, so `l` walks onto them.
    #[test]
    fn the_cursor_walks_from_the_last_tiled_pane_onto_a_float() {
        use heca_core::layout::workspace::FloatingPane;
        use heca_core::layout::Point;
        use heca_grid_ui::event::{Event, WidgetIntent};
        let mut s = session();
        s.workspaces[0].floating_panes.push(FloatingPane {
            pane: Pane::new(PaneId(9), "float"),
            position: Point::new(40.0, 30.0),
            size: Size::new(200.0, 150.0),
            is_active: false,
            original_column_idx: None,
            original_pane_idx: None,
        });

        let rows = model(&s, |p| p.title.clone());
        let theme = heca_grid_ui::theme::Theme::default();
        let chosen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let emit: crate::chrome::ChromeIntentEmitter = {
            let chosen = chosen.clone();
            std::rc::Rc::new(move |intent| chosen.borrow_mut().push(intent))
        };
        // Open on the LAST tiled pane, so one step right is the float.
        let mut root = map(&rows, &theme, emit, Some(PaneId(2)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        let vp = heca_grid_ui::Size::new(1900.0, 1200.0);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);

        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), vp);
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::Activate));

        let focused: Vec<PaneId> = chosen
            .borrow()
            .iter()
            .filter_map(|i| match i {
                crate::app::interaction::InteractionIntent::FocusPaneThenAction {
                    pane_id, ..
                } => Some(*pane_id),
                _ => None,
            })
            .collect();
        assert_eq!(
            focused,
            vec![PaneId(9)],
            "one step past the last tiled pane lands on the float, and choosing it focuses it",
        );
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
        let mut root = map(&rows, &theme, emit, None, &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));

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
        let mut root = map(&rows, &theme, emit, Some(PaneId(2)), &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));

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
        let mut root = map(&rows, &theme, emit, None, &LayoutOptions::default(), &shipped_keys(), (1900.0, 1200.0));
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
        let got = seen.borrow().join(" ");
        assert!(
            got.contains("CloseOverlay"),
            "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
        );
    }

}
