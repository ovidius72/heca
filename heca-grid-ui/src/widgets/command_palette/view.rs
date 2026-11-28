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
    /// What else can be done with the row — its ⋯ button, right-click and keyboard menu.
    pub menu: Option<crate::widgets::ContextMenu>,
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
    /// The ⋯ button's tooltip when it opens a menu.
    pub menu_tip: Signal<String>,
}

/// The whole palette: the overlay and what it holds.
pub(super) fn view(state: &ViewState) -> Overlay {
    let query_changed = state.query_changed;
    let list = {
        let rows = state.rows.clone();
        let (selected, results, picks) = (state.selected, state.results, state.picks.clone());
        let size = state.size;
        let menu_tip = state.menu_tip;
        List::new(state.results)
            .render_item(move |&(_, index), _| {
                let rows = rows.borrow();
                match &rows[index].content {
                    Some(make) => make(),
                    None => crate::WidgetModel::new(row(&rows[index], index, selected, menu_tip)),
                }
            })
            // What else can be done with a row — declared on the row itself, so a right-click on it
            // and the keyboard's menu action on the selected one both find it.
            .item_menu({
                let rows = state.rows.clone();
                move |&(_, index), _| rows.borrow()[index].menu.clone()
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
    // The focus stays in the field while the arrows move the selection; the keyboard acts on the
    // list's selected row — its menu opens on the keyboard's menu action.
    let selected_row = list.selected_item();
    let query = Input::new()
        .active_descendant(selected_row)
        .value(state.query)
        .placeholder(state.placeholder)
        .placeholder_on_focus(true)
        .follow_focus(state.open)
        .width(Length::FULL)
        .on_change(move |_| query_changed.set(true));
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
fn row(
    data: &RowData,
    index: usize,
    selected: Signal<Option<usize>>,
    menu_tip: Signal<String>,
) -> Grid {
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
    // **One shortcut per line**, right-aligned: side by side, `^h ^← λh λ←` read as one long
    // chord, and nobody could tell where one binding ended and the next began. A shortcut of
    // several keys stays together on its line.
    let shortcuts = Flex::column()
        .align("end")
        .gap(Spacing::Xs)
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
        .template_column("auto 1fr auto auto")
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
            match &data.menu {
                // On the selected row — and the row under the pointer is the selected one: the list
                // selects what the pointer moves onto.
                Some(menu) => crate::WidgetModel::new(more_button(menu, is_selected, menu_tip)),
                // No menu: the column holds nothing.
                None => crate::WidgetModel::new(Flex::row()),
            },
        ])
}

/// **The ⋯ at the end of a row with a menu** — the same icon button a pane's header uses (a
/// `Button` showing only its icon), outlined so it reads as a control and never as a key: the
/// keycaps beside it are chips, this is a framed button that brightens to the accent on hover.
/// Centred on the row's height. Drawn while the row is the selected one (the list selects the row
/// the pointer moves onto), its box kept the rest of the time so the row does not move. It opens the menu where
/// it was clicked; a menu of one entry is not opened — the button runs that entry, and its words
/// (shown on hover) name it.
fn more_button(
    menu: &crate::widgets::ContextMenu,
    shown: impl Fn() -> bool + 'static,
    menu_tip: Signal<String>,
) -> Flex {
    use crate::builders::{EventExt, PresenceExt};
    let items = menu.menu().clone().into_items();
    let single = (items.len() == 1)
        .then(|| items.into_iter().next())
        .flatten();
    let words = single
        .as_ref()
        .and_then(|item| item.label_text().map(str::to_string))
        .unwrap_or_else(|| menu_tip.get_untracked());
    let menu = menu.clone();
    let button = crate::widgets::Button::new(words)
        .icon(Glyph::DotsThree)
        .icon_only(true)
        .variant(crate::widgets::ButtonVariant::Outline)
        .size(WidgetSize::Small)
        .visible(shown)
        // A button's click is its own: the row is not chosen by it.
        .on_click(move |ev| match &single {
            Some(item) => item.activate(),
            None => menu.show(ev.event()),
        });
    // Apart from the keycaps by a step of the theme's spacing, and centred on the row.
    Flex::row()
        .padding_left(Spacing::Xs)
        .align_self("center")
        .child(button)
}
