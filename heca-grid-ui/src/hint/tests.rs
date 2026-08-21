//! The rules of the picker, held as tests rather than prose.

#[cfg(test)]
mod declarations {
    use crate::hint::*;
    use heca_core::layout::Rectangle;
    use crate::component::Component;
    use crate::builders::ComponentExt as _;
    use crate::reactive::SignalUpdate;
    use crate::widgets::{Flex, KeyHint, Surface};
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
    ) -> Box<dyn Component> {
        let log = log.clone();
        let mut wrapper = KeyHint::new(Surface::new()).on_hint(move || log.borrow_mut().push(tag));
        wrapper.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
        Box::new(wrapper)
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
        use crate::builders::ComponentExt;
        use crate::reactive::SignalGet;

        let inner = Surface::new().key("pane:7").on_click(|_| {});
        let wrapper = KeyHint::new(inner).on_hint(|| {});
        let mut root = Flex::column();
        root.base_mut().children.push(Box::new(wrapper));

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
        use crate::builders::ComponentExt;
        use crate::reactive::SignalGet;

        let declaring = KeyHint::new(Surface::new()).on_hint(|| {});
        let mut named = Flex::column().key("workspaces");
        named.base_mut().children.push(Box::new(declaring));
        let mut root = Flex::column();
        root.base_mut().children.push(Box::new(named));

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
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0], vec![1]],
            "document order, addressed by path"
        );
        assert_eq!(targets[1].1.loc.y, 50.0, "…each with the rect its letter goes over");

        assert!(fire_hint(&mut root, &targets[1].0));
        assert_eq!(*log.borrow(), vec!["second"]);
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
        root.base_mut().children.push(Box::new(silent_parent));

        let mut hidden = KeyHint::new(Surface::new()).on_hint(|| unreachable!("hidden"));
        hidden.base_mut().visible.set(false);
        root.base_mut().children.push(Box::new(hidden));

        let targets = collect_hints(&root);
        assert_eq!(
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0, 0]],
            "a silent parent is transparent; a hidden declaration is skipped"
        );
    }
}

#[cfg(test)]
mod actionable_tests {
    use crate::hint::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::{Button, Flex, Label};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// **Being pickable is not opt-in** (F003/P082/T441). A button gets a letter for having a click
    /// handler, and nothing else. Before this, a widget had to be wrapped in a `KeyHint` and given
    /// an `on_hint`, which is why `prefix+/` showed a curated handful and felt not worth having.
    #[test]
    fn anything_actionable_is_lettered_with_nothing_declared() {
        let tree = Flex::column()
            .child(Button::new("Restart").on_click(|| {}))
            .child(Button::new("Stop").on_click(|| {}).hintable(false))
            .child(Label::new("just text"));

        let targets = collect_hints(&tree);
        assert_eq!(
            targets.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
            vec![vec![0]],
            "the plain button is a target; `hintable(false)` is not, and a Label has nothing to run"
        );
    }

    /// **Picking an ordinary widget does what clicking it does.** The letter over a button promises
    /// the button's own behaviour, so it is delivered as a real `Click` through the handler that is
    /// already there — not a second path that could drift from what the mouse does.
    #[test]
    fn picking_an_ordinary_button_runs_its_click() {
        let ran = Rc::new(RefCell::new(0));
        let seen = ran.clone();
        let mut tree =
            Flex::column().child(Button::new("Restart").on_click(move || *seen.borrow_mut() += 1));

        let targets = collect_hints(&tree);
        assert_eq!(targets.len(), 1);
        assert!(fire_hint(&mut tree, &targets[0].0));
        assert_eq!(*ran.borrow(), 1, "the button's own click handler ran");
    }

}

/// **The pick is an event, and it behaves like one** (F003/P082/T432).
#[cfg(test)]
mod pick_delivery_tests {
    use crate::hint::*;
    use crate::builders::{ComponentExt, Parent};
    use crate::event::EventKind;
    use crate::widgets::{Flex, Row};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<String>>>;

    fn log() -> Log {
        Rc::new(RefCell::new(Vec::new()))
    }

    fn note(log: &Log, what: &'static str) -> impl Fn() + 'static {
        let log = log.clone();
        move || log.borrow_mut().push(what.to_string())
    }

    /// **A pick acts on the widget it named, and on nothing else.**
    ///
    /// The case it exists for: a pane that is itself pickable and holds pickable rows. Without the
    /// target phase the pane's declaration fires too and you land on the pane instead of the row —
    /// and every such container would hand-write the DOM's `e.target !== e.currentTarget` guard.
    ///
    /// It diverges from a click on purpose: clicking a child of a clickable box **is** clicking the
    /// box, because the pointer is over both. A pick is nominal, not spatial.
    #[test]
    fn only_the_picked_widget_runs_its_own_declaration() {
        let log = log();
        let mut tree = Flex::column().child(
            Row::new()
                .on_hint(note(&log, "the pane"))
                .child(Row::new().key("pane:7").on_hint(note(&log, "the row"))),
        );

        let targets = collect_hints(&tree);
        let row = targets
            .iter()
            .find(|(path, _)| path.len() == 2)
            .expect("the nested row is its own target");

        assert!(fire_hint(&mut tree, &row.0));
        assert_eq!(
            *log.borrow(),
            vec!["the row"],
            "the container it is inside must not answer for it"
        );
    }

    /// **A container can watch picks from its children** — the delegation seam, which is the
    /// ordinary bubbled event. It declares nothing, so it gets no letter of its own, and it learns
    /// **which** widget was picked from the event.
    #[test]
    fn a_listening_container_sees_which_child_was_picked() {
        let seen: Rc<RefCell<Vec<Option<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let heard = seen.clone();
        let mut tree = Flex::column().child(
            Row::new()
                .on(EventKind::Hint, move |e| {
                    if let crate::event::Event::Hint(h) = e.event() {
                        heard.borrow_mut().push(h.key.clone());
                    }
                })
                .child(Row::new().key("pane:7").on_hint(|| {}))
                .child(Row::new().key("pane:9").on_hint(|| {})),
        );

        let targets = collect_hints(&tree);
        assert_eq!(targets.len(), 2, "the listener declares nothing, so it gets no letter");

        assert!(fire_hint(&mut tree, &targets[1].0));
        assert_eq!(
            *seen.borrow(),
            vec![Some("pane:9".to_string())],
            "the container hears the pick and can tell which row it was"
        );
    }

    /// **A listener can stop the pick going further up** — the same lever every other event has.
    ///
    /// Note what it does *not* undo: the widget that was picked has already acted, because handlers
    /// bubble and the target phase came first. That is the point of the phases, not a gap in them —
    /// a pick is aimed at one widget, and an ancestor watching it is watching, not vetoing. What
    /// `stop_propagation` decides is whether **the ancestors above** hear it.
    #[test]
    fn a_listener_can_stop_a_pick_going_further_up() {
        let log = log();
        let inner = log.clone();
        let outer = log.clone();
        let mut tree = Flex::column().child(
            Row::new()
                .on(EventKind::Hint, move |_| {
                    outer.borrow_mut().push("the outer container".into())
                })
                .child(
                    Row::new()
                        .on(EventKind::Hint, move |e| {
                            inner.borrow_mut().push("the inner container".into());
                            e.stop_propagation();
                        })
                        .child(Row::new().key("pane:7").on_hint(note(&log, "the row"))),
                ),
        );

        let targets = collect_hints(&tree);
        assert_eq!(targets.len(), 1, "only the row declares anything");
        assert!(fire_hint(&mut tree, &targets[0].0));
        assert_eq!(
            *log.borrow(),
            vec!["the row", "the inner container"],
            "the target acted, the nearest listener heard it, and the outer one did not"
        );
    }
}

#[cfg(test)]
mod offer_tests {
    use crate::hint::*;
    use crate::reactive::SignalGet;
    use crate::component::Component;
    use crate::builders::{ComponentExt, Parent};
    use crate::widgets::{Flex, KeyHint, Label, Row};

    /// **The pick is on the PARENT** — a row names itself and the `KeyHint` wrapping it says what a
    /// pick does. This is every sidebar row, and searching only downward from the named node left
    /// all of them dark under `prefix+q` while the docks lit correctly.
    #[test]
    fn a_named_row_wrapped_in_a_keyhint_is_letterable() {
        let tree = Flex::column().child(
            KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}),
        );

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
        assert_eq!(inner.base().hint_label.get_untracked().as_deref(), Some("b"));
    }

    /// **The same target shown twice must be lettered twice** — one pane listed in the left
    /// sidebar *and* the right one is two places you can pick it, and both wear the letter.
    ///
    /// Antonio, driving, 2026-08-17: *"prefix+q, m, M, t, T show letters on the left sidebar but not
    /// on the right one. WHY? they are the same component so nothing should be adapted."*
    #[test]
    fn a_target_listed_in_two_places_is_lettered_in_both() {
        let tree = Flex::column()
            .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}))
            .child(KeyHint::new(Row::new().key("pane:7").child(Label::new("zsh"))).on_hint(|| {}));

        assert!(offer_hint_by_key(&tree, "pane:7", Some("a".into())));
        let left = &tree.base().children[0];
        let right = &tree.base().children[1];
        assert_eq!(left.base().hint_label.get_untracked().as_deref(), Some("a"), "the first one");
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
}
