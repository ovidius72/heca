use crate::builders::*;
use crate::component::Component;
use crate::widgets::{Flex, Label};

fn subtree() -> crate::WidgetModel {
    crate::WidgetModel::new(Label::new("realized"))
}

/// **A widget and an already-built subtree go through the SAME builder.**
///
/// `crate::WidgetModel` is not itself a `Component`, and for that one reason every
/// child-taking builder came in twos — `child`/`child_boxed`, `body`/`body_boxed`,
/// `leading`/`leading_boxed`, sixteen in all. A caller holding a dynamically built subtree,
/// which is what `realize()` returns from a description, had to know which spelling to reach
/// for; a widget author had to remember to write both, and a new slot that forgot its twin
/// was simply unreachable from the declarative side with nothing to report it.
#[test]
fn a_boxed_subtree_and_a_widget_take_the_same_builder() {
    let from_widget = Flex::column().child(Label::new("realized"));
    let from_boxed = Flex::column().child(subtree());
    assert_eq!(from_widget.base().children.len(), 1);
    assert_eq!(from_boxed.base().children.len(), 1);
    assert_eq!(
        from_boxed.base().children[0].text_summary(),
        from_widget.base().children[0].text_summary(),
        "the same subtree, whichever way it arrived",
    );
}

/// **A subtree that is already boxed is not boxed again.** The whole point of the second
/// builder was to avoid that, and the merged one must not give it back: `Box<Box<dyn …>>`
/// would still work and still walk, so nothing would fail — it would only cost an allocation
/// and a pointer hop per realized child, invisibly.
#[test]
fn an_already_boxed_subtree_is_handed_through_rather_than_wrapped() {
    let boxed = subtree();
    let addr = (&*boxed) as *const dyn Component as *const () as usize;
    let parent = Flex::column().child(boxed);
    let child = &parent.base().children[0];
    let after = (&**child) as *const dyn Component as *const () as usize;
    assert_eq!(addr, after, "the same allocation, not a box around a box");
}

/// The named slots take it too — a header, a body, a leading icon. One builder each.
#[test]
fn a_named_slot_takes_a_boxed_subtree_through_its_own_builder() {
    use crate::widgets::Item;
    let it = Item::new("row")
        .leading(subtree())
        .trailing(Label::new("x"));
    assert_eq!(it.base().children.len(), 3, "leading, label, trailing");
}
