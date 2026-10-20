//! The **exposé** — a bird's-eye view of every workspace at once.
//!
//! One row per workspace, each laying out **all** of that workspace's columns side by side —
//! including the ones scrolled off-screen, which is the whole point of the view. Panes stack
//! vertically inside their column, and a floating pane is drawn over the strip where it actually
//! sits. It is the same idea as niri's overview (`docs/niri-wiki/03-usage/overview.md`): a
//! zoomed-out map you navigate and act on, not a second way to use the app.
//!
//! # The shape of this surface
//!
//! It is **components**, not one function that draws a picture — each taking properties, events and
//! callbacks, encapsulating its own logic, and testable on its own without a window:
//!
//! | | owns |
//! |---|---|
//! | [`model`](mod@model) | what is on screen, where and how big, as plain data reduced from the session |
//! | [`PaneCard`](pane_card::PaneCard) | one pane: its name, its cursor state, its delete letters, its activate |
//! | [`ColumnCard`](column_card::ColumnCard) | a column: the panes' **vertical** shares |
//! | [`WorkspaceRow`](workspace_row::WorkspaceRow) | a row: the columns' **horizontal** shares, and the floats' rects |
//! | [`ExposeGrid`] | the rows' shares of the map, and the cursor |
//! | this module | [`map`] assembles the layer from plain data |
//! | [`wiring`](mod@wiring) | the host side — [`register`] gathers `AppState`, [`callbacks`] turns the map's acts into intents |
//!
//! They live **here, beside the surface**, and move up to `crate::components` the day a second
//! surface wants one (§ 0b).
//!
//! # ⚠️ Every size is a share; nothing is a pixel
//!
//! This module used to compute a `zoom` from the session's extents and emit `Length::Px(width *
//! zoom)`. Doing that takes over the layout engine's job — and then it owns *every* term: the
//! window, the overlay margin, the panel padding, the gaps between cards, the row count, each row's
//! height, the scroll region's centring, the overscroll range. Miss one and the map is wrong by
//! exactly that term, which happened **seven times, six of them differently**.
//!
//! So a column's width is a percentage of one shared denominator, a pane's height is a `grow`
//! weight, a float is a rect of percentages, and taffy answers all of it exactly at every window
//! size. There is no fit to compute, nothing to cap, and nothing to scroll: a picture built out of
//! shares of the window cannot overflow it.
//!
//! **The zoom *effect* is a different thing and stays** — it is declared on the surface itself
//! ([`Overlay::animation`](heca_grid_ui::widgets::Overlay::animation)) and plays over any surface,
//! a plugin's included. A paint transform is not a size: nothing is measured again.
//!
//! Panes are drawn as **boxes** for now: a border, the pane's name and its icon. A real snapshot
//! needs a scene primitive `heca-grid-ui` does not have (its vocabulary is `Rect`, `Text`, `Icon`,
//! clips — there is no image command) plus per-pane offscreen capture in `heca-renderer`. Nothing
//! in the layout, the navigation or the layer changes when that lands: only what fills the box.

pub(crate) mod column_card;
pub(crate) mod expose_grid;
pub(crate) mod model;
pub(crate) mod pane_card;
mod wiring;
pub(crate) mod workspace_row;

#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

use heca_core::layout::PaneId;
use heca_grid_ui::Component;
use heca_grid_ui::animation::Animation;
use heca_grid_ui::builders::{LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Length, Spacing};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{KeyHintGroup, Overlay, Surface};

pub(crate) use expose_grid::ExposeGrid;
pub(crate) use model::{ExposeWorkspace, model};
pub(crate) use pane_card::{DispatchAction, ExposeCallbacks, ExposeDeleteKeys};
use wiring::callbacks;
pub(crate) use wiring::{record_expose_cursor, register};

/// The surface name the map registers under, and the one a `[[keys.surface]]` entry addresses. One
/// constant so the layer, the config entry and the key lookup cannot drift apart.
pub(crate) const SURFACE: &str = "expose";

/// **The map's own picker, declared on the widget**.
///
/// The action a `[[keys.surface]] heca.expose` entry binds as `pick`. It is the map's, not the
/// app's: the surface declares the verb, the widget answers it, and config chooses the key. Before
/// this the map had to borrow the built-in `hint_pick` — the only picker there was — which is why a
/// plugin's overlay could contribute targets to heca's picker and never open one of its own.
pub(crate) const PICK_ACTION: &str = "heca.expose.pick";

/// Build the overview layer — **a composition, not a widget**: it binds `PaneId` to a `FocusPane`
/// intent and the `close_overlay` action, which is the only reason it is not in `heca-grid-ui`.
///
/// Plain data in, a `Component` out, and **never `&AppState`** (§ 0b): the session is already
/// reduced to [`ExposeWorkspace`]s by [`model`](fn@model), so the whole map is testable without a window.
///
/// What is drawn, and what is not:
/// - **A workspace is a row.** No box, no border, no fill — position and scale say which it is.
/// - **A column is invisible grouping** that takes its share of the row's width.
/// - **A pane is the only thing with a surface.**
///
/// The overlay behaviour is [`Overlay`]'s and the cursor is [`ExposeGrid`]'s. Neither is
/// re-implemented here.
pub(crate) fn map(
    rows: &[ExposeWorkspace],
    theme: &GuiTheme,
    emit: crate::chrome::ChromeIntentEmitter,
    start: Option<PaneId>,
    geometry: &heca_core::layout::LayoutOptions,
    keys: &ExposeDeleteKeys,
    // The pane a back-and-forth binding would return to — plain data, resolved by `register` from
    // the same field `prefix+i` reads.
    previous: Option<PaneId>,
) -> Box<dyn Component> {
    let cb = callbacks(emit, keys.clone());

    let grid = ExposeGrid {
        rows,
        // **The map's spacing is the layout's, not this module's.** `overview_gap` is a
        // `LayoutOptions` field the user sets in config.
        gap_frac: geometry.overview_gap,
        start,
        previous,
        theme,
        cb: &cb,
    }
    .build();

    // **The picker is a widget, and the map declares it.** Open it and every card beneath wears a
    // letter — each drawn by the card itself, so it lands wherever the card is, at any nesting
    // depth. Typing one runs that card's own `on_hint`. Nothing host-side is involved: no input
    // mode, no host paint pass, no registry.
    let grid = KeyHintGroup::new(grid)
        .opens_on(PICK_ACTION)
        // The wrapper hugs its child, so the room the panel gives it has to be passed on
        // deliberately — the grid inside is a share of *this*, and a hugged wrapper would leave it
        // resolving a percentage of nothing (the same term the cards' `KeyHint` needs).
        .width(Length::FULL)
        .height(Length::FULL);

    Box::new(
        Overlay::new()
            .blocking(true)
            // **The session stays there underneath, legibly out of focus.** A flat fill made the
            // map a different screen; the blur makes it a lens. One builder on the widget — the
            // surface asks and the host performs it, so this travels with the tree instead of
            // being a fact the registry held about it.
            .frosted(true)
            // **The map names how it comes and goes, exactly as a plugin's surface would.**
            //
            // It opens by pulling back, the way niri's overview does — the same session seen from
            // further away, rather than a different picture arriving: a zoom, not a fade. Going,
            // the shrink leads and the dissolve rides it,
            // which is what `ZoomFade` **is**: the gesture is defined once, in the library, so no
            // surface composes it out of parts and none of them can drift.
            //
            // How far back is `[settings] overview_zoom_from`, not a constant here. Starting the
            // cards at exactly life size is the truest reading and overshoots — at 2× the outer
            // rows begin off-screen and rush in. The setting defaults to a gentler value, and a
            // user who wants the literal reading sets 2.0.
            .animation(Animation::ZoomFade.from(geometry.overview_zoom_from as f32))
            .panel_size(Length::Percent(1.0), Length::Percent(1.0))
            // **The panel is the frost's tint, not a lid.** The blurred frame is stamped under this
            // surface (`.frosted(true)` above), so the surface here is the background
            // colour at the theme's scrim strength: enough to hold the cards' contrast, thin
            // enough that the session reads behind them as a blurred wash. Painted opaque it hid
            // the blur completely; left at heca's own window colour — translucent by design, for
            // the frosted compositor — the app read straight through it, sharp.
            .panel(
                Surface::column()
                    .background(
                        theme
                            .colors
                            .background
                            .with_alpha(theme.colors.interaction.scrim),
                    )
                    .padding(Spacing::Md)
                    .child(grid),
            )
            // **Closed until the stack shows it**, which is what plays the arrival: `ShowLayer`
            // rebuilds the map and then shows the layer, and showing states that the surface is
            // open. A rebuild while it is already up carries the gesture over from the tree it
            // replaces, so nothing replays.
            .default_open(false),
    )
}
