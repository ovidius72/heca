use crate::builders::{IdentityExt, Parent};
use crate::hint::*;
use crate::widgets::{Button, Flex, FocusScope, Input};

fn holder(root: &dyn crate::component::Component) -> Option<Vec<usize>> {
    crate::keyboard_owner(root).map(|owner| owner.path)
}

/// **Anything that holds the keyboard is lettered, and picking it gives it the keyboard** — a
/// text field, a terminal, whatever is made next — exactly as a press on it would.
#[test]
fn a_text_field_is_lettered_and_picking_it_gives_it_the_keyboard() {
    let mut root = Flex::column().child(Input::new());
    let hints = collect_hints(&root);
    assert_eq!(hints.len(), 1, "the field has a letter");
    assert!(fire_hint(&mut root, &hints[0].0));
    assert_eq!(
        holder(&root),
        Some(vec![0]),
        "and holds the keyboard after the pick"
    );
}

/// **Inside a region that keeps the keyboard, a pick leaves it with the region** — picking a
/// sidebar row moves its cursor; the keys that follow still go to the sidebar.
#[test]
fn picking_a_row_in_a_region_that_keeps_the_keyboard_gives_it_to_the_region() {
    let mut root = Flex::row().child(
        FocusScope::new(Flex::column().child(Button::new("row").on_click(|| {})))
            .keeps_keyboard(true)
            .scope_key("rows"),
    );
    let hints = collect_hints(&root);
    assert_eq!(hints.len(), 1);
    assert!(fire_hint(&mut root, &hints[0].0));
    assert_eq!(holder(&root), Some(vec![0]));
}
