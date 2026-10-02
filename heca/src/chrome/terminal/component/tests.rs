use super::super::input::Grid;
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

/// **A terminal an extension placed hears the pointer move only while it holds the keyboard** —
/// the rule a pane follows by being the focused pane. A click gives it the keyboard (the
/// framework's rule for every focusable); until then a pointer passing over a panel does not
/// drive the program's mouse tracking.
#[test]
fn a_placed_terminal_hears_the_pointer_move_only_while_it_holds_the_keyboard() {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.declare("demo.pad".into());
    t.bind(seams);
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    let at = heca_grid_ui::Point::new(45.0, 61.0);

    heca_grid_ui::dispatch(root.as_mut(), &Event::pointer_moved(at));
    assert!(pointer_input(&said).is_empty(), "not focused: nothing said");

    // A click on it gives it the keyboard — nothing declared anywhere.
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(at, PointerButton::Left),
    );
    heca_grid_ui::dispatch(root.as_mut(), &Event::pointer_moved(at));
    let said = pointer_input(&said);
    assert!(
        said.iter().any(|i| matches!(i, TerminalInput::Move { .. })),
        "focused: the move is said {said:?}"
    );
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

/// **Typing reaches a terminal like any widget**: click it, and what is typed is said to its owner —
/// text as text, so a shifted symbol arrives as the character, and the terminal takes it so nothing
/// behind it answers too. Nothing is declared on it and nothing is routed by the host.
#[test]
fn typed_text_reaches_the_terminal_that_was_clicked() {
    let (mut root, said) = focused_terminal();

    let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("@".into()));

    assert_eq!(taken, Handled::Yes);
    assert_eq!(*said.borrow(), vec![TerminalInput::Text("@".into())]);
}

/// A key that is not text arrives as a key, with what was held when it went down — Ctrl+C is the
/// key `c` and the Ctrl that was down, not a character.
#[test]
fn a_key_that_is_not_text_reaches_it_with_what_was_held() {
    use heca_grid_ui::{GridKey, Modifiers};
    let (mut root, said) = focused_terminal();
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };

    heca_grid_ui::dispatch(root.as_mut(), &Event::ModifiersChanged(ctrl));
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Key {
            key: GridKey::Char('c'),
            pressed: true,
        },
    );
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::ModifiersChanged(Modifiers::default()),
    );
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::Key {
            key: GridKey::ArrowUp,
            pressed: true,
        },
    );

    assert_eq!(
        *said.borrow(),
        vec![
            TerminalInput::Key {
                key: GridKey::Char('c'),
                modifiers: ctrl
            },
            TerminalInput::Key {
                key: GridKey::ArrowUp,
                modifiers: Modifiers::default()
            },
        ]
    );
}

/// **A program in a docked terminal gets its paging keys**: the tree gives the key to the terminal
/// first, and only what it leaves is the dock's.
#[test]
fn the_paging_keys_reach_the_program_in_the_terminal() {
    use heca_grid_ui::{GridKey, Modifiers};
    let (mut root, said) = focused_terminal();

    for key in [
        GridKey::PageUp,
        GridKey::PageDown,
        GridKey::Home,
        GridKey::End,
    ] {
        let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::Key { key, pressed: true });
        assert_eq!(taken, Handled::Yes, "{key:?}");
    }

    assert_eq!(
        said.borrow()[0],
        TerminalInput::Key {
            key: GridKey::PageUp,
            modifiers: Modifiers::default()
        }
    );
    assert_eq!(said.borrow().len(), 4);
}

/// Nothing focused, nothing delivered: a terminal nobody clicked hears no typing, and a key coming
/// *up* is not typing.
#[test]
fn a_terminal_that_does_not_hold_the_keyboard_hears_no_typing() {
    use heca_grid_ui::GridKey;
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);

    let taken = heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".into()));
    assert_eq!(taken, Handled::No);

    let (mut focused, said_focused) = focused_terminal();
    heca_grid_ui::dispatch(
        focused.as_mut(),
        &Event::Key {
            key: GridKey::Enter,
            pressed: false,
        },
    );
    assert!(pointer_input(&said).is_empty());
    assert!(said_focused.borrow().is_empty(), "a release is not typing");
}

/// A terminal with no owner listening says nothing and takes nothing, so what it did not take
/// bubbles on to whatever is behind it.
#[test]
fn a_terminal_nobody_listens_to_takes_nothing() {
    let t = Terminal::new();
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    let at = heca_grid_ui::Point::new(45.0, 61.0);
    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(at, PointerButton::Left),
    );

    assert_eq!(
        heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".into())),
        Handled::No
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
