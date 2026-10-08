//! **A terminal's own search**: the bar over its picture, the highlights on its matches, and what
//! the user can ask of it.
//!
//! The bar is a child of the [`Terminal`](super::Terminal), like the scrollback chip, so placing a
//! terminal anywhere places its search with it: nothing is wired by whoever placed it. The terminal
//! hears the query field, says what the user did ([`Search`](super::input::Search)), and paints the
//! matches it is told. Finding them is the owner's — it holds the process.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::backend::SearchMatch;
use heca_core::layout::{Point, Rectangle, Size};
use heca_grid_ui::SignalData;
use heca_grid_ui::builders::{ComponentExt, LayoutExt};
use heca_grid_ui::component::{Base, Component, PaintCx};
use heca_grid_ui::event::EventKind;
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::{Direction, Length, Spacing};
use heca_grid_ui::widgets::{Input, Tag};

use super::input::{Search, TerminalInput};
use super::shared::Shared;

/// **What one terminal's search is**, shared by every node placed for it — so a tree rebuilt around
/// the terminal shows the search it had.
#[derive(Default)]
pub(super) struct SearchSlot {
    /// Whether the bar is shown.
    open: Cell<bool>,
    /// The field should take the keyboard on the next tick. A request, consumed once.
    focus_wanted: Cell<bool>,
    /// What the field holds: the last edit.
    query: RefCell<String>,
    matches: RefCell<Vec<SearchMatch>>,
    current: Cell<Option<usize>>,
    /// The first row on screen, as a stable row, and how many rows are on screen.
    top: Cell<isize>,
    rows: Cell<usize>,
    /// How strongly a match is highlighted, and the current one — the user's, said by the owner.
    alpha: Cell<(u8, u8)>,
}

impl SearchSlot {
    /// **Show the bar and give the field the keyboard.**
    pub(super) fn open(&self) {
        self.open.set(true);
        self.focus_wanted.set(true);
    }

    pub(super) fn is_open(&self) -> bool {
        self.open.get()
    }

    /// **Dismiss the search**: the bar goes and so do the matches and the query.
    pub(super) fn close(&self) {
        self.open.set(false);
        self.focus_wanted.set(false);
        self.query.borrow_mut().clear();
        self.matches.borrow_mut().clear();
        self.current.set(None);
    }

    /// The field's text changed.
    pub(super) fn set_query(&self, text: &str) {
        if *self.query.borrow() != text {
            *self.query.borrow_mut() = text.to_string();
        }
    }

    /// **Show these matches**, with `current` the focused one. Returns whether anything the user can
    /// see changed — the same result twice changes nothing, so an open search costs no frame.
    pub(super) fn set_result(&self, matches: Vec<SearchMatch>, current: Option<usize>) -> bool {
        if *self.matches.borrow() == matches && self.current.get() == current {
            return false;
        }
        *self.matches.borrow_mut() = matches;
        self.current.set(current);
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
            !self.query.borrow().is_empty(),
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
/// bottom-right corner. One per node placed for a terminal; they all show the one [`SearchSlot`].
pub(super) struct SearchBar {
    base: Base,
    shared: Rc<Shared>,
    /// The field's text, to put back what a rebuilt tree should show.
    text: Signal<String>,
    /// The counter's label and whether it is shown.
    label: Signal<String>,
    label_shown: Signal<bool>,
}

impl SearchBar {
    pub(super) fn new(shared: Rc<Shared>) -> Self {
        let edited = shared.clone();
        let gained = shared.clone();
        let lost = shared.clone();
        let field = Input::new()
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
                lost.emit(TerminalInput::Search(Search::Editing(false)));
            });
        let text = field.text();
        let chip = Tag::new("");
        let label = chip.label_signal();
        let label_shown = chip.base().visible;
        label_shown.set(false);

        let mut base = Base::new();
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = "center".into();
        base.style.layout.gap = Spacing::Sm.into();
        base.children.push(Box::new(field));
        base.children.push(Box::new(chip));
        base.visible.set(shared.search.is_open());
        // Out of the flow, every edge left to the terminal, and told to end at the bottom: it
        // lands in the corner opposite the scrollback chip, taking no room and moving nothing.
        let bar = Self {
            base,
            shared,
            text,
            label,
            label_shown,
        };
        bar.at_rect(Length::Auto, Length::Auto, Length::Auto, Length::Auto)
            .align_self("end")
            .margin(Spacing::Sm)
    }
}

impl Component for SearchBar {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            heca_grid_ui::paint_child(child.as_ref(), cx);
        }
    }

    /// Follow the terminal's search: shown while it is open, the counter and the text as it says,
    /// and the field takes the keyboard when it was asked to.
    fn tick(&mut self, dt: f32) -> bool {
        let search = &self.shared.search;
        let open = search.is_open();
        if self.base.visible.get_untracked() != open {
            self.base.visible.set(open);
            self.base.mark_needs_paint();
        }
        if open {
            if search.focus_wanted.replace(false) {
                self.base.children[0].base().focus(true);
                // Asked for by code, so no Focus event announces it: say it here.
                self.shared
                    .emit(TerminalInput::Search(Search::Editing(true)));
            }
            let query = search.query.borrow().clone();
            if self.text.get_untracked() != query {
                self.text.set(query);
            }
            let label = search.label();
            let shown = label.is_some();
            if self.label_shown.get_untracked() != shown {
                self.label_shown.set(shown);
            }
            if let Some(label) = label
                && self.label.get_untracked() != label
            {
                self.label.set(label);
            }
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

impl LayoutExt for SearchBar {}

#[cfg(test)]
mod tests;
