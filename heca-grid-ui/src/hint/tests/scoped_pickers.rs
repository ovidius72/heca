//! **Two verbs over one tree** — the failure scopes exist to stop.
//!
//! A surface can mean more than one thing by a letter: the exposé's cards mean *go there* and
//! their ⊠ icons mean *remove that*. One picker over both hands out twice the letters, and half of
//! them delete what you meant to jump to.

use crate::builders::{HintExt, Parent};
use crate::component::Component;
use crate::reactive::{SignalGet, SignalUpdate};
use crate::widgets::{Button, Flex, KeyHintGroup, Label};

/// Real bounds, because a target nobody can see is not a target — a zero-sized widget is
/// dropped before it can be lettered, which would make every assertion below pass for the
/// wrong reason.
fn lay_out(root: &mut dyn Component) {
    crate::LayoutEngine::new().compute(root, heca_core::layout::Size::new(800.0, 600.0));
}

/// What each lettered target says, so an assertion reads as the letters a user would see.
fn labels(root: &dyn Component) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn walk(node: &dyn Component, out: &mut Vec<String>) {
    if node.base().hint_label.get_untracked().is_some() {
        out.push(node.text_summary().unwrap_or_default());
    }
    for child in &node.base().children {
        walk(child.as_ref(), out);
    }
}

/// ⚠️ Both buttons declare `on_hint` rather than relying on being actionable, because the two
/// walks do not agree on what a target is: the global one takes anything actionable, a
/// surface-owned picker takes only explicit declarations. That difference is older than scopes
/// and is not changed here — but with scopes in place the restriction is redundant,
/// and a surface's picker silently ignoring an ordinary button is worth closing.
fn card_and_close() -> Flex {
    Flex::column()
        .child(Button::new("card").on_hint(|| {}))
        .child(Button::new("close").on_hint(|| {}).hint_scope(["close"]))
}

/// **Naming a scope takes a target out of the ordinary picker.** This is the property the
/// whole thing rests on: the ⊠ must not wear a letter in the picker you jump with.
#[test]
fn a_scoped_target_is_not_in_the_ordinary_picker() {
    let mut tree = card_and_close();
    lay_out(&mut tree);
    let targets = crate::collect_hints(&tree);
    assert_eq!(
        targets.len(),
        1,
        "the card is lettered; the scoped close button is not",
    );
}

/// …and the scoped picker letters its own set, and nothing else.
#[test]
fn a_scoped_picker_letters_only_what_named_it() {
    let mut group = KeyHintGroup::new(card_and_close()).scope("close");
    lay_out(&mut group);
    group.open_signal().set(true);
    group.tick(0.0);
    let labelled = labels(&group);
    assert_eq!(labelled, vec!["close"], "only the ⊠, never the card");
}

/// An unscoped picker keeps behaving exactly as every picker does today.
#[test]
fn an_unscoped_picker_letters_the_unscoped_targets() {
    let mut group = KeyHintGroup::new(card_and_close());
    lay_out(&mut group);
    group.open_signal().set(true);
    group.tick(0.0);
    assert_eq!(
        labels(&group),
        vec!["card"],
        "the card, and not the scoped ⊠"
    );
}

/// **A picker whose scope matches nothing shows no letters** — it does not fall back to
/// lettering everything. The fallback is the dangerous direction: a "close" picker that
/// silently lettered every card would remove what you meant to go to.
#[test]
fn a_picker_that_matches_nothing_letters_nothing() {
    let mut group = KeyHintGroup::new(card_and_close()).scope("nobody");
    lay_out(&mut group);
    group.open_signal().set(true);
    group.tick(0.0);
    assert!(labels(&group).is_empty(), "fail closed, never open");
}

/// **A picker does not walk into another picker's subtree** — and the target inside names no
/// scope, so *only* the stop keeps it out.
///
/// That shape is the whole point: a nested picker's children are ordinary targets from their
/// own picker's point of view, and the scope filter has nothing to say about them. Written the
/// other way — with the inner target scoped — the filter alone excludes it and the test passes
/// with the stop deleted, which is exactly what sabotaging it revealed.
#[test]
fn a_picker_stops_at_another_pickers_subtree() {
    let inner = KeyHintGroup::new(Button::new("theirs").on_hint(|| {})).scope("other");
    let mut outer = KeyHintGroup::new(
        Flex::column()
            .child(Button::new("mine").on_hint(|| {}))
            .child(inner),
    );
    lay_out(&mut outer);
    outer.open_signal().set(true);
    outer.tick(0.0);
    assert_eq!(
        labels(&outer),
        vec!["mine"],
        "the nested picker keeps its own"
    );
}

/// The ordinary picker stops there too — the same rule, the same reason.
#[test]
fn the_ordinary_picker_stops_at_a_scoped_picker() {
    let mut tree = Flex::column()
        .child(Button::new("mine").on_click(|| {}))
        .child(KeyHintGroup::new(Button::new("theirs").on_hint(|| {})).scope("other"));
    lay_out(&mut tree);
    assert_eq!(
        crate::collect_hints(&tree).len(),
        1,
        "only the page's own target"
    );
}

/// **A surface's own picker letters an ordinary button** — the same rule the global one
/// follows, because it is now literally the same question.
///
/// The two walks had drifted: `hint::is_target` takes anything actionable, and this one asked
/// only whether a declaration was present. So "being pickable is not opt-in" held for
/// `prefix+/` and not for a picker a plugin owns — it could put a panel of buttons behind its
/// own verb and get no letters, with nothing to say why.
#[test]
fn a_surfaces_own_picker_letters_an_ordinary_button() {
    let mut group = KeyHintGroup::new(
        Flex::column()
            .child(Button::new("act").on_click(|| {}))
            .child(Label::new("just text")),
    );
    lay_out(&mut group);
    group.open_signal().set(true);
    group.tick(0.0);
    assert_eq!(
        labels(&group),
        vec!["act"],
        "actionable is enough here too; a Label has nothing to run",
    );
}

/// A target may answer to several pickers.
#[test]
fn a_target_can_belong_to_more_than_one_picker() {
    let button = Button::new("both")
        .on_click(|| {})
        .hint_scope(["close", "other"]);
    // Asked through what a picker actually calls, not the scope test underneath it.
    assert!(crate::hint::collect::is_target_of(&button, Some("close")));
    assert!(crate::hint::collect::is_target_of(&button, Some("other")));
    assert!(
        !crate::hint::collect::is_target_of(&button, None),
        "still out of the ordinary picker"
    );
}
