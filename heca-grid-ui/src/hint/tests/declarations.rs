use crate::builders::HintExt as _;
use crate::component::Component;
use crate::hint::*;
use crate::reactive::SignalUpdate;
use crate::widgets::{Flex, KeyHint, Surface};
use heca_core::layout::Rectangle;
use heca_core::layout::{Point, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// A hinting wrapper with laid-out bounds, appending `tag` to `log` when it is picked.
fn hint_at(
    tag: &'static str,
    log: &Rc<RefCell<Vec<&'static str>>>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> crate::WidgetModel {
    let log = log.clone();
    let mut wrapper = KeyHint::new(Surface::new()).on_hint(move || log.borrow_mut().push(tag));
    wrapper.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
    crate::WidgetModel::new(wrapper)
}

/// **The widget that DECLARED the pick gets the letter — not something merely actionable
/// inside it.** Both pickers must land on the same widget, or the same target wears the letter
/// in two different places: a declaration carries its own placement, so labelling a different
/// node silently moves the keycap.
///
/// heca's case: a sidebar pane row carries the identity and is actionable (`on_activate`),
/// wrapped in a `KeyHint` that carries the declaration and `CenterRight`. Labelling by key used
/// to take the row — so the same pane showed a right-aligned cap under `prefix+/` and a
/// top-centred one under `prefix+q`.
#[test]
fn a_declaration_outranks_mere_actionability_when_offering_by_key() {
    use crate::builders::{EventExt, HintExt, IdentityExt};
    use crate::reactive::SignalGet;

    let inner = Surface::new().key("pane:7").on_click(|_| {});
    let wrapper = KeyHint::new(inner).on_hint(|| {});
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(wrapper));

    assert!(offer_hint_by_key(&root, "pane:7", Some("a".into())));

    let wrapper = &root.base().children[0];
    assert_eq!(
        wrapper.base().hint_label.get_untracked().as_deref(),
        Some("a"),
        "the letter must go to the widget that declared the pick"
    );
    assert!(
        wrapper.base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .is_none(),
        "the merely-actionable node inside must not also wear it"
    );
}

/// The other direction still works: a container that names itself on the OUTSIDE and declares
/// the pick on a child — heca's mounted docks — must letter the child.
#[test]
fn a_declaration_inside_the_named_node_still_wins() {
    use crate::builders::{HintExt, IdentityExt};
    use crate::reactive::SignalGet;

    let declaring = KeyHint::new(Surface::new()).on_hint(|| {});
    let mut named = Flex::column().key("workspaces");
    named.base_mut().children.push(crate::WidgetModel::new(declaring));
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(named));

    assert!(offer_hint_by_key(&root, "workspaces", Some("b".into())));
    assert_eq!(
        root.base().children[0].base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .as_deref(),
        Some("b")
    );
}

/// **A nested identity owns its own letter.**
///
/// The search inside a named node exists for a container that names itself on the outside and
/// declares its pick within. But a child that names something *else* is a different thing, and
/// its declaration says what a pick does to **it** — so handing it the parent's letter draws
/// the parent's keycap on the child, in the child's place and the child's colour.
///
/// heca's case: a workspace dock names itself and declares its pick on the wrapper *outside*
/// it, so the search went in and found the first pane row's declaration. Picking a workspace to
/// move a pane into put the workspace's letter over a pane — blue and right-aligned instead of
/// orange on the workspace's own header.
#[test]
fn a_letter_never_crosses_into_something_that_names_itself() {
    use crate::builders::{EventExt, HintExt, IdentityExt};
    use crate::reactive::SignalGet;

    // A workspace that names itself, declares its pick on the wrapper AROUND it, and holds a
    // pane row that names itself and declares its own pick.
    let row = KeyHint::new(Surface::new().key("pane:7").on_click(|_| {})).on_hint(|| {});
    let mut workspace = Flex::column().key("ws:1");
    workspace.base_mut().children.push(crate::WidgetModel::new(row));
    let outer = KeyHint::new(workspace).on_hint(|| {});
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(outer));

    assert!(offer_hint_by_key(&root, "ws:1", Some("f".into())));

    let outer = &root.base().children[0];
    assert_eq!(
        outer.base().hint_label.get_untracked().as_deref(),
        Some("f"),
        "the workspace's own declaration must draw the workspace's letter",
    );
    let row = &outer.base().children[0].base().children[0];
    assert!(
        row.base().hint_label.get_untracked().is_none(),
        "the pane row inside is a different thing — it must not wear the workspace's letter",
    );
}

/// **A container keeps its own letter whether it holds one thing or many.**
///
/// Under `prefix+/`, the workspaces dock wore a letter with two workspaces in it and none with
/// one. Nothing about the dock changed — only how many things
/// were inside it.
///
/// The cause was the rule that stops a wrapper and what it wraps both spending a letter. It
/// decided "this is a mere layer of the thing below" by **counting children** — exactly one
/// meant wrapper — so with a single workspace the chain ran unbroken from the dock down to the
/// workspace row, which declares a pick and replaced the dock's entry. A parent counting its
/// children to work out what it is is the pattern this codebase forbids everywhere else.
///
/// A node answers for itself now (`Base::transparent`, set by `wrap_transparently`), so shape
/// decides nothing.
#[test]
fn a_real_container_keeps_its_letter_however_many_children_it_holds() {
    use crate::builders::{EventExt, HintExt, IdentityExt};

    // A dock that is actionable but declares nothing — heca's focus wrapper — holding a list
    // with `n` workspace rows, each of which names itself and declares its own pick.
    let dock_with = |rows: usize| {
        let mut list = Flex::column();
        for i in 0..rows {
            let row = KeyHint::new(Surface::new().key(format!("ws:{i}")).on_click(|_| {}))
                .on_hint(|| {});
            list.base_mut().children.push(crate::WidgetModel::new(row));
        }
        let mut dock = Flex::column().key("workspaces").on_click(|_| {});
        dock.base_mut().children.push(crate::WidgetModel::new(list));
        let mut root = Flex::column();
        root.base_mut().children.push(crate::WidgetModel::new(dock));
        root
    };

    let named = |root: &Flex| -> Vec<String> {
        collect_hints(root)
            .into_iter()
            .filter_map(|(path, _)| crate::nav::identity_of(root, &path))
            .collect()
    };

    let one = named(&dock_with(1));
    let two = named(&dock_with(2));
    assert!(
        one.iter().any(|n| n == "workspaces"),
        "the dock is a target with one row in it: {one:?}",
    );
    assert!(
        two.iter().any(|n| n == "workspaces"),
        "…and with two, which is the half that already worked: {two:?}",
    );
    assert!(
        one.iter().any(|n| n.ends_with("ws:0")),
        "and the row inside still gets its own — a name nests under its container: {one:?}",
    );
}

/// **A scope keeps a target out of the ordinary picker without making it unreachable.**
///
/// A scope answers *which collected picker may letter this*. Naming a target outright is not a
/// collection — the host has already decided what it means — so the two questions are separate.
///
/// Without the separation the workspaces dock could not be both things it needs to be: a
/// keyboard destination `prefix+Shift+e` letters by name, and not something `prefix+/` spends
/// one of its 52 on. Scoping it removed it from both at once, and the dock pick went dark.
#[test]
fn a_scoped_target_is_still_reachable_when_it_is_named() {
    use crate::builders::{EventExt, HintExt, IdentityExt};
    use crate::reactive::SignalGet;

    let dock = Surface::new()
        .key("workspaces")
        .on_click(|_| {})
        .hint_scope(["dock.destination"]);
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(dock));

    let collected: Vec<String> = collect_hints(&root)
        .into_iter()
        .filter_map(|(path, _)| crate::nav::identity_of(&root, &path))
        .collect();
    assert!(
        collected.is_empty(),
        "the ordinary picker does not letter a target that named a scope: {collected:?}",
    );

    assert!(
        offer_hint_by_key(&root, "workspaces", Some("a".into())),
        "…and naming it outright still reaches it — that is not a collection",
    );
    assert_eq!(
        root.base().children[0]
            .base()
            .hint_label
            .get_untracked()
            .as_deref(),
        Some("a"),
    );
}

/// **A wrapper and what it wraps still spend only one letter.** The rule the child count was
/// standing in for, kept — a transparent wrapper says so, so the pair is still one thing.
#[test]
fn a_transparent_wrapper_does_not_spend_a_letter_of_its_own() {
    use crate::builders::{EventExt, HintExt, IdentityExt};

    let inner = Surface::new().key("pane:7").on_click(|_| {});
    let wrapped = KeyHint::new(inner).on_hint(|| {});
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(wrapped));

    let targets = collect_hints(&root);
    assert_eq!(
        targets.len(),
        1,
        "the wrapper and the widget inside it are one thing, so one letter",
    );
}

/// The pane inside still gets its **own** letter, by its own name. The boundary stops a letter
/// crossing into somebody else; it does not make a nested target unreachable.
#[test]
fn a_nested_identity_still_gets_the_letter_that_is_its_own() {
    use crate::builders::{EventExt, HintExt, IdentityExt};
    use crate::reactive::SignalGet;

    let row = KeyHint::new(Surface::new().key("pane:7").on_click(|_| {})).on_hint(|| {});
    let mut workspace = Flex::column().key("ws:1");
    workspace.base_mut().children.push(crate::WidgetModel::new(row));
    let outer = KeyHint::new(workspace).on_hint(|| {});
    let mut root = Flex::column();
    root.base_mut().children.push(crate::WidgetModel::new(outer));

    assert!(offer_hint_by_key(&root, "pane:7", Some("j".into())));

    let outer = &root.base().children[0];
    let row = &outer.base().children[0].base().children[0];
    assert_eq!(
        row.base().hint_label.get_untracked().as_deref(),
        Some("j"),
        "the pane's own letter reaches the pane's own declaration",
    );
    assert!(
        outer.base().hint_label.get_untracked().is_none(),
        "and the workspace around it does not take it",
    );
}

#[test]
fn collects_declarations_with_bounds_in_document_order() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut root = Flex::column();
    root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
    root.base_mut()
        .children
        .push(hint_at("first", &log, 0.0, 0.0, 100.0, 40.0));
    root.base_mut()
        .children
        .push(hint_at("second", &log, 0.0, 50.0, 100.0, 40.0));

    let targets = collect_hints(&root);
    assert_eq!(
        targets
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        vec![vec![0], vec![1]],
        "document order, addressed by path"
    );
    assert_eq!(
        targets[1].1.loc.y, 50.0,
        "…each with the rect its letter goes over"
    );

    assert!(fire_hint(&mut root, &targets[1].0));
    assert_eq!(*log.borrow(), vec!["second"]);
}

/// **A button gets its letter for being a button — whatever it was put inside.**
///
/// Where a widget sits must never decide what it can do (AGENTS § 0d). The picker used to break
/// that: a container saying what picking *it* did switched off letters for everything beneath,
/// so a button in a pane's bar had to repeat its own click as a hint to win one back, and the
/// control the bar builds for itself had nobody to do that and silently wore none.
#[test]
fn a_button_inside_a_declaring_container_still_gets_its_own_letter() {
    use crate::widgets::Button;

    let log = Rc::new(RefCell::new(Vec::new()));
    let mut region = Flex::column();
    region.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
    // The container says what picking IT does — a sidebar row, a pane's shell.
    let tag = "region";
    let seen = log.clone();
    region = region.on_hint(move || seen.borrow_mut().push(tag));
    // …and it holds two things of its own, one of which is an ordinary button.
    let mut label = Surface::new();
    label.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 40.0));
    let mut button = Button::new("Close").on_click(|| {});
    button.base_mut().bounds = Rectangle::new(Point::new(0.0, 50.0), Size::new(40.0, 40.0));
    region.base_mut().children.push(crate::WidgetModel::new(label));
    region.base_mut().children.push(crate::WidgetModel::new(button));

    let targets = collect_hints(&region);
    assert_eq!(
        targets.len(),
        2,
        "the container and the button are two different things, so two letters"
    );
    assert!(
        targets.iter().any(|(path, _)| path == &vec![1]),
        "the button is one of them, having declared nothing at all"
    );
}

/// **A wrapper and the one thing it holds are one thing, and share one letter.**
///
/// The case the old rule existed for, and the only one it was right about: a decorator saying
/// what picking does, around a card that can itself be activated, produced two letters for one
/// card. Counting things keeps that fixed while leaving real containers alone.
#[test]
fn a_wrapper_and_the_one_thing_it_holds_share_a_letter() {
    use crate::widgets::Button;

    let log = Rc::new(RefCell::new(Vec::new()));
    let seen = log.clone();
    let mut card = Button::new("Card").on_click(|| {});
    card.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0));
    let mut wrapper = KeyHint::new(card).on_hint(move || seen.borrow_mut().push("wrapper"));
    wrapper.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0));

    let targets = collect_hints(&wrapper);
    assert_eq!(
        targets.len(),
        1,
        "one card, one letter — not one per layer wrapping it"
    );
    // The wrapper is the layer that *declared*, and a declaration is the statement that a pick
    // is not the click — so it is the one the letter must run.
    assert!(
        targets[0].0.is_empty(),
        "the declaring layer keeps it, since it is what says a pick differs from a click"
    );
    assert!(fire_hint(&mut wrapper, &targets[0].0));
    assert_eq!(
        *log.borrow(),
        vec!["wrapper"],
        "and picking runs what it said"
    );
}

#[test]
fn skips_hidden_subtrees_and_silent_nodes() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut root = Flex::column();
    root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
    // A silent parent holding a declaring child, then a hidden declaring node.
    let mut silent_parent = Flex::column();
    silent_parent
        .base_mut()
        .children
        .push(hint_at("nested", &log, 0.0, 0.0, 10.0, 10.0));
    root.base_mut().children.push(crate::WidgetModel::new(silent_parent));

    let mut hidden = KeyHint::new(Surface::new()).on_hint(|| unreachable!("hidden"));
    hidden.base_mut().visible.set(false);
    root.base_mut().children.push(crate::WidgetModel::new(hidden));

    let targets = collect_hints(&root);
    assert_eq!(
        targets
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        vec![vec![0, 0]],
        "a silent parent is transparent; a hidden declaration is skipped"
    );
}
