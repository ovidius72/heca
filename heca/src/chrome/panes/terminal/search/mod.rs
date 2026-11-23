//! **A terminal's own search**: the bar over its picture, the highlights on its matches, and what
//! the user can ask of it.
//!
//! The bar is a child of the [`Terminal`](super::Terminal), like the scrollback chip, so placing a
//! terminal anywhere places its search with it: nothing is wired by whoever placed it. The terminal
//! hears the query field, says what the user did ([`Search`]), and paints the
//! matches it is told. Finding them is the owner's — it holds the process.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::backend::SearchMatch;
use heca_core::layout::{Point, Rectangle, Size};
use heca_grid_ui::SignalData;
use heca_grid_ui::builders::{EventExt, FocusExt, LayoutExt, Parent, PresenceExt, TooltipExt};
use heca_grid_ui::component::PaintCx;
use heca_grid_ui::event::EventKind;
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, SignalWith, signal};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Flex, Glyph, Icon, IconButton, Input, Label, PanelElevation, Surface};

use super::input::{Search, TerminalInput};
use super::shared::Shared;

/// **What one terminal's search is**, shared by every node placed for it — so a tree rebuilt around
/// the terminal shows the search it had. What the bar shows (open, the query, the counter, where the
/// keyboard goes) is held as signals the bar is bound to; the matches stay plain data the terminal
/// paints.
pub(super) struct SearchSlot {
    /// Whether the bar is shown.
    open: Signal<bool>,
    /// The field should hold the keyboard: set when the search is asked for, cleared when the field
    /// lets the keyboard go — so asking again while the bar is open but unfocused moves the keyboard.
    focus: Signal<bool>,
    /// What the field holds — shared with it both ways.
    query: Signal<String>,
    /// The counter's words (`3/12`, `no matches`), empty while there is no query.
    count: Signal<String>,
    matches: RefCell<Vec<SearchMatch>>,
    current: Cell<Option<usize>>,
    /// The first row on screen, as a stable row, and how many rows are on screen.
    top: Cell<isize>,
    rows: Cell<usize>,
    /// How strongly a match is highlighted, and the current one — the user's, said by the owner.
    alpha: Cell<(u8, u8)>,
}

impl Default for SearchSlot {
    fn default() -> Self {
        Self {
            open: signal(false),
            focus: signal(false),
            query: signal(String::new()),
            count: signal(String::new()),
            matches: RefCell::default(),
            current: Cell::default(),
            top: Cell::default(),
            rows: Cell::default(),
            alpha: Cell::default(),
        }
    }
}

impl SearchSlot {
    /// **Show the bar and give the field the keyboard.**
    pub(super) fn open(&self) {
        if !self.open.get_untracked() {
            self.open.set(true);
        }
        self.focus.set(true);
    }

    pub(super) fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// **Dismiss the search**: the bar goes and so do the matches and the query.
    pub(super) fn close(&self) {
        self.open.set(false);
        self.focus.set(false);
        self.query.set(String::new());
        self.matches.borrow_mut().clear();
        self.current.set(None);
        self.recount();
    }

    /// The counter follows the query, the matches and the current one — written in one place.
    fn recount(&self) {
        let words = self.label().unwrap_or_default();
        if self.count.get_untracked() != words {
            self.count.set(words);
        }
    }

    /// The field's text changed.
    pub(super) fn set_query(&self, text: &str) {
        if self.query.get_untracked() != text {
            self.query.set(text.to_string());
        }
        self.recount();
    }

    /// **Show these matches**, with `current` the focused one. Returns whether anything the user can
    /// see changed — the same result twice changes nothing, so an open search costs no frame.
    pub(super) fn set_result(&self, matches: Vec<SearchMatch>, current: Option<usize>) -> bool {
        if *self.matches.borrow() == matches && self.current.get() == current {
            return false;
        }
        *self.matches.borrow_mut() = matches;
        self.current.set(current);
        self.recount();
        true
    }

    /// **Move the current match by one**, wrapping. Returns whether it moved: with no matches
    /// there is nowhere to go.
    pub(super) fn step(&self, forward: bool) -> bool {
        let n = self.matches.borrow().len();
        if n == 0 {
            return false;
        }
        let cur = self.current.get().unwrap_or(0);
        let next = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        self.current.set(Some(next));
        self.recount();
        true
    }

    /// The current match, if there is one — a copy of that one, not of the list.
    pub(super) fn current_match(&self) -> Option<SearchMatch> {
        let i = self.current.get()?;
        self.matches.borrow().get(i).cloned()
    }

    /// Say which rows are on screen. Returns whether a highlight moved: with no matches there is
    /// nothing drawn to move, so scrolling costs nothing.
    pub(super) fn set_view(&self, top: isize, rows: usize) -> bool {
        let moved = self.top.replace(top) != top;
        let resized = self.rows.replace(rows) != rows;
        (moved || resized) && !self.matches.borrow().is_empty()
    }

    /// Say how strongly matches are highlighted: `matches`, and the `current` one. Returns whether a
    /// highlight changed — with none drawn, a new look is not a change anyone can see.
    pub(super) fn set_look(&self, matches: u8, current: u8) -> bool {
        let changed = self.alpha.replace((matches, current)) != (matches, current);
        changed && !self.matches.borrow().is_empty()
    }

    /// The match position as it reads — `3/12`, `no matches` — or nothing while the query is empty.
    pub(super) fn label(&self) -> Option<String> {
        count_label(
            !self.query.with_untracked(String::is_empty),
            self.matches.borrow().len(),
            self.current.get(),
        )
    }

    /// **Where each visible match is drawn**, and whether it is the current one. Cells are counted
    /// from `origin`, the terminal's top-left, in cells of `cell` pixels.
    pub(super) fn highlights(&self, origin: Point, cell: (f32, f32)) -> Vec<(Rectangle, bool)> {
        let (cell_w, cell_h) = (cell.0 as f64, cell.1 as f64);
        let current = self.current.get();
        let (top, rows) = (self.top.get(), self.rows.get() as isize);
        self.matches
            .borrow()
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let row = m.stable_row - top;
                (0..rows).contains(&row).then(|| {
                    let rect = Rectangle::new(
                        Point::new(
                            origin.x + m.start_col as f64 * cell_w,
                            origin.y + row as f64 * cell_h,
                        ),
                        Size::new(
                            m.end_col.saturating_sub(m.start_col) as f64 * cell_w,
                            cell_h,
                        ),
                    );
                    (rect, Some(i) == current)
                })
            })
            .collect()
    }

    /// Paint every visible match as a tinted cell run, the current one bolder.
    pub(super) fn paint_highlights(&self, cx: &mut PaintCx, origin: Point, cell: (f32, f32)) {
        let accent = cx.theme().colors.accent;
        let radius = cx.theme().colors.control_radius();
        let (match_alpha, current_alpha) = self.alpha.get();
        for (rect, current) in self.highlights(origin, cell) {
            let alpha = if current { current_alpha } else { match_alpha };
            cx.rect(rect, accent.with_alpha(alpha), None, radius, None);
        }
    }
}

/// The match counter's text: nothing until there is a query, then `no matches` or `position/total`.
pub(super) fn count_label(has_query: bool, total: usize, current: Option<usize>) -> Option<String> {
    if !has_query {
        return None;
    }
    if total == 0 {
        return Some("no matches".to_string());
    }
    Some(format!("{}/{}", current.map_or(0, |i| i + 1), total))
}

/// **The search bar**: the query field and the match counter, in a row at the terminal's
/// bottom-right corner — out of the flow, opposite the scrollback chip, taking no room and moving
/// nothing. One per node placed for a terminal; all are bound to the one [`SearchSlot`], so they
/// follow it with nothing copied by hand.
pub(super) fn search_bar(shared: Rc<Shared>) -> Flex {
    let search = &shared.search;
    let (open, focus, query, count) = (search.open, search.focus, search.query, search.count);
    let (edited, gained, lost) = (shared.clone(), shared.clone(), shared.clone());
    let field = Input::new()
        .value(query)
        .clearable(true)
        .follow_focus(focus)
        .on_change(move |action| {
            if let SignalData::String(text) = action.data {
                edited.search.set_query(&text);
                edited.emit(TerminalInput::Search(Search::Query(text)));
            }
        })
        .on(EventKind::Focus, move |_| {
            gained.emit(TerminalInput::Search(Search::Editing(true)));
        })
        .on(EventKind::Blur, move |_| {
            lost.search.focus.set(false);
            lost.emit(TerminalInput::Search(Search::Editing(false)));
        });
    let (back, ahead) = (shared.clone(), shared.clone());
    // **One panel in the terminal's bottom-right corner, in the menus' material** — the theme's
    // surface fill, rounded corners, edge and drop shadow — so the terminal's text never shows
    // through it. Inside, the match count in
    // small quiet text over the field's right end, then the field and its two arrows. The panel is
    // anchored at the bottom, so the count appearing, changing or going grows it upwards and never
    // moves the field; it is never wider than the terminal, and when room is short only the field
    // gives some up. An arrow steps and hands the keyboard back to the field, so typing and
    // Ctrl+j/k go on where they were.
    let panel = Surface::column()
        .elevation(PanelElevation::Panel)
        .padding(Spacing::Sm)
        .gap(Spacing::Xs)
        .child(
            Label::new("")
                .text(count)
                .muted(true)
                .size(WidgetSize::Caption)
                .align_self("end")
                .hidden(move || count.with(String::is_empty)),
        )
        .child(
            Flex::row()
                .align("center")
                .gap(Spacing::Xs)
                .child(field)
                .child(
                    IconButton::new(Icon::new(Glyph::CaretUp))
                        .shrink(0.0)
                        .tooltip("Previous match")
                        .on_click(move |_| {
                            back.step(false);
                            back.search.open();
                        }),
                )
                .child(
                    IconButton::new(Icon::new(Glyph::CaretDown))
                        .shrink(0.0)
                        .tooltip("Next match")
                        .on_click(move |_| {
                            ahead.step(true);
                            ahead.search.open();
                        }),
                ),
        );
    // The row across the terminal's bottom holds the panel at its right end; the space around the
    // panel is the row's padding, inside the terminal, so a narrow terminal shrinks the panel
    // rather than pushing it past the left edge.
    Flex::row()
        .justify("end")
        .padding(Spacing::Sm)
        .child(panel)
        .hidden(move || !open.get())
        .at_rect(
            Length::Auto,
            Length::Auto,
            Length::Percent(1.0),
            Length::Auto,
        )
        .align_self("end")
}

#[cfg(test)]
mod tests;
