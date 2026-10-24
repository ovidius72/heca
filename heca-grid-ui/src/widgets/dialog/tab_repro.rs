use super::*;
use crate::component::GridKey;
use crate::widgets::{Button, Label};

/// **The first Tab moves the keyboard**.
///
/// It did not. Opening a dialog quietly focused its first button through a `FocusManager` the
/// **dialog** owned, while Tab was answered by the one the composed **overlay** owns — two
/// positions over one panel. The first press moved the overlay's manager to *its* first
/// control, which was the button the keyboard was already on, so nothing appeared to happen
/// and only the second press moved.
///
/// One manager now, on the overlay, which is what holds the keyboard. The dialog keeps none
/// and delegates all four operations — opening, Tab, arrow motion, and a click inside the
/// panel. Collapsing only *one* of them is what broke the arrow-key traversal on the first
/// attempt: the halves have to move together or they disagree in a new place instead.
#[test]
fn the_first_tab_in_a_dialog_moves_off_the_default_button() {
    let mut d = Dialog::new("Close pane?")
        .body(Label::new("This action cannot be undone."))
        .action(Button::new("Cancel"))
        .action(Button::new("Close"))
        .default_open(true);
    crate::component::dispatch(
        &mut d,
        &Event::Key {
            key: GridKey::Tab,
            pressed: true,
        },
    );

    fn focused(n: &dyn Component, out: &mut Vec<String>) {
        if n.base().is_focused()
            && let Some(name) = n.text_summary()
        {
            out.push(name);
        }
        for c in &n.base().children {
            focused(c.as_ref(), out);
        }
    }
    let mut names = Vec::new();
    focused(&d, &mut names);

    assert!(
        names.iter().any(|n| n == "Close"),
        "one Tab moved off the button the dialog opened on, got {names:?}",
    );
    assert!(
        !names.iter().any(|n| n == "Cancel"),
        "and left it, rather than lighting both: {names:?}",
    );
}
