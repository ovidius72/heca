//! [`Terminal`] — the component itself.

use std::cell::Cell;
use std::rc::Rc;

use heca_core::layout::Rectangle;
use heca_grid_ui::Size;

use super::model::TerminalId;
use heca_grid_ui::builders::LayoutExt;
use heca_grid_ui::component::{Base, Component, PaintCx};
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
}

/// **A terminal you can place anywhere** — the DOM's `<terminal/>`.
///
/// It fills whatever box it is put in, and learns that box's size from the layout, which is the
/// only thing it needs to know to size its grid. Nothing outside computes a rect for it.
///
/// `Terminal` is a handle: [`Clone`] gives another view of the **same** terminal, so the one
/// placed in a tree and the one kept by the caller agree at once.
pub(crate) struct Terminal {
    base: Base,
    shared: Rc<Shared>,
}

impl Terminal {
    /// A terminal. What it runs, and what it is called, arrive with the process it shows.
    pub(crate) fn new() -> Self {
        Self::of(Rc::new(Shared {
            id: Cell::new(None),
            size: Cell::new(None),
            placed: Cell::new(None),
        }))
    }

    fn of(shared: Rc<Shared>) -> Self {
        let mut base = Base::new();
        // It fills its parent, and says nothing more: a share of the box it is given, never a
        // measure of its own. Unset already stretches, but a leaf has no content to size it, so it
        // asks for the whole of the cell it sits in.
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Percent(1.0);
        Self { base, shared }
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

    /// **One command: where the terminal's picture goes.** The cell glyphs are rasterised by the
    /// host into a texture it keeps (they are the hottest path in the app); all this says is which
    /// terminal and which box, and the host draws it there.
    fn paint(&self, cx: &mut PaintCx) {
        if let Some(id) = self.shared.id.get() {
            cx.surface(self.base.bounds, id.0);
        }
    }

    /// The box is known: say how big it is.
    fn on_layout(&mut self) {
        self.shared.size.set(Some(self.base.bounds.size));
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
}
