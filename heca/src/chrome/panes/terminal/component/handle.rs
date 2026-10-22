//! **What a placed terminal's handle can do** — how it is declared, what it runs, what it is
//! called, its search, running a line and ending it, and what it says about where it was drawn.
//! The same handle a tree holds and its owner keeps: every method speaks to the shared terminal.

use heca_core::layout::Rectangle;

use super::super::input::{Cell as GridCell, Seams, TerminalCommand};
use super::super::model::TerminalId;
use super::super::shared::Declared;
use super::super::viewport::Viewport;
use super::Terminal;
use crate::app::backend_store::Program;

impl Terminal {
    /// **A terminal an extension places by name** — the one every handle of that name shares, so a
    /// dock built twice, or an overlay opened again, shows the same running process. Reached through
    /// `extension("demo").terminal("shell")`, never by naming the owner half yourself.
    pub(crate) fn declared(name: String) -> Self {
        super::super::declared::of(name)
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
    pub(in crate::chrome::panes::terminal) fn to_start(
        &self,
    ) -> Option<(String, Program, Option<std::path::PathBuf>)> {
        let declared = self.shared.declared.borrow();
        let declared = declared.as_ref().filter(|d| !d.resolved)?;
        Some((
            declared.name.clone(),
            declared.program.clone(),
            declared.cwd.clone(),
        ))
    }

    /// Whether the host has yet to start what was declared.
    pub(in crate::chrome::panes::terminal) fn is_waiting(&self) -> bool {
        self.shared
            .declared
            .borrow()
            .as_ref()
            .is_some_and(|d| !d.resolved)
    }

    /// The host started it (or could not): never start it again.
    pub(in crate::chrome::panes::terminal) fn resolved(&self) {
        if let Some(declared) = self.shared.declared.borrow_mut().as_mut() {
            declared.resolved = true;
        }
    }

    /// Make this handle's shared state a declared terminal named `name`.
    pub(in crate::chrome::panes::terminal) fn declare(&self, name: String) {
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
    pub(in crate::chrome::panes::terminal) fn bind(&self, seams: Seams) {
        *self.shared.intents.borrow_mut() = Some(seams);
    }

    /// Show how the terminal's viewport looks: the chip and the scrollbar follow it. Returns whether
    /// anything the user can see changed, so the caller can ask for a frame.
    pub(crate) fn show(&self, viewport: &Viewport) -> bool {
        self.shared.cell.set(viewport.cell);
        self.shared.nominal.set(viewport.nominal_cell);
        self.shared.link_modifier.set(viewport.open_link_modifier);
        self.shared
            .view
            .set((viewport.top_stable_row, viewport.rows));
        let search = &self.shared.search;
        let highlights_changed = search.set_view(viewport.top_stable_row, viewport.rows)
            | search.set_look(viewport.match_alpha, viewport.current_match_alpha);
        self.shared.controls.borrow_mut().show(viewport) | highlights_changed
    }

    /// **Open the search bar and put the keyboard in its field** — the handle's `find`. Works
    /// wherever the terminal is placed; the bar is its own.
    pub fn find(&self) {
        self.shared.search.open();
    }

    /// Whether the search bar is open — what a test asks, since the bar itself is the user's view.
    #[cfg(test)]
    pub(crate) fn search_open(&self) -> bool {
        self.shared.search.is_open()
    }

    /// **Dismiss the search** — the bar goes, and the matches and the query with it.
    pub fn close_search(&self) {
        self.shared.search.close();
    }

    /// **Go to the next match** of the search that is open.
    pub fn find_next(&self) {
        self.shared.step(true);
    }

    /// **Go to the previous match** of the search that is open.
    pub fn find_previous(&self) {
        self.shared.step(false);
    }

    /// **Move to the next or previous match**, wrapping. Returns whether it moved, so the caller
    /// can ask for a frame. The terminal holds the matches, so it is the one that steps.
    pub(crate) fn step_match(&self, forward: bool) -> bool {
        self.shared.search.step(forward)
    }

    /// The match the search is on, if any: where the caller scrolls to.
    pub(crate) fn current_match(&self) -> Option<heca_core::backend::SearchMatch> {
        self.shared.search.current_match()
    }

    /// **Show these matches**, `current` the focused one — what the owner found for the query the
    /// terminal said. Returns whether anything the user can see changed: the same result again
    /// changes nothing and asks for no frame.
    pub(crate) fn show_matches(
        &self,
        matches: Vec<heca_core::backend::SearchMatch>,
        current: Option<usize>,
    ) -> bool {
        self.shared.search.set_result(matches, current)
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
    ///
    /// An ended named terminal stays ended: the dock that placed it keeps showing the ended
    /// terminal, and the declared table keeps its entry, so asking for the same name again does not
    /// start another behind the user's back.
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
    pub(in crate::chrome::panes::terminal) fn attach(&self, id: TerminalId) {
        self.shared.id.set(Some(id));
    }

    /// Which terminal process this shows, once it has been said.
    pub(crate) fn id(&self) -> Option<TerminalId> {
        self.shared.id.get()
    }

    /// Say where it was drawn this frame, or that it was not. Said once a frame, by whoever reads
    /// the frame's surface requests.
    pub(in crate::chrome::panes::terminal) fn place(&self, rect: Option<Rectangle>) {
        self.shared.placed.set(rect);
    }

    /// Where it was drawn on the last frame it was on screen, in window coordinates.
    pub(crate) fn placed(&self) -> Option<Rectangle> {
        self.shared.placed.get()
    }

    /// **The window's identity of it on screen** — the node drawn on the last frame, which the
    /// window answers about ([`heca_grid_ui::keyboard_holder`]) and gives the keyboard to
    /// ([`heca_grid_ui::focus_widget_by_id`]). `None` when it was not drawn.
    pub(crate) fn node(&self) -> Option<u64> {
        self.shared.placed.get()?;
        Some(self.shared.node.get()).filter(|id| *id != 0)
    }

    /// **Take the window's keyboard** in `root`: at once when it is on screen, else the first time
    /// it is drawn.
    pub(crate) fn take_keyboard(&self, root: &mut dyn heca_grid_ui::Component) {
        let taken = self
            .node()
            .is_some_and(|node| heca_grid_ui::focus_widget_by_id(root, node));
        self.shared.wants_keyboard.set(!taken);
    }

    /// **The grid cell under `pos`**, in window coordinates — `None` when the terminal was not drawn
    /// last frame, the point is off its grid, or its cell size is not known yet. The same answer its
    /// own pointer handlers give, so a question about "the cell the mouse is on" has one source.
    pub(crate) fn cell_at(&self, pos: (f32, f32)) -> Option<GridCell> {
        self.shared.placed.get()?;
        let (cell_w, cell_h) = self.shared.cell.get();
        GridCell::at(
            (pos.0 as f64, pos.1 as f64),
            self.shared.content.get(),
            cell_w as f64,
            cell_h as f64,
        )
    }

    /// **Where cell `(row, col)` is on screen** (its top-left, window coordinates) — the inverse of
    /// [`cell_at`](Self::cell_at). `None` when the terminal was not drawn last frame.
    pub(crate) fn cell_origin(&self, row: usize, col: usize) -> Option<(f32, f32)> {
        self.shared.placed.get()?;
        let (cell_w, cell_h) = self.shared.cell.get();
        let at = self.shared.content.get().loc;
        Some((
            at.x as f32 + col as f32 * cell_w,
            at.y as f32 + row as f32 * cell_h,
        ))
    }

    /// **Say which hyperlinks are on screen** — the backend's capture for this frame.
    pub(crate) fn show_links(&self, links: &[heca_core::backend::HyperlinkSpan]) {
        let mut mine = self.shared.links.borrow_mut();
        if mine.as_slice() != links {
            *mine = links.to_vec();
        }
    }

    /// Every link on screen, in the order the backend captured them.
    pub(crate) fn links(&self) -> Vec<heca_core::backend::HyperlinkSpan> {
        self.shared.links.borrow().clone()
    }

    /// The target of the link under `pos` (window coordinates), if the terminal was drawn and the
    /// point is on one.
    pub(crate) fn link_at(&self, pos: (f32, f32)) -> Option<String> {
        let cell = self.cell_at(pos)?;
        self.shared.link_at(cell.row, cell.col)
    }

    /// The target of the link at a **stable-row** cell — the rows a selection keeps — if that row is
    /// on screen.
    pub(crate) fn link_at_stable(&self, stable_row: isize, col: usize) -> Option<String> {
        let (top, rows) = self.shared.view.get();
        let row = stable_row - top;
        if row < 0 || row >= rows as isize {
            return None;
        }
        self.shared.link_at(row as usize, col)
    }

    /// **Hold the grid in by per-side padding** — `(top, bottom, left, right)`,
    /// logical px. Said once by whoever reads config (each new terminal, and every
    /// terminal again on `prefix+Shift+r`); the component applies it at layout:
    /// the grid flows into the smaller box, the process is told fewer cells, and
    /// nothing is cropped. The default is no padding at all.
    pub(crate) fn set_content_padding(&self, top: f64, bottom: f64, left: f64, right: f64) {
        self.shared.padding.set((top, bottom, left, right));
    }

    /// **The grid's box, relative to where the terminal was placed** — where the
    /// padding holds it in (left, top) and how much room is left. What the render
    /// path needs to put the terminal where the scene asks and keep its cells
    /// aligned; `None` before the first layout.
    ///
    /// Both halves come from the layout alone (the padding and the grid's size),
    /// so the box never depends on when it was painted: the inset is the padding,
    /// whatever the frame.
    pub(crate) fn content_box(&self) -> Option<Rectangle> {
        let (top, _bottom, left, _right) = self.shared.padding.get();
        let size = self.shared.size.get()?;
        Some(Rectangle::new(
            heca_core::layout::Point::new(left, top),
            size,
        ))
    }

    /// **The surface's whole box, relative to where it is drawn** — (0, 0) and the
    /// full box the layout gave the terminal, padding included: the background's
    /// box, where the grid above sits held in. `None` before the first layout.
    pub(crate) fn full_box(&self) -> Option<Rectangle> {
        self.shared.size.get()?;
        let size = self.shared.bounds.get().size;
        Some(Rectangle::new(
            heca_core::layout::Point::new(0.0, 0.0),
            size,
        ))
    }
}
