//! [`Terminal`] — the component itself.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use heca_core::layout::Rectangle;
use heca_grid_ui::{Point, Size};

use super::input::{Cell as GridCell, Grid, Seams, TerminalCommand, TerminalInput};
use super::model::TerminalId;
use super::viewport::{Controls, IntentSlot, Placed, Viewport};
use crate::app::backend_store::Program;
use heca_grid_ui::builders::{ComponentExt, LayoutExt};
use heca_grid_ui::component::{Base, Component, PaintCx};
use heca_grid_ui::event::EventKind;
use heca_grid_ui::style::Length;

/// What a placed terminal and everyone holding it share: its name, and how much room it was given.
struct Shared {
    /// Which terminal process this shows, once whoever owns the processes has said.
    id: Cell<Option<TerminalId>>,
    /// The size of the box the layout gave it, written each time it is laid out. `None` until the
    /// first layout.
    size: Cell<Option<Size>>,
    /// Where it was drawn on the last frame it was on screen, in window coordinates — `None` when
    /// it was not drawn (scrolled out of view, or not in a tree). Said by whoever read the frame's
    /// surface requests.
    placed: Cell<Option<Rectangle>>,
    /// What a click on the scrollback controls means and where input goes, said once by whoever
    /// owns the terminal.
    intents: IntentSlot,
    /// One cell's size in logical pixels, as the last viewport said.
    cell: Cell<(f32, f32)>,
    /// The box the terminal was last painted in — what its cells are counted from.
    bounds: Cell<Rectangle>,
    /// The font's own cell size — what the grid is fitted from.
    nominal: Cell<(f32, f32)>,
    /// The grid last asked of the process. A new one is asked only when this changes.
    reported: Cell<Option<Grid>>,
    /// The scrollback controls of every node placed for this terminal.
    controls: RefCell<Placed>,
    /// What an extension asked of it, if it placed it by name (`demo.terminal("shell")`).
    declared: RefCell<Option<Declared>>,
}

/// **What an extension declared about a terminal it placed.** The name is the identity and never
/// changes (`demo.shell`); the title is only what the user reads, and defaults to the name.
pub(super) struct Declared {
    pub(super) name: String,
    pub(super) title: Option<String>,
    pub(super) program: Program,
    pub(super) cwd: Option<std::path::PathBuf>,
    /// Whether the host has already started (or failed to start) what was declared. Done once: a
    /// terminal that was later killed stays ended rather than starting again behind the user's back.
    pub(super) resolved: bool,
}

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
        // **So are the buttons and the pointer's moves**, but only when the terminal itself is what
        // they landed on: the chip and the scrollbar are its children, and a press or a move on
        // them is theirs. None of these takes the event — a window gesture (a divider, a move
        // modifier) decides about the same press on its own.
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
            let me = me.clone();
            move |cx| {
                me.pointer(cx, |p, cell| TerminalInput::Release {
                    button: p.button,
                    cell,
                    modifiers: p.modifiers,
                })
            }
        })
        .on(EventKind::PointerMove, move |cx| {
            me.pointer(cx, |p, cell| TerminalInput::Move {
                cell,
                modifiers: p.modifiers,
            })
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

impl Shared {
    /// Say `input` to the terminal's owner. `false` when nobody has said where it goes.
    fn emit(&self, input: TerminalInput) -> bool {
        match self.intents.borrow().as_ref() {
            Some(seams) => {
                (seams.input)(input);
                true
            }
            None => false,
        }
    }

    /// **Ask for the grid this box and font call for — once per change.** What it asked for last is
    /// kept, so a frame that changes nothing says nothing; a bound owner is told, an unbound one is
    /// not (and the grid stays unasked for, so it is said as soon as someone listens).
    fn report_grid(&self) {
        let Some(room) = self.size.get() else {
            return;
        };
        let Some(grid) = Grid::fit(room, self.nominal.get()) else {
            return;
        };
        if self.reported.get() == Some(grid) {
            return;
        }
        if self.emit(TerminalInput::Resize(grid)) {
            self.reported.set(Some(grid));
        }
    }

    /// The grid cell the pointer is on, if it is on the grid.
    fn cell_at(&self, p: &heca_grid_ui::PointerEvent) -> Option<GridCell> {
        let (cell_w, cell_h) = self.cell.get();
        GridCell::at(
            (p.pos.x, p.pos.y),
            self.bounds.get(),
            cell_w as f64,
            cell_h as f64,
        )
    }

    /// A button or the pointer, on the terminal itself: say what it did, and where in the grid.
    /// Anything whose target is a descendant (the scrollback controls) is not the terminal's.
    fn pointer(
        &self,
        cx: &mut heca_grid_ui::event::EventCx<'_>,
        make: impl Fn(&heca_grid_ui::PointerEvent, Option<GridCell>) -> TerminalInput,
    ) {
        let Some(p) = cx.pointer().copied() else {
            return;
        };
        if p.target_bounds != Some(self.bounds.get()) {
            return;
        }
        self.emit(make(&p, self.cell_at(&p)));
    }

    /// The wheel turned over the terminal.
    fn wheel(&self, cx: &mut heca_grid_ui::event::EventCx<'_>) {
        let Some(p) = cx.pointer().copied() else {
            return;
        };
        let cell = self.cell_at(&p);
        let said = self.emit(TerminalInput::Wheel {
            x: p.delta_x,
            y: p.delta_y,
            pixels: p.delta_pixels,
            cell,
            modifiers: p.modifiers,
        });
        if said {
            cx.stop_propagation();
        }
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

    /// The box is known: say how big it is.
    fn on_layout(&mut self) {
        self.shared.size.set(Some(self.base.bounds.size));
        self.shared.report_grid();
    }
}

impl LayoutExt for Terminal {}

#[cfg(test)]
mod tests {
    use super::super::testing::{header_above, laid_out_in};
    use super::*;
    use heca_grid_ui::scene::{DrawCommand, HostDraw, Scene};
    use heca_grid_ui::theme::Theme;

    /// A terminal in a box learns the box's size — the only thing it needs to size its grid.
    #[test]
    fn a_terminal_learns_the_size_of_the_box_it_is_given() {
        let t = Terminal::new();
        assert_eq!(t.room(), None, "nothing is known before the first layout");

        let _root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);

        let size = t.room().expect("laid out");
        assert_eq!((size.w, size.h), (300.0, 200.0));
    }

    /// Below a header the terminal gets what the header leaves — not the whole pane. That is the
    /// number the host used to compute by hand and got wrong for a tiled pane (two rows hidden
    /// under the header).
    #[test]
    fn below_a_header_a_terminal_gets_what_the_header_leaves() {
        let t = Terminal::new();
        let _root = header_above(Box::new(t.clone()), 33.0, 308.0, 720.0);

        let size = t.room().expect("laid out");
        assert_eq!((size.w, size.h), (308.0, 687.0));
    }

    /// It never exceeds the box it is given, whatever the header takes.
    #[test]
    fn a_terminal_never_exceeds_its_box() {
        for (w, h, header) in [
            (100.0, 50.0, 10.0),
            (308.0, 720.0, 33.0),
            (40.0, 40.0, 60.0),
        ] {
            let t = Terminal::new();
            let _root = header_above(Box::new(t.clone()), header, w, h);
            let size = t.room().expect("laid out");
            assert!(size.w <= w && size.h <= h, "{size:?} exceeds {w}x{h}");
        }
    }

    /// A clone is the same terminal: what one learns the other reports.
    #[test]
    fn a_clone_is_the_same_terminal() {
        let t = Terminal::new();
        let placed = t.clone();
        let _root = laid_out_in(Box::new(placed), 120.0, 80.0);
        assert_eq!(t.room().map(|s| (s.w, s.h)), Some((120.0, 80.0)));
    }

    /// Painting is one command, and it is the terminal's box below the header — not the pane's.
    #[test]
    fn a_terminal_paints_one_surface_at_its_own_box() {
        let t = Terminal::new();
        t.attach(TerminalId(7));
        let root = header_above(Box::new(t.clone()), 33.0, 308.0, 720.0);

        let theme = Theme::default();
        let mut scene = Scene::new();
        heca_grid_ui::paint_child(root.as_ref(), &mut PaintCx::new(&mut scene, &theme));

        let surfaces: Vec<_> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Host(h) => Some(*h),
                _ => None,
            })
            .collect();
        assert_eq!(surfaces.len(), 1, "one command, nothing else");
        assert_eq!(surfaces[0].draw, HostDraw::Surface { id: 7 });
        assert_eq!(
            (
                surfaces[0].rect.loc.x,
                surfaces[0].rect.loc.y,
                surfaces[0].rect.size.w,
                surfaces[0].rect.size.h
            ),
            (0.0, 33.0, 308.0, 687.0),
        );
    }

    /// A terminal nobody has named a process for has nothing to show, and says so by drawing
    /// nothing.
    #[test]
    fn a_terminal_with_no_process_paints_nothing() {
        let root = laid_out_in(Box::new(Terminal::new()), 100.0, 100.0);
        let theme = Theme::default();
        let mut scene = Scene::new();
        heca_grid_ui::paint_child(root.as_ref(), &mut PaintCx::new(&mut scene, &theme));
        assert!(scene.iter().all(|c| !matches!(c, DrawCommand::Host(_))));
    }
    // ── the scrollback controls ──

    use super::super::viewport::ScrollIntents;
    use heca_config::appearance::ScrollbarVisibility;
    use heca_grid_ui::component::{Event, Handled};
    use heca_grid_ui::event::PointerButton;

    /// Seams that ignore input — for tests of the scrollback controls.
    fn seams(scroll: ScrollIntents) -> Seams {
        Seams {
            scroll,
            input: Box::new(|_| {}),
            command: Box::new(|_, _| {}),
        }
    }

    /// Seams that keep what the terminal said.
    fn recording() -> (Seams, Rc<RefCell<Vec<TerminalInput>>>) {
        let said = Rc::new(RefCell::new(Vec::new()));
        let seen = said.clone();
        let seams = Seams {
            scroll: ScrollIntents {
                to_bottom: Box::new(|| {}),
                to_offset: Box::new(|_| {}),
            },
            input: Box::new(move |i| seen.borrow_mut().push(i)),
            command: Box::new(|_, _| {}),
        };
        (seams, said)
    }

    fn scrolled(offset: usize) -> Viewport {
        Viewport {
            rows: 24,
            scrollback_rows: 124,
            offset,
            scrollbar: ScrollbarVisibility::WhenNeeded,
            badge: true,
            cell: (10.0, 20.0),
            nominal_cell: (10.0, 20.0),
        }
    }

    fn painted(root: &dyn heca_grid_ui::Component) -> Scene {
        let theme = Theme::default();
        let mut scene = Scene::new();
        heca_grid_ui::paint_child(root, &mut PaintCx::new(&mut scene, &theme));
        scene
    }

    fn chip(scene: &Scene) -> Option<Rectangle> {
        scene.iter().find_map(|c| match c {
            DrawCommand::Text(t) if t.text.ends_with("above") => Some(t.rect),
            _ => None,
        })
    }

    /// At the live bottom there is nothing to show; scrolled up, the chip says how far.
    #[test]
    fn a_scrolled_terminal_says_how_far_above_the_live_bottom_it_is() {
        let t = Terminal::new();
        let root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        assert!(
            chip(&painted(root.as_ref())).is_none(),
            "live bottom: no chip"
        );

        assert!(t.show(&scrolled(5)), "showing a new viewport is a change");
        let mut root = root;
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
        let scene = painted(root.as_ref());
        let texts: Vec<_> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|x| x == "5 lines above"), "{texts:?}");
        assert!(!t.show(&scrolled(5)), "the same viewport changes nothing");
    }

    /// The chip stays inside the terminal's box whatever the box is.
    #[test]
    fn the_chip_never_leaves_the_terminal_it_belongs_to() {
        for (w, h) in [(120.0, 60.0), (300.0, 200.0), (900.0, 700.0)] {
            let t = Terminal::new();
            t.show(&scrolled(40));
            let root = laid_out_in(Box::new(t.clone()), w, h);
            let rect = chip(&painted(root.as_ref())).expect("chip is shown");
            assert!(
                rect.loc.x >= 0.0
                    && rect.loc.y >= 0.0
                    && rect.loc.x + rect.size.w <= w
                    && rect.loc.y + rect.size.h <= h,
                "{rect:?} leaves {w}x{h}"
            );
            assert!(
                rect.loc.x + rect.size.w > w - 80.0,
                "it sits at the right edge"
            );
        }
    }

    /// Clicking the chip means "back to the live bottom" — said to whoever bound the terminal.
    #[test]
    fn clicking_the_chip_asks_for_the_live_bottom() {
        let to_bottom = Rc::new(Cell::new(0));
        let t = Terminal::new();
        let seen = to_bottom.clone();
        t.bind(seams(ScrollIntents {
            to_bottom: Box::new(move || seen.set(seen.get() + 1)),
            to_offset: Box::new(|_| {}),
        }));
        t.show(&scrolled(5));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let rect = chip(&painted(root.as_ref())).expect("chip is shown");
        let at = heca_grid_ui::Point::new(
            rect.loc.x + rect.size.w / 2.0,
            rect.loc.y + rect.size.h / 2.0,
        );
        for ev in [
            Event::pointer_pressed(at, PointerButton::Left),
            Event::pointer_released(at, PointerButton::Left),
        ] {
            let _ = heca_grid_ui::dispatch(root.as_mut(), &ev);
        }
        assert_eq!(to_bottom.get(), 1);
    }

    /// Dragging the thumb asks to scroll to the row it points at.
    #[test]
    fn pressing_the_scrollbar_asks_to_scroll_to_where_it_points() {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let t = Terminal::new();
        let seen = asked.clone();
        t.bind(seams(ScrollIntents {
            to_bottom: Box::new(|| {}),
            to_offset: Box::new(move |rows| seen.borrow_mut().push(rows)),
        }));
        t.show(&scrolled(50));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        // The bar is on the right edge, under the chip; press in its upper half, below the chip.
        let at = heca_grid_ui::Point::new(296.0, 40.0);
        assert_eq!(
            heca_grid_ui::dispatch(
                root.as_mut(),
                &Event::pointer_pressed(at, PointerButton::Left)
            ),
            Handled::Yes
        );
        let rows = asked.borrow().last().copied().expect("asked to scroll");
        assert!(
            rows > 50,
            "the upper half of the track is far up the history: {rows}"
        );
    }

    /// A terminal with nothing to scroll shows neither control, so a press lands on the terminal.
    #[test]
    fn a_terminal_at_the_live_bottom_leaves_the_right_edge_to_the_terminal() {
        let t = Terminal::new();
        t.show(&scrolled(0));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let at = heca_grid_ui::Point::new(296.0, 100.0);
        assert_eq!(
            heca_grid_ui::dispatch(
                root.as_mut(),
                &Event::pointer_pressed(at, PointerButton::Left)
            ),
            Handled::No
        );
    }

    /// Every node placed for a terminal is shown the same viewport, so a rebuilt tree agrees with
    /// the one it replaced.
    #[test]
    fn every_node_placed_for_a_terminal_shows_the_same_viewport() {
        let t = Terminal::new();
        let first = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let second = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        t.show(&scrolled(9));
        for root in [first, second] {
            let mut root = root;
            heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
            assert!(chip(&painted(root.as_ref())).is_some());
        }
    }
    /// The wheel over a terminal is the terminal's own: it says where in the grid it turned, and
    /// nothing beneath it is offered the turn.
    #[test]
    fn the_wheel_says_which_cell_it_turned_over() {
        let (seams, said) = recording();
        let t = Terminal::new();
        t.bind(seams);
        t.show(&scrolled(0));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let _ = painted(root.as_ref()); // the terminal counts its cells from where it was painted
        let at = heca_grid_ui::Point::new(45.0, 61.0);
        let ev = Event::wheel(at, 0.0, 3.0);
        assert_eq!(heca_grid_ui::dispatch(root.as_mut(), &ev), Handled::Yes);
        // A trackpad's pixels ride along, so the owner can scroll by its own unit.
        let mut pad = heca_grid_ui::RawPointer::new(heca_grid_ui::RawPointerKind::Wheel, at);
        pad.delta_y = 0.7;
        pad.delta_pixels = Some((0.0, 14.0));
        assert_eq!(
            heca_grid_ui::dispatch(root.as_mut(), &Event::Raw(pad)),
            Handled::Yes
        );
        let said = pointer_input(&said);
        let [
            TerminalInput::Wheel { x, y, cell, .. },
            TerminalInput::Wheel { pixels, .. },
        ] = said.as_slice()
        else {
            panic!("two wheel messages, got {said:?}");
        };
        assert_eq!(*pixels, Some((0.0, 14.0)));
        assert_eq!((*x, *y), (0.0, 3.0));
        let cell = cell.expect("on the grid");
        assert_eq!((cell.row, cell.col), (3, 4));
    }

    /// A terminal nobody has bound lets the wheel carry on outward, rather than swallowing it.
    #[test]
    fn an_unbound_terminal_leaves_the_wheel_alone() {
        let mut root = laid_out_in(Box::new(Terminal::new()), 300.0, 200.0);
        let ev = Event::wheel(heca_grid_ui::Point::new(5.0, 5.0), 0.0, 1.0);
        assert_eq!(heca_grid_ui::dispatch(root.as_mut(), &ev), Handled::No);
    }
    /// A press, a release and a move on the terminal itself say where in the grid they were — and
    /// none of them takes the event, so a window gesture can still decide about the same press.
    #[test]
    fn buttons_and_moves_on_the_terminal_say_which_cell_and_leave_the_event_alone() {
        let (seams, said) = recording();
        let t = Terminal::new();
        t.bind(seams);
        t.show(&scrolled(0));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let _ = painted(root.as_ref());
        let at = heca_grid_ui::Point::new(45.0, 61.0);
        for ev in [
            Event::pointer_moved(at),
            Event::pointer_pressed(at, PointerButton::Right),
            Event::pointer_released(at, PointerButton::Right),
        ] {
            assert_eq!(heca_grid_ui::dispatch(root.as_mut(), &ev), Handled::No);
        }
        let said = pointer_input(&said);
        let want = Some(GridCell {
            row: 3,
            col: 4,
            x_offset: 5,
            y_offset: 1,
        });
        assert!(
            matches!(said[0], TerminalInput::Move { cell, .. } if cell == want),
            "{said:?}"
        );
        assert!(
            matches!(said[1], TerminalInput::Press { button: PointerButton::Right, cell, .. } if cell == want),
            "{said:?}"
        );
        assert!(
            matches!(said[2], TerminalInput::Release { button: PointerButton::Right, cell, .. } if cell == want),
            "{said:?}"
        );
    }

    /// The scrollback controls are the terminal's children: a press or a move on them is theirs,
    /// and the terminal says nothing about it — which is what keeps a hover on the chip from
    /// reaching the program underneath.
    #[test]
    fn a_press_or_move_on_the_chip_is_not_the_terminals() {
        let (seams, said) = recording();
        let t = Terminal::new();
        t.bind(seams);
        t.show(&scrolled(5));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let rect = chip(&painted(root.as_ref())).expect("chip is shown");
        let at = heca_grid_ui::Point::new(
            rect.loc.x + rect.size.w / 2.0,
            rect.loc.y + rect.size.h / 2.0,
        );
        for ev in [
            Event::pointer_moved(at),
            Event::pointer_pressed(at, PointerButton::Left),
            Event::pointer_released(at, PointerButton::Left),
        ] {
            let _ = heca_grid_ui::dispatch(root.as_mut(), &ev);
        }
        let heard = pointer_input(&said);
        assert!(heard.is_empty(), "{heard:?}");
    }
    /// The terminal answers "which cell is here" and "where is that cell" for everyone who asks,
    /// from the box it was drawn in — and says nothing when it was not drawn.
    #[test]
    fn the_terminal_says_which_cell_is_where() {
        let t = Terminal::new();
        t.show(&scrolled(0));
        let root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let _ = painted(root.as_ref());
        let on_screen = Rectangle::new(heca_grid_ui::Point::new(0.0, 0.0), Size::new(300.0, 200.0));
        t.place(Some(on_screen));
        assert_eq!(
            t.cell_at((45.0, 61.0)).map(|c| (c.row, c.col)),
            Some((3, 4))
        );
        assert_eq!(t.cell_at((301.0, 5.0)), None, "off the grid");
        assert_eq!(t.cell_origin(3, 4), Some((40.0, 60.0)));

        t.place(None);
        assert_eq!(t.cell_at((45.0, 61.0)), None, "not drawn: no cell");
        assert_eq!(t.cell_origin(3, 4), None);
    }
    /// The handle's `run` and `kill` say what they mean, about the terminal they belong to — and say
    /// nothing until there is a process to mean it about.
    #[test]
    fn the_handle_runs_and_kills_by_id() {
        let told: Rc<RefCell<Vec<(u64, TerminalCommand)>>> = Rc::default();
        let seen = told.clone();
        let t = Terminal::new();
        t.bind(Seams {
            scroll: ScrollIntents {
                to_bottom: Box::new(|| {}),
                to_offset: Box::new(|_| {}),
            },
            input: Box::new(|_| {}),
            command: Box::new(move |id, c| seen.borrow_mut().push((id.0, c))),
        });
        t.run("ls");
        assert!(
            told.borrow().is_empty(),
            "no process yet, so nothing to run in"
        );

        t.attach(TerminalId(5));
        t.run("ls -la");
        t.kill();
        assert_eq!(
            *told.borrow(),
            vec![
                (
                    5,
                    TerminalCommand::Run {
                        text: "ls -la".into(),
                        enter: true
                    }
                ),
                (5, TerminalCommand::Kill),
            ]
        );
    }
    /// What the pointer said, without the grid the terminal also asks for on its first layout.
    fn pointer_input(said: &Rc<RefCell<Vec<TerminalInput>>>) -> Vec<TerminalInput> {
        said.borrow()
            .iter()
            .filter(|i| !matches!(i, TerminalInput::Resize(_)))
            .cloned()
            .collect()
    }

    fn grids(said: &Rc<RefCell<Vec<TerminalInput>>>) -> Vec<Grid> {
        said.borrow()
            .iter()
            .filter_map(|i| match i {
                TerminalInput::Resize(g) => Some(*g),
                _ => None,
            })
            .collect()
    }

    /// A terminal asks for the grid its box and font call for, and asks again only when that
    /// changes — a frame that changes nothing says nothing.
    #[test]
    fn a_terminal_reports_its_grid_only_when_it_changes() {
        let (seams, said) = recording();
        let t = Terminal::new();
        t.bind(seams);
        t.show(&scrolled(0)); // cells are 10 x 20
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let first = grids(&said);
        assert_eq!(first.len(), 1, "{first:?}");
        assert_eq!((first[0].cols, first[0].rows), (30, 10));

        for _ in 0..3 {
            heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
        }
        assert_eq!(grids(&said).len(), 1, "the same box says nothing again");

        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));
        let after_resize = grids(&said);
        assert_eq!(after_resize.len(), 2);
        assert_eq!(after_resize[1].cols, 40);

        // A bigger font in the same box is a different grid too.
        let mut zoomed = scrolled(0);
        zoomed.nominal_cell = (20.0, 40.0);
        t.show(&zoomed);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));
        let after_zoom = grids(&said);
        assert_eq!(after_zoom.len(), 3);
        assert_eq!((after_zoom[2].cols, after_zoom[2].rows), (20, 5));
    }

    /// Nobody listening, nothing is lost: the grid is said as soon as someone binds.
    #[test]
    fn a_grid_nobody_heard_is_said_when_someone_listens() {
        let t = Terminal::new();
        t.show(&scrolled(0));
        let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        let (seams, said) = recording();
        t.bind(seams);
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
        assert_eq!(grids(&said).len(), 1);
    }

    /// Before the font is measured there is no grid to ask for.
    #[test]
    fn no_grid_is_asked_for_before_the_cell_size_is_known() {
        let (seams, said) = recording();
        let t = Terminal::new();
        t.bind(seams);
        let _root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
        assert!(grids(&said).is_empty());
    }
}
