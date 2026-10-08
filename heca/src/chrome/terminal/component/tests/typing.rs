//! Typing: what reaches the program while the terminal holds the keyboard.

use super::*;

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
