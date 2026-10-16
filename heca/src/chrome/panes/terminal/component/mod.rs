//! [`Terminal`] — the component itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::Rectangle;
use heca_grid_ui::{Point, Size};

use super::input::{Search, TerminalInput};
use super::search::{SearchBar, SearchSlot};
use super::shared::Shared;
use super::viewport::{Controls, IntentSlot};
use heca_grid_ui::builders::{EventExt, LayoutExt};
use heca_grid_ui::component::{Base, Component, GridKey, PaintCx, WidgetIntent};
use heca_grid_ui::event::EventKind;
use heca_grid_ui::style::Length;

/// **A terminal you can place anywhere** — the DOM's `<terminal/>`.
///
/// It fills whatever box it is put in, and learns that box's size from the layout, which is the
/// only thing it needs to know to size its grid. Nothing outside computes a rect for it.
///
/// `Terminal` is a handle: [`Clone`] gives another view of the **same** terminal, so the one
/// placed in a tree and the one kept by the caller agree at once.
pub struct Terminal {
    base: Base,
    shared: Rc<Shared>,
    /// This node's own scrollback controls. They are its children; the handle is kept so they stay
    /// alive in `shared.controls` for as long as the node does.
    _controls: Rc<Controls>,
}

impl Terminal {
    /// A terminal. What it runs, and what it is called, arrive with the process it shows.
    pub(crate) fn new() -> Self {
        Self::of(Rc::new(Shared {
            id: Cell::new(None),
            size: Cell::new(None),
            placed: Cell::new(None),
            intents: IntentSlot::default(),
            cell: Cell::new((0.0, 0.0)),
            bounds: Cell::new(Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0))),
            nominal: Cell::new((0.0, 0.0)),
            reported: Cell::new(None),
            controls: RefCell::default(),
            declared: RefCell::new(None),
            search: SearchSlot::default(),
            links: RefCell::default(),
            view: Cell::new((0, 0)),
            link_modifier: Cell::new(heca_config::theme::ModifierKey::default()),
            node: Cell::new(0),
            wants_keyboard: Cell::new(false),
        }))
    }

    fn of(shared: Rc<Shared>) -> Self {
        let mut base = Base::new();
        // It fills its parent, and says nothing more: a share of the box it is given, never a
        // measure of its own. Unset already stretches, but a leaf has no content to size it, so it
        // asks for the whole of the cell it sits in.
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Percent(1.0);
        // It lays out what sits over its picture: a row that ends at the right edge.
        base.style.layout.direction = heca_grid_ui::style::Direction::Row;
        base.style.layout.justify = "end".into();
        base.style.layout.align = "start".into();
        // **It takes the keyboard when it is clicked**, like any focusable — and says nothing else:
        // no host wiring teaches it that.
        base.focusable = true;
        // **The scrollback controls are part of the terminal**, drawn over its picture by the same
        // walk, so placing a terminal anywhere places them with it.
        let (controls, layers) = Controls::build(&shared.intents);
        base.children.extend(layers);
        // **So is its search**: the bar over the picture, drawn by the same walk, hearing the same
        // keyboard. Declared after the controls, so it is on top of them.
        base.children.push(Box::new(SearchBar::new(shared.clone())));
        let controls = Rc::new(controls);
        shared.controls.borrow_mut().add(&controls);
        let me = shared.clone();
        // **The wheel is the terminal's own.** It works out where in the grid it turned and says so
        // to whoever owns the process; whatever is beneath it is not offered the turn.
        Self {
            base,
            shared,
            _controls: controls,
        }
        .on_scroll({
            let me = me.clone();
            move |cx| me.wheel(cx)
        })
        // **So are the buttons** (and the pointer's moves, in `on_event`), but only when the
        // terminal itself is what they landed on: the chip and the scrollbar are its children, and
        // a press or a move on them is theirs. None of these takes the event — a window gesture (a
        // divider, a move modifier) decides about the same press on its own.
        // **A press with the window's key held is the window's**: it starts a pane move, or — if it
        // never travels — is a click that opens a link. The program is told neither half.
        .on(EventKind::PointerDown, {
            let me = me.clone();
            move |cx| {
                if me.held_for_window(cx) {
                    return;
                }
                me.pointer(cx, |p, cell| TerminalInput::Press {
                    button: p.button,
                    cell,
                    modifiers: p.modifiers,
                })
            }
        })
        .on(EventKind::PointerUp, {
            let me = me.clone();
            move |cx| {
                if me.held_for_window(cx) {
                    return;
                }
                me.pointer(cx, |p, cell| TerminalInput::Release {
                    button: p.button,
                    cell,
                    modifiers: p.modifiers,
                })
            }
        })
        // **Clicking a link with the window's key held opens it.** The framework tells a click from
        // the start of a drag by how far the pointer travelled, so the same key serves both.
        .on(EventKind::Click, move |cx| me.open_link(cx))
    }
}

impl Terminal {
    /// The search is open and the keyboard is not on the terminal itself: it is in the bar.
    fn keyboard_is_in_search(&self) -> bool {
        self.shared.search.is_open() && !self.base.is_focused()
    }
}

impl Clone for Terminal {
    /// Another view of the same terminal: a separate node for a tree, the same name and the same
    /// report of its size.
    fn clone(&self) -> Self {
        Self::of(self.shared.clone())
    }
}

impl Component for Terminal {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// **A terminal is called "terminal"** — what it is, never the text of a control drawn over it.
    /// The derived name is what its pick letter is remembered by, and the scroll button's count
    /// ("12 lines above") changes as you scroll, which moved the letter with it.
    fn text_summary(&self) -> Option<String> {
        Some("terminal".to_string())
    }

    /// **A hand over a link while the window's key is held** — the moment a click would open it.
    fn cursor_over(&self, point: Point) -> Option<heca_grid_ui::Cursor> {
        (self.shared.reserved(heca_grid_ui::event::modifiers())
            && self.link_at((point.x as f32, point.y as f32)).is_some())
        .then_some(heca_grid_ui::Cursor::Pointer)
    }

    /// **The terminal's picture, then what sits over it.** The cell glyphs are rasterised by the
    /// host into a texture it keeps (they are the hottest path in the app); all this says is which
    /// terminal and which box, and the host draws it there. The scrollback controls are drawn after
    /// it, in the same walk, so they are on top.
    fn paint(&self, cx: &mut PaintCx) {
        self.shared.bounds.set(self.base.bounds);
        // Which node is on screen, so the window can be asked whether it holds the keyboard, or
        // be told to give it the keyboard. Nothing about the keyboard is kept here: the window's
        // own focus is the one record of it.
        self.shared.node.set(self.base.focus_id());
        // Asked for the keyboard before it was on screen: take it now, like an `autofocus`.
        if self.shared.wants_keyboard.replace(false) {
            self.base.focus(false);
        }
        if let Some(id) = self.shared.id.get() {
            cx.surface(self.base.bounds, id.0);
        }
        self.shared
            .search
            .paint_highlights(cx, self.base.bounds.loc, self.shared.cell.get());
        for child in &self.base.children {
            heca_grid_ui::paint_child(child.as_ref(), cx);
        }
    }

    /// **What the user does to it while it holds the keyboard.**
    ///
    /// The pointer's moves are told to the program only while it holds the keyboard when an
    /// extension placed it, the rule a pane follows by being the focused pane: a program that
    /// tracks the mouse is not driven by a pointer merely passing over a panel. It reads its own
    /// flag, so a rebuilt tree cannot leave it believing something the framework no longer does.
    ///
    /// **Typing** reaches it like any widget: the tree delivers the key to the focus owner, so it
    /// needs no registration and no host surface. Text arrives as text, a key that is not text as a
    /// key with what was held; both go to whoever owns the process, which takes them (`Handled::Yes`)
    /// so nothing behind it also answers. Nothing is said when nobody has said where input goes.
    fn on_event(&mut self, ev: &heca_grid_ui::Event) -> heca_grid_ui::Handled {
        use heca_grid_ui::{Event, Handled};
        let said = match ev {
            Event::PointerMove(p) => {
                if self.shared.declared.borrow().is_none() || self.base.is_focused() {
                    self.shared.pointer_event(p, |p, cell| TerminalInput::Move {
                        cell,
                        modifiers: p.modifiers,
                    });
                }
                false
            }
            // **While the keyboard is in the search field, the field's keys are not the program's.**
            // What the field did not take bubbles here: Enter keeps the matches and gives the
            // keyboard to the terminal, Escape dismisses the search, and nothing else is typed
            // into the process behind the bar.
            Event::TextInput(_) | Event::Key { .. } if self.keyboard_is_in_search() => {
                if let Event::Key { key, pressed: true } = ev {
                    match key {
                        GridKey::Enter => self.base.focus(false),
                        GridKey::Escape => {
                            self.shared.search.close();
                            self.shared.emit(TerminalInput::Search(Search::Close));
                            self.base.focus(false);
                        }
                        _ => {}
                    }
                }
                true
            }
            // **What a user binds to find answers here** — to whichever terminal holds the keyboard.
            Event::Widget(WidgetIntent::Find) => {
                self.shared.search.open();
                true
            }
            Event::Widget(WidgetIntent::FindNext) => self.shared.step(true),
            Event::Widget(WidgetIntent::FindPrevious) => self.shared.step(false),
            // **Its own font size and its own history**: the terminal the keyboard is on answers,
            // wherever it is placed — the paging keys scroll it as they scroll any scroll area.
            Event::Widget(intent) => {
                if let Some(step) = crate::input::FontZoomStep::of_intent(*intent) {
                    self.shared.emit(TerminalInput::FontZoom(step))
                } else if let Some(page) = super::Page::of_intent(*intent) {
                    self.shared.emit(TerminalInput::Page(page))
                } else {
                    false
                }
            }
            Event::TextInput(text) => self.shared.emit(TerminalInput::Text(text.clone())),
            Event::Key { key, pressed: true } => self.shared.emit(TerminalInput::Key {
                key: *key,
                modifiers: heca_grid_ui::event::modifiers(),
            }),
            _ => false,
        };
        if said { Handled::Yes } else { Handled::No }
    }

    /// The box is known: say how big it is.
    fn on_layout(&mut self) {
        self.shared.size.set(Some(self.base.bounds.size));
        self.shared.report_grid();
    }
}

impl LayoutExt for Terminal {}

mod handle;

#[cfg(test)]
mod tests;
