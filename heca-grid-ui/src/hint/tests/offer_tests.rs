use crate::builders::{HintExt, IdentityExt, Parent};
use crate::component::Component;
use crate::hint::*;
use crate::reactive::SignalGet;
use crate::widgets::{Flex, KeyHint, Label, Row};

/// **The pick is on the PARENT** — a row names itself and the `KeyHint` wrapping it says what a
/// pick does. This is every sidebar row, and searching only downward from the named node left
/// all of them dark under `prefix+q` while the docks lit correctly.
#[test]
fn a_named_row_wrapped_in_a_keyhint_is_letterable() {
    let tree = Flex::column()
        .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}));

    assert!(offer_hint_by_key(&tree, "pane:7", Some("a".into())));
    let hint = &tree.base().children[0];
    assert_eq!(hint.base().hint_label.get_untracked().as_deref(), Some("a"));
}

/// **The pick is on a CHILD** — a mounted dock names itself on the outer wrapper and declares
/// the pick within. The other direction, from the same one call.
#[test]
fn a_named_wrapper_declaring_its_pick_within_is_letterable() {
    let tree = Flex::column().child(
        Flex::column()
            .scope_key("workspaces")
            .child(KeyHint::new(Label::new("dock")).on_hint(|| {})),
    );

    assert!(offer_hint_by_key(&tree, "workspaces", Some("b".into())));
    let inner = &tree.base().children[0].base().children[0];
    assert_eq!(
        inner.base().hint_label.get_untracked().as_deref(),
        Some("b")
    );
}

/// **The same target shown twice must be lettered twice** — one pane listed in the left
/// sidebar *and* the right one is two places you can pick it, and both wear the letter.
///
/// The same component in the left and the right sidebar wears letters in both.
#[test]
fn a_target_listed_in_two_places_is_lettered_in_both() {
    let tree = Flex::column()
        .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}))
        .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}));

    assert!(offer_hint_by_key(&tree, "pane:7", Some("a".into())));
    let left = &tree.base().children[0];
    let right = &tree.base().children[1];
    assert_eq!(
        left.base().hint_label.get_untracked().as_deref(),
        Some("a"),
        "the first one"
    );
    assert_eq!(
        right.base().hint_label.get_untracked().as_deref(),
        Some("a"),
        "and the second — the walk must not stop at the first match"
    );
}

/// A name nobody declares, and a name with no pick anywhere around it, are both "not
/// letterable" — never a keycap that does nothing.
#[test]
fn a_target_with_no_pick_is_not_letterable() {
    let tree = Flex::column().child(Row::new().key("pane:9"));
    assert!(!offer_hint_by_key(&tree, "pane:9", Some("c".into())));
    assert!(!offer_hint_by_key(&tree, "nothing", Some("c".into())));
}
