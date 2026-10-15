use super::*;
use crate::builders::Parent;
use crate::component::{Base, Event, Handled};
use crate::reactive::SignalUpdate;
use crate::widgets::Flex;

/// A widget the keyboard can land on.
struct Control {
    base: Base,
}

impl Control {
    fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true;
        Self { base }
    }
}

impl Component for Control {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_event(&mut self, _: &Event) -> Handled {
        Handled::No
    }
}

/// Two controls side by side; answers the identity of each.
fn window() -> (Flex, u64, u64) {
    let root = Flex::row().child(Control::new()).child(Control::new());
    let a = root.base().children[0].base().focus_id();
    let b = root.base().children[1].base().focus_id();
    (root, a, b)
}

/// **The keyboard goes to the widget named by its identity, from whoever had it**, and the
/// answer to "who holds it" is that identity.
#[test]
fn a_widget_given_the_keyboard_by_identity_holds_it() {
    let (mut root, a, b) = window();
    assert_eq!(keyboard_holder(&root), None);

    assert!(focus_widget_by_id(&mut root, a));
    assert_eq!(keyboard_holder(&root), Some(a));

    assert!(focus_widget_by_id(&mut root, b));
    assert_eq!(
        keyboard_holder(&root),
        Some(b),
        "taken from the one that had it"
    );
    assert!(!root.base().children[0].base().is_focused());
}

/// A widget that is hidden, or not in the tree at all, takes nothing, and the keyboard stays where
/// it was.
#[test]
fn a_hidden_or_missing_widget_takes_nothing() {
    let (mut root, a, b) = window();
    assert!(focus_widget_by_id(&mut root, a));
    root.base().children[1].base().visible.set(false);

    assert!(!focus_widget_by_id(&mut root, b), "hidden");
    assert!(!focus_widget_by_id(&mut root, u64::MAX), "missing");
    assert_eq!(keyboard_holder(&root), Some(a));
}
