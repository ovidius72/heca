//! What the pointer says to the terminal: the wheel, the buttons, the moves.

use super::*;

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
