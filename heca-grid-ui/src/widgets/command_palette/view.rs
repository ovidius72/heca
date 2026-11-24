//! **The palette, composed** — an [`Overlay`] holding the query field over a scrolling list of
//! rows, laid out by the engine. The rows are a [`List`]: it builds only the rows on screen, draws
//! and moves the selection, and reports a choice; a row here is only what it shows. It is **built once and kept**, following the palette's `open`:
//! built again on each opening, every row's text had to be measured again and the panel was slow
//! to appear. Nothing here positions, measures or hit-tests anything: the
//! overlay places the panel, the scroll region scrolls and keeps the selected row in view
//! (`Row::reveal_when`), each row answers its own pointer, and the rows follow the ranked results
//! (`.child(results.map(..))`), keeping the ones still there.
//!
//! ```text
//! Overlay ─ Flex column, its width from the palette's size, capped at the window by the overlay
//!             ├ query Input (keeps its height)
//!             └ List of the results (as many rows as the size shows; gives way in a short window)
//! row: Row ─ Grid "auto 1fr auto": icon · title / description / detail · shortcuts (Keycaps)
//! ```

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::metrics::{PANEL_FADE_SECONDS, PANEL_HEIGHT_SHARE, SHORTCUTS_MAX_EM, panel_metrics};
use crate::animation::Animation;
use crate::builders::{FocusExt, LayoutExt, Parent};
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::style::{Length, Spacing, WidgetSize};
use crate::widgets::{
    Ellipsis, Flex, Glyph, Grid, Icon, Input, KeyCap, Keycap, KeycapVariant, Label, List, Overlay,
    OverlayPosition,
};

/// What a row shows, kept by the palette for as long as its command exists — so a row that leaves
/// the results and comes back is built again from the same signals, and a rename or a re-mark
/// written while it was away is what it shows.
pub(super) struct RowData {
    pub title: Signal<String>,
    pub title_marks: Signal<Vec<usize>>,
    pub description: Option<(String, bool)>,
    pub description_marks: Signal<Vec<usize>>,
    pub detail: Option<String>,
    pub icon: Option<Glyph>,
    pub chords: Vec<Vec<KeyCap>>,
    pub trailing: Option<Rc<dyn Fn() -> crate::WidgetModel>>,
    /// What the row shows instead of the standard one, when its command says.
    pub content: Option<Rc<dyn Fn() -> crate::WidgetModel>>,
}

/// A row chosen by the pointer, by command index, for the palette to run on its next look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pick {
    Choose(usize),
}

/// The signals and shared state the view reads, all owned by the palette.
#[derive(Clone)]
pub(super) struct ViewState {
    pub rows: Rc<RefCell<Vec<RowData>>>,
    /// The commands shown, in rank order — `(generation, index)`, so a list rebuilt from a live
    /// source makes new rows rather than keeping the old ones under the same index.
    pub results: Signal<Vec<(u64, usize)>>,
    /// The selected command, by index — what a row's description reads to reflow.
    pub selected: Signal<Option<usize>>,
    /// The selected row, by position in the results — the [`List`]'s selection.
    pub list_selected: Signal<Option<usize>>,
    pub picks: Rc<Cell<Option<Pick>>>,
    pub size: Signal<WidgetSize>,
    pub open: Signal<bool>,
    /// The query text: the field is built on it, so what is typed is written here.
    pub query: Signal<String>,
    /// Set when the user edits the query; the palette ranks again on its next tick.
    pub query_changed: Signal<bool>,
    pub placeholder: Signal<String>,
}

/// The whole palette: the overlay and what it holds.
pub(super) fn view(state: &ViewState) -> Overlay {
    let query_changed = state.query_changed;
    let query = Input::new()
        .value(state.query)
        .placeholder(state.placeholder)
        .placeholder_on_focus(true)
        .follow_focus(state.open)
        .width(Length::FULL)
        .on_change(move |_| query_changed.set(true));
    let list = {
        let rows = state.rows.clone();
        let (selected, results, picks) = (state.selected, state.results, state.picks.clone());
        let size = state.size;
        List::new(state.results)
            .render_item(move |&(_, index), _| {
                let rows = rows.borrow();
                match &rows[index].content {
                    Some(make) => make(),
                    None => crate::WidgetModel::new(row(&rows[index], index, selected)),
                }
            })
            .selected(state.list_selected)
            .on_choose(move |at| {
                if let Some(&(_, index)) = results.get_untracked().get(at) {
                    picks.set(Some(Pick::Choose(index)));
                }
            })
            // A flat one-pixel line over the selection's fill, as a pane's frame.
            .border_width(1.0)
            // As many rows as the palette's size shows before it scrolls.
            .visible_items(move || panel_metrics(size.get()).1)
    };
    // The panel's body: the query field over the list, as wide as the palette's size says. A
    // column, so in a short window the list gives way and the field keeps its height.
    let size = state.size;
    let body = Flex::column()
        .padding(Spacing::Md)
        .gap(Spacing::Sm)
        .width(move || Length::Px(panel_metrics(size.get()).0 as f32))
        // Never taller than its share of the window, so it ends at about 60% of it; the list gives
        // way and scrolls.
        .max_height(Length::Percent(PANEL_HEIGHT_SHARE))
        .child(crate::WidgetModel::new(query))
        .child(crate::WidgetModel::new(list));
    let open = state.open;
    Overlay::new()
        // In the upper part of the window, so the list has room to grow downward.
        .position(OverlayPosition::Top)
        .panel(body)
        .on_outside_click(move || open.set(false))
        // A quick dissolve in and out — quicker than the default, so it is there as you type.
        .animation(Animation::Fade.seconds(PANEL_FADE_SECONDS))
        .open(state.open)
}

/// What one result row shows — the [`List`] puts it in the row it selects.
fn row(data: &RowData, index: usize, selected: Signal<Option<usize>>) -> Grid {
    let is_selected = move || selected.get() == Some(index);
    let title = Label::new("")
        .text(data.title)
        .marks(data.title_marks)
        .truncate(Ellipsis::End);
    let first_line: crate::WidgetModel = match &data.trailing {
        Some(make) => crate::WidgetModel::new(
            Grid::new()
                .template_column("1fr auto")
                .align("center")
                .gap(Spacing::Sm)
                .child([crate::WidgetModel::new(title), make()]),
        ),
        None => crate::WidgetModel::new(title),
    };
    // `min_width(0)`: the text column may be handed less than its text is wide — CSS's grid item
    // `min-width: 0` — so a long title is cut and a description cut or reflowed instead of running
    // into the shortcuts' column.
    let mut text = Flex::column().min_width(0.0).child(first_line);
    if let Some((description, path)) = &data.description {
        let label = Label::new(description.clone())
            .muted(true)
            .marks(data.description_marks);
        // A path is cut from its start and never reflows; a description reflows while its row is
        // the selected one — that reflow, moving the rows below it, is the selection's visual.
        text = text.child(if *path {
            label.truncate(Ellipsis::Start)
        } else {
            label.truncate(Ellipsis::End).wrap(is_selected)
        });
    }
    if let Some(detail) = &data.detail {
        text = text.child(
            Label::new(detail.clone())
                .muted(true)
                .truncate(Ellipsis::End),
        );
    }
    // Side by side, starting a new line when they do not fit — a row with ten one-key shortcuts
    // (`1` … `0`) takes a line or two, not ten. A shortcut of several keys stays together.
    let shortcuts = Flex::row()
        .wrap(true)
        .justify("end")
        .gap(Spacing::Sm)
        .max_width(Length::Em(SHORTCUTS_MAX_EM))
        .child(
            data.chords
                .iter()
                .map(|chord| {
                    crate::WidgetModel::new(
                        Flex::row().gap(Spacing::Xs).child(
                            chord
                                .iter()
                                .map(|key| {
                                    crate::WidgetModel::new(
                                        Keycap::new(key.clone())
                                            .variant(KeycapVariant::Bordered)
                                            .size(WidgetSize::Small),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        );
    Grid::new()
        .template_column("auto 1fr auto")
        .align("start")
        .gap(Spacing::Sm)
        .width(Length::FULL)
        .child([
            match data.icon {
                Some(glyph) => crate::WidgetModel::new(Icon::new(glyph)),
                // No icon: the column holds nothing, and the text starts at the row's edge.
                None => crate::WidgetModel::new(Flex::row()),
            },
            crate::WidgetModel::new(text),
            crate::WidgetModel::new(shortcuts),
        ])
}
