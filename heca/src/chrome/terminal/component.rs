//! [`Terminal`] — the component itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::Rectangle;
use heca_grid_ui::{Point, Size};

use super::input::{Cell as GridCell, Seams, TerminalCommand, TerminalInput};
use super::model::TerminalId;
use super::shared::{Declared, Shared};
use super::viewport::{Controls, IntentSlot, Viewport};
use crate::app::backend_store::Program;
use heca_grid_ui::builders::{ComponentExt, LayoutExt};
use heca_grid_ui::component::{Base, Component, PaintCx};
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
        .on(EventKind::PointerDown, {
            let me = me.clone();
            move |cx| {
                me.pointer(cx, |p, cell| TerminalInput::Press {
                    button: p.button,
                    cell,
                    modifiers: p.modifiers,
                })
            }
        })
        .on(EventKind::PointerUp, {
            move |cx| {
                me.pointer(cx, |p, cell| TerminalInput::Release {
                    button: p.button,
                    cell,
                    modifiers: p.modifiers,
                })
            }
        })
    }

    /// **A terminal an extension places by name** — the one every handle of that name shares, so a
    /// dock built twice, or an overlay opened again, shows the same running process. Reached through
    /// `extension("demo").terminal("shell")`, never by naming the owner half yourself.
    pub(crate) fn declared(name: String) -> Self {
        super::declared::of(name)
    }

    /// **Run this command instead of the user's shell**, under the shell so job control and rc behave
    /// as in a pane. Fixed when the terminal starts: a running terminal's program does not change.
    pub fn command(self, command: impl Into<String>) -> Self {
        if let Some(declared) = self.shared.declared.borrow_mut().as_mut() {
            declared.program = Program::Command(command.into());
        }
        self
    }

    /// **Start in this folder** (heca's own when unset). Fixed when the terminal starts.
    pub fn cwd(self, folder: impl Into<std::path::PathBuf>) -> Self {
        if let Some(declared) = self.shared.declared.borrow_mut().as_mut() {
            declared.cwd = Some(folder.into());
        }
        self
    }

    /// **What the user reads** as this terminal's name; it defaults to the name it was declared
    /// with. Only a title: the name is the identity and does not change.
    pub fn title(self, title: impl Into<String>) -> Self {
        if let Some(declared) = self.shared.declared.borrow_mut().as_mut() {
            declared.title = Some(title.into());
        }
        self
    }

    /// The title to show: the one given, else the name; `None` for a terminal that was not
    /// declared by an extension.
    pub fn shown_title(&self) -> Option<String> {
        self.shared
            .declared
            .borrow()
            .as_ref()
            .map(|d| d.title.clone().unwrap_or_else(|| d.name.clone()))
    }

    /// The declaration the host should start, if it has not yet.
    pub(super) fn to_start(&self) -> Option<(String, Program, Option<std::path::PathBuf>)> {
        let declared = self.shared.declared.borrow();
        let declared = declared.as_ref().filter(|d| !d.resolved)?;
        Some((
            declared.name.clone(),
            declared.program.clone(),
            declared.cwd.clone(),
        ))
    }

    /// Whether the host has yet to start what was declared.
    pub(super) fn is_waiting(&self) -> bool {
        self.shared
            .declared
            .borrow()
            .as_ref()
            .is_some_and(|d| !d.resolved)
    }

    /// The host started it (or could not): never start it again.
    pub(super) fn resolved(&self) {
        if let Some(declared) = self.shared.declared.borrow_mut().as_mut() {
            declared.resolved = true;
        }
    }

    /// Make this handle's shared state a declared terminal named `name`.
    pub(super) fn declare(&self, name: String) {
        *self.shared.declared.borrow_mut() = Some(Declared {
            name,
            title: None,
            program: Program::Shell,
            cwd: None,
            resolved: false,
        });
    }

    /// Say what the terminal's owner wants of it: what a click on the scrollback controls means,
    /// and where its input goes. Said once, by whoever owns the terminal.
    pub(super) fn bind(&self, seams: Seams) {
        *self.shared.intents.borrow_mut() = Some(seams);
    }

    /// Show how the terminal's viewport looks: the chip and the scrollbar follow it. Returns whether
    /// anything the user can see changed, so the caller can ask for a frame.
    pub(crate) fn show(&self, viewport: &Viewport) -> bool {
        self.shared.cell.set(viewport.cell);
        self.shared.nominal.set(viewport.nominal_cell);
        self.shared.controls.borrow_mut().show(viewport)
    }

    /// **Type a line into this terminal and press Enter** — the handle's `run`. Does nothing until the
    /// terminal shows a process.
    pub fn run(&self, text: &str) {
        self.tell(TerminalCommand::Run {
            text: text.to_string(),
            enter: true,
        });
    }

    /// **End this terminal** — the handle's `kill`.
    pub fn kill(&self) {
        self.tell(TerminalCommand::Kill);
    }

    fn tell(&self, command: TerminalCommand) {
        let Some(id) = self.shared.id.get() else {
            return;
        };
        if let Some(seams) = self.shared.intents.borrow().as_ref() {
            (seams.command)(id, command);
        }
    }

    /// Say which terminal process this shows. Said once, by whoever owns the processes.
    pub(super) fn attach(&self, id: TerminalId) {
        self.shared.id.set(Some(id));
    }

    /// Which terminal process this shows, once it has been said.
    pub(crate) fn id(&self) -> Option<TerminalId> {
        self.shared.id.get()
    }

    /// Say where it was drawn this frame, or that it was not. Said once a frame, by whoever reads
    /// the frame's surface requests.
    pub(super) fn place(&self, rect: Option<Rectangle>) {
        self.shared.placed.set(rect);
    }

    /// Where it was drawn on the last frame it was on screen, in window coordinates.
    pub(crate) fn placed(&self) -> Option<Rectangle> {
        self.shared.placed.get()
    }

    /// **The grid cell under `pos`**, in window coordinates — `None` when the terminal was not drawn
    /// last frame, the point is off its grid, or its cell size is not known yet. The same answer its
    /// own pointer handlers give, so a question about "the cell the mouse is on" has one source.
    pub(crate) fn cell_at(&self, pos: (f32, f32)) -> Option<GridCell> {
        self.shared.placed.get()?;
        let (cell_w, cell_h) = self.shared.cell.get();
        GridCell::at(
            (pos.0 as f64, pos.1 as f64),
            self.shared.bounds.get(),
            cell_w as f64,
            cell_h as f64,
        )
    }

    /// **Where cell `(row, col)` is on screen** (its top-left, window coordinates) — the inverse of
    /// [`cell_at`](Self::cell_at). `None` when the terminal was not drawn last frame.
    pub(crate) fn cell_origin(&self, row: usize, col: usize) -> Option<(f32, f32)> {
        self.shared.placed.get()?;
        let (cell_w, cell_h) = self.shared.cell.get();
        let at = self.shared.bounds.get().loc;
        Some((
            at.x as f32 + col as f32 * cell_w,
            at.y as f32 + row as f32 * cell_h,
        ))
    }

    /// How much room the layout gave it the last time it was laid out.
    pub(crate) fn room(&self) -> Option<Size> {
        self.shared.size.get()
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

    /// **The terminal's picture, then what sits over it.** The cell glyphs are rasterised by the
    /// host into a texture it keeps (they are the hottest path in the app); all this says is which
    /// terminal and which box, and the host draws it there. The scrollback controls are drawn after
    /// it, in the same walk, so they are on top.
    fn paint(&self, cx: &mut PaintCx) {
        self.shared.bounds.set(self.base.bounds);
        if let Some(id) = self.shared.id.get() {
            cx.surface(self.base.bounds, id.0);
        }
        for child in &self.base.children {
            heca_grid_ui::paint_child(child.as_ref(), cx);
        }
    }

    /// **Where the pointer is, told to the program** — only while it holds the keyboard when an
    /// extension placed it, the rule a pane follows by being the focused pane: a program that
    /// tracks the mouse is not driven by a pointer merely passing over a panel. It reads its own
    /// flag, so a rebuilt tree cannot leave it believing something the framework no longer does.
    fn on_event(&mut self, ev: &heca_grid_ui::Event) -> heca_grid_ui::Handled {
        if let heca_grid_ui::Event::PointerMove(p) = ev
            && (self.shared.declared.borrow().is_none() || self.base.is_focused())
        {
            self.shared.pointer_event(p, |p, cell| TerminalInput::Move {
                cell,
                modifiers: p.modifiers,
            });
        }
        heca_grid_ui::Handled::No
    }

    /// The box is known: say how big it is.
    fn on_layout(&mut self) {
        self.shared.size.set(Some(self.base.bounds.size));
        self.shared.report_grid();
    }
}

impl LayoutExt for Terminal {}

#[cfg(test)]
mod tests;
