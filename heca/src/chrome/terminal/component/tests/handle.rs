//! The handle: what a caller asks of a terminal by its id.

use super::*;

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

/// **The window is asked whether a terminal holds the keyboard**: the terminal says which node of
/// it is on screen, and the window says whether that node holds the keyboard. One not drawn has no
/// node on screen, so it is never the answer.
#[test]
fn the_window_says_whether_a_terminal_on_screen_holds_the_keyboard() {
    let t = Terminal::new();
    t.bind(recording().0);
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    t.place(Some(root.base().bounds));
    assert_ne!(
        heca_grid_ui::keyboard_holder(root.as_ref()),
        t.node(),
        "not focused yet"
    );

    heca_grid_ui::dispatch(
        root.as_mut(),
        &Event::pointer_pressed(heca_grid_ui::Point::new(45.0, 61.0), PointerButton::Left),
    );
    assert!(t.node().is_some());
    assert_eq!(
        heca_grid_ui::keyboard_holder(root.as_ref()),
        t.node(),
        "a click gives it the keyboard"
    );

    t.place(None);
    assert_eq!(t.node(), None, "not drawn: nothing of it on screen");
}

/// **A terminal told to take the keyboard takes it** — at once when it is on screen, and the first
/// time it is drawn when it is not yet: a pane's terminal is told when the pane becomes the focused
/// one, which may be before its first frame.
#[test]
fn a_terminal_told_to_take_the_keyboard_takes_it_on_screen_or_once_drawn() {
    let other = Terminal::new();
    let t = Terminal::new();
    for terminal in [&other, &t] {
        terminal.bind(recording().0);
        terminal.show(&scrolled(0));
    }
    let mut root = laid_out_in(Box::new(other.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    other.place(Some(root.base().bounds));
    other.take_keyboard(root.as_mut());
    assert_eq!(
        heca_grid_ui::keyboard_holder(root.as_ref()),
        other.node(),
        "on screen: at once"
    );

    // Not drawn yet: it takes the keyboard when it is.
    t.take_keyboard(root.as_mut());
    assert_eq!(heca_grid_ui::keyboard_holder(root.as_ref()), other.node());
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    heca_grid_ui::settle_focus(root.as_mut());
    t.place(Some(root.base().bounds));
    assert_eq!(
        heca_grid_ui::keyboard_holder(root.as_ref()),
        t.node(),
        "once drawn"
    );
}
