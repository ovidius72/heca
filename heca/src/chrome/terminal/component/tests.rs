//! The terminal component's tests, by what they test; the fixtures they share are here.

use super::super::input::Grid;
use super::super::testing::{header_above, laid_out_in, recording};
use super::super::viewport::ScrollIntents;
use super::*;
use heca_config::appearance::ScrollbarVisibility;
use heca_grid_ui::component::{Event, Handled};
use heca_grid_ui::event::PointerButton;
use heca_grid_ui::scene::{DrawCommand, HostDraw, Scene};
use heca_grid_ui::theme::Theme;

mod controls;
mod grid;
mod handle;
mod layout;
mod pointer;
mod typing;

/// Seams that ignore input — for tests of the scrollback controls.
fn seams(scroll: ScrollIntents) -> Seams {
    Seams {
        scroll,
        input: Box::new(|_| {}),
        command: Box::new(|_, _| {}),
    }
}

fn scrolled(offset: usize) -> Viewport {
    Viewport {
        rows: 24,
        scrollback_rows: 124,
        offset,
        scrollbar: ScrollbarVisibility::WhenNeeded,
        badge: true,
        cell: (10.0, 20.0),
        top_stable_row: 0,
        match_alpha: 64,
        current_match_alpha: 150,
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

/// A clicked terminal with an owner listening, ready to be typed into.
fn focused_terminal() -> (
    Box<dyn heca_grid_ui::Component>,
    Rc<RefCell<Vec<TerminalInput>>>,
) {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    let at = heca_grid_ui::Point::new(45.0, 61.0);
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(at, PointerButton::Left),
    );
    said.borrow_mut().clear();
    (root, said)
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
