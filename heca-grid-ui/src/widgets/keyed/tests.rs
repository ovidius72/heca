use super::*;
use crate::builders::{ComponentExt, Parent};
use crate::layout::LayoutEngine;
use crate::reactive::{Signal, SignalGet};
use crate::widgets::{Flex, Label};
use heca_core::layout::Size;

/// Content the way a caller builds it: a strip holding a label addressed by its `key`.
fn content() -> (Box<dyn Component>, Signal<String>) {
    let label = Label::new("old").key("word");
    let text = label.text_signal();
    (Box::new(Flex::row().child(label)), text)
}

/// The node placed in a tree, once, the way its owner places it.
fn placed(keyed: &Keyed) -> Flex {
    Flex::column()
        .width(300.0)
        .height(100.0)
        .child(keyed.clone())
}

fn lay_out(root: &mut Flex) {
    LayoutEngine::new().compute(root, Size::new(300.0, 100.0));
}

fn built_by(count: &Cell<u32>) -> impl FnOnce() -> Option<Box<dyn Component>> + '_ {
    move || {
        count.set(count.get() + 1);
        Some(content().0)
    }
}

/// **A frame where nothing changed builds nothing.** The same key again is the same content, and no
/// widget is made to find that out.
#[test]
fn the_same_key_again_builds_nothing() {
    let keyed = Keyed::new();
    let mut root = placed(&keyed);
    let built = Cell::new(0);

    assert!(keyed.show("a", built_by(&built)));
    lay_out(&mut root);
    for _ in 0..5 {
        assert!(!keyed.show("a", built_by(&built)));
        lay_out(&mut root);
    }

    assert_eq!(built.get(), 1, "built once, then left alone");
}

/// A new key builds new content, once.
#[test]
fn a_new_key_builds_once() {
    let keyed = Keyed::new();
    let mut root = placed(&keyed);
    let built = Cell::new(0);
    keyed.show("a", built_by(&built));
    lay_out(&mut root);

    assert!(keyed.show("b", built_by(&built)));
    assert!(!keyed.show("b", built_by(&built)));

    assert_eq!(built.get(), 2);
}

/// What was built is the node's child by the end of the layout that asked for it.
#[test]
fn the_content_arrives_in_the_layout_that_asked_for_it() {
    let keyed = Keyed::new();
    let mut root = placed(&keyed);
    keyed.show("a", || Some(content().0));

    lay_out(&mut root);

    assert_eq!(root.base().children[0].base().children.len(), 1);
}

/// **Words are written, not built.** A changed word lands on the tree already there: nothing is
/// rebuilt, and the widget says the new thing after the next layout.
#[test]
fn a_changed_word_is_written_without_a_rebuild() {
    let keyed = Keyed::new();
    let mut root = placed(&keyed);
    let text = std::cell::RefCell::new(None);
    keyed.show("a", || {
        let (tree, signal) = content();
        *text.borrow_mut() = Some(signal);
        Some(tree)
    });
    keyed.texts(vec![("word".into(), "one".into())]);
    lay_out(&mut root);
    let signal = text.borrow().expect("built");
    assert_eq!(signal.get_untracked(), "one");

    let rebuilt = keyed.show("a", || unreachable!("the key did not change"));
    keyed.texts(vec![("word".into(), "two".into())]);
    lay_out(&mut root);

    assert!(!rebuilt);
    assert_eq!(signal.get_untracked(), "two");
}

/// The same words again are not a change: nothing is written.
#[test]
fn the_same_words_again_are_not_rewritten() {
    let keyed = Keyed::new();
    keyed.texts(vec![("word".into(), "one".into())]);
    keyed.state.texts_dirty.set(false);

    keyed.texts(vec![("word".into(), "one".into())]);

    assert!(!keyed.state.texts_dirty.get());
}

/// A node placed anew holds no content, so the next `show` builds for it: a parent that was rebuilt
/// gets its child back.
#[test]
fn a_node_placed_anew_gets_content_on_the_next_show() {
    let keyed = Keyed::new();
    let mut first = placed(&keyed);
    let built = Cell::new(0);
    keyed.show("a", built_by(&built));
    lay_out(&mut first);

    let mut second = placed(&keyed);
    lay_out(&mut second);
    assert_eq!(
        second.base().children[0].base().children.len(),
        0,
        "nothing yet"
    );
    keyed.show("a", built_by(&built));
    lay_out(&mut second);

    assert_eq!(built.get(), 2);
    assert_eq!(second.base().children[0].base().children.len(), 1);
}

/// A build that makes nothing shows nothing — and is still the key shown, so it is not asked again.
#[test]
fn a_build_that_makes_nothing_is_not_asked_again() {
    let keyed = Keyed::new();
    let asked = Cell::new(0);
    for _ in 0..3 {
        keyed.show("none", || {
            asked.set(asked.get() + 1);
            None
        });
    }
    assert_eq!(asked.get(), 1);
}
