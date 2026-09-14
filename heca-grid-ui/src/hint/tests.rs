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
    /// orange on the workspace's own header (Antonio, driving, screenshot).
    #[test]
    fn a_letter_never_crosses_into_something_that_names_itself() {
        use crate::builders::ComponentExt;
        use crate::reactive::SignalGet;

        // A workspace that names itself, declares its pick on the wrapper AROUND it, and holds a
        // pane row that names itself and declares its own pick.
        let row = KeyHint::new(Surface::new().key("pane:7").on_click(|_| {})).on_hint(|| {});
        let mut workspace = Flex::column().key("ws:1");
        workspace.base_mut().children.push(Box::new(row));
        let outer = KeyHint::new(workspace).on_hint(|| {});
        let mut root = Flex::column();
        root.base_mut().children.push(Box::new(outer));

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

    /// The pane inside still gets its **own** letter, by its own name. The boundary stops a letter
    /// crossing into somebody else; it does not make a nested target unreachable.
    #[test]
    fn a_nested_identity_still_gets_the_letter_that_is_its_own() {
        use crate::builders::ComponentExt;
        use crate::reactive::SignalGet;

        let row = KeyHint::new(Surface::new().key("pane:7").on_click(|_| {})).on_hint(|| {});
        let mut workspace = Flex::column().key("ws:1");
        workspace.base_mut().children.push(Box::new(row));
        let outer = KeyHint::new(workspace).on_hint(|| {});
        let mut root = Flex::column();
        root.base_mut().children.push(Box::new(outer));

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
            targets.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(),
            vec![vec![0], vec![1]],
            "document order, addressed by path"
        );
        assert_eq!(targets[1].1.loc.y, 50.0, "…each with the rect its letter goes over");

        assert!(fire_hint(&mut root, &targets[1].0));
        assert_eq!(*log.borrow(), vec!["second"]);
    }

    /// **A button gets its letter for being a button — whatever it was put inside.**
    ///
    /// Where a widget sits must never decide what it can do (AGENTS § 0d). The picker used to break
    /// that: a container saying what picking *it* did switched off letters for everything beneath,
    /// so a button in a pane's bar had to repeat its own click as a hint to win one back, and the
    /// control the bar builds for itself had nobody to do that and silently wore none (Antonio,
    /// driving, 2026-09-04).
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
        region.base_mut().children.push(Box::new(label));
        region.base_mut().children.push(Box::new(button));

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

/// **A letter goes only where it can be seen** (F003/P082/T438).
///
/// The picker was the one tree walk that never asked `Component::clips_children`. Paint honours it,
/// input honours it (`pointer::hit_test`), and the picker handed letters to rows scrolled past a
/// sidebar's fold — whose keycaps then painted over the top bar and the status bar, because a cap
/// goes into the overlay band and an overlay segment starts unclipped so a dropdown can escape a
/// scroll region.
#[cfg(test)]
mod clipped_away {
    use crate::hint::*;
    use crate::builders::{ComponentExt as _, LayoutExt as _, Parent as _};
    use crate::component::Component;
    use crate::reactive::SignalGet;
    use crate::style::Length;
    use crate::widgets::{Flex, Label, Row, ScrollRegion};
    use crate::LayoutEngine;
    use heca_core::layout::Size;

    /// Six 40px rows in a 100px viewport, scrolled to the bottom: the first rows are above the
    /// fold, the last are on screen. The same shape as a sidebar with more panes than room.
    fn scrolled_sidebar() -> ScrollRegion {
        let rows = (0..6).fold(Flex::column(), |c, i| {
            c.child(
                Row::new()
                    .height(Length::Px(40.0))
                    .key(format!("pane:{i}"))
                    .child(Label::new(format!("zsh {i}")))
                    .on_hint(|| {}),
            )
        });
        let mut region = ScrollRegion::new()
            .width(Length::Px(200.0))
            .height(Length::Px(100.0))
            .child(rows);
        LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
        region.scroll_to(120.0);
        LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
        region
    }

    fn row(region: &ScrollRegion, i: usize) -> &dyn Component {
        region.base().children[0].base().children[i].as_ref()
    }

    fn letter(region: &ScrollRegion, i: usize) -> Option<String> {
        row(region, i).base().hint_label.get_untracked()
    }

    /// A row scrolled clean past the fold gets **no letter** — by key, which is the `prefix+q`
    /// path, where the candidates come from the session model and nothing geometric is consulted.
    #[test]
    fn a_row_past_the_fold_is_not_lettered() {
        let region = scrolled_sidebar();
        assert!(
            row(&region, 0).base().bounds.loc.y + row(&region, 0).base().bounds.size.h < 0.0,
            "row 0 is entirely above the viewport, which is the case under test",
        );

        assert!(!offer_hint_by_key(&region, "pane:0", Some("a".into())));
        assert_eq!(letter(&region, 0), None, "no keycap to paint over the chrome above");
    }

    /// **A row you can half see keeps its letter**, so you can peek at it (Antonio, 2026-08-23:
    /// *"a half visible pane row should have the letter to peek"*). Any overlap at all is visible.
    #[test]
    fn a_half_visible_row_keeps_its_letter() {
        let region = scrolled_sidebar();
        let straddling = (0..6)
            .find(|&i| {
                let b = row(&region, i).base().bounds;
                b.loc.y < 0.0 && b.loc.y + b.size.h > 0.0
            })
            .expect("one row straddles the top of the viewport");

        assert!(offer_hint_by_key(&region, &format!("pane:{straddling}"), Some("s".into())));
        assert_eq!(letter(&region, straddling).as_deref(), Some("s"));
    }

    /// A row in full view is untouched by any of this.
    #[test]
    fn a_visible_row_is_lettered_as_before() {
        let region = scrolled_sidebar();
        assert!(offer_hint_by_key(&region, "pane:5", Some("d".into())));
        assert_eq!(letter(&region, 5).as_deref(), Some("d"));
    }

    /// **Withdrawal is never refused.** A row that scrolled out of view *after* being lettered must
    /// still lose its keycap, or the cap outlives the picker that put it up.
    #[test]
    fn a_letter_is_always_withdrawable_wherever_the_row_has_gone() {
        let mut region = scrolled_sidebar();
        assert!(offer_hint_by_key(&region, "pane:5", Some("d".into())));

        region.scroll_to(0.0);
        LayoutEngine::new().compute(&mut region, Size::new(200.0, 100.0));
        assert!(
            row(&region, 5).base().bounds.loc.y > 100.0,
            "row 5 has scrolled below the fold while wearing its letter",
        );

        assert!(offer_hint_by_key(&region, "pane:5", None), "the withdrawal still lands");
        assert_eq!(letter(&region, 5), None);
    }

    /// **A tree that has never been laid out still gets its letters.** Every widget's bounds are
    /// zero before the first layout, so a clipping widget with no geometry would clip everything
    /// away — which took every letter off the workspaces sidebar in the provider's own tests, where
    /// the body is built and collected without being laid out. Degenerate bounds are *"no answer
    /// yet"*, never *"nothing is visible"*.
    #[test]
    fn a_tree_with_no_layout_yet_is_not_clipped_to_nothing() {
        let rows = (0..3).fold(Flex::column(), |c, i| {
            c.child(Row::new().key(format!("pane:{i}")).child(Label::new("zsh")).on_hint(|| {}))
        });
        let region = ScrollRegion::new().child(rows); // never laid out: all bounds are 0x0

        assert_eq!(collect_hints(&region).len(), 3, "the rows are still candidates");
        assert!(offer_hint_by_key(&region, "pane:1", Some("a".into())));
    }

    /// The same rule on the other walk: `collect_hints` drops a clipped-away candidate, so it does
    /// not spend one of the 52 either.
    #[test]
    fn a_clipped_away_candidate_is_not_collected() {
        let region = scrolled_sidebar();
        let found = collect_hints(&region);
        for (path, bounds) in &found {
            assert!(
                bounds.loc.y + bounds.size.h > 0.0 && bounds.loc.y < 100.0,
                "a candidate outside the viewport was collected at {path:?} ({bounds:?})",
            );
        }
        assert!(!found.is_empty(), "the rows still in view are still candidates");
    }
}

/// **Whose targets are whose** — the grouping a host with layered surfaces needs, so no host
/// writes it twice (F003/P097/T499).
#[cfg(test)]
mod by_surface {
    use crate::builders::Parent as _;
    use crate::component::Component;
    use crate::hint::*;
    use crate::reactive::SignalUpdate as _;
    use crate::widgets::Flex;
    use heca_core::layout::Size;

    /// A page with a surface seated above it, each holding one pickable widget — the shape every
    /// host with layers has.
    fn page_with_a_seated_surface() -> Flex {
        use crate::widgets::Button;

        let mut surface = Flex::column().child(Button::new("in the surface").on_click(|| {}));
        surface.base_mut().key = Some("surface:1".into());
        surface.base_mut().surface = true;

        let mut root = Flex::column()
            .child(Flex::column().child(Button::new("on the page").on_click(|| {})))
            .child(surface);
        crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));
        root
    }

    /// **A widget is collected once, under the surface that owns it** (F003/P097/T499).
    ///
    /// The host used to collect the whole tree and then each seated surface again, so a widget
    /// inside one carried two candidates — each drawing its own letter. Grouping is what removes
    /// the second walk, and it is here rather than in the host so no future host writes it again.
    ///
    /// The assertion is **where each target lands**, never how many there are in total: a total is
    /// satisfied by collecting the right number of the wrong things, which is what the doubled
    /// version did.
    #[test]
    fn a_widget_is_collected_once_under_the_surface_that_owns_it() {
        let root = page_with_a_seated_surface();
        let groups = collect_hints_by_surface(&root);

        let mut seen: Vec<(Vec<usize>, Vec<Vec<usize>>)> = groups
            .iter()
            .map(|g| {
                (
                    g.path.clone(),
                    g.targets.iter().map(|(p, _)| p.clone()).collect(),
                )
            })
            .collect();
        for (_, t) in &mut seen {
            t.sort();
        }
        assert_eq!(
            seen,
            vec![(vec![1], vec![vec![1, 0]]), (vec![], vec![vec![0, 0]]),],
            "front → back: the surface owning its own widget, then the page owning its own; \
             each target once, addressed from the ROOT",
        );
    }

    /// **The page is the group with no path**, so a host tells it from a surface structurally
    /// rather than by matching a name — and gets the surface's key and box handed to it, instead of
    /// walking back down the tree to find them.
    #[test]
    fn the_page_is_the_group_with_no_path_and_a_surface_carries_its_own_key() {
        let root = page_with_a_seated_surface();
        let groups = collect_hints_by_surface(&root);

        let page = groups.iter().find(|g| g.path.is_empty()).expect("the page");
        assert_eq!(page.key, None, "the page is named by being the page");

        let surface = groups
            .iter()
            .find(|g| !g.path.is_empty())
            .expect("the surface");
        assert_eq!(surface.key.as_deref(), Some("surface:1"));
        assert!(
            surface.bounds.size.w > 0.0,
            "its laid-out box comes back too, so a host never re-walks for it",
        );
    }

    /// **A target belongs to the DEEPEST surface enclosing it.** A menu inside a dialog groups
    /// under the menu, so a host never assumes surfaces are direct children of the root — which is
    /// what reading the first step of a path would assume.
    #[test]
    fn a_nested_surface_owns_its_own_targets() {
        use crate::widgets::Button;

        let mut inner = Flex::column().child(Button::new("in the menu").on_click(|| {}));
        inner.base_mut().key = Some("menu".into());
        inner.base_mut().surface = true;

        let mut outer = Flex::column()
            .child(Button::new("in the dialog").on_click(|| {}))
            .child(inner);
        outer.base_mut().key = Some("dialog".into());
        outer.base_mut().surface = true;

        let mut root = Flex::column().child(outer);
        crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

        let groups = collect_hints_by_surface(&root);
        let owner = |k: &str| {
            groups
                .iter()
                .find(|g| g.key.as_deref() == Some(k))
                .unwrap_or_else(|| panic!("no group for {k}"))
        };
        assert_eq!(
            owner("menu").targets.len(),
            1,
            "the menu owns its own button"
        );
        assert_eq!(
            owner("dialog").targets.len(),
            1,
            "the dialog owns only what is not inside the menu",
        );
        assert_eq!(
            groups.first().map(|g| g.key.as_deref()),
            Some(Some("menu")),
            "front → back: the nested surface is in front of the one holding it",
        );
    }

    /// **The line a plugin author writes** (⭐⭐ RULE ZERO, F003/P097/T499).
    ///
    /// A surface says what it obscures with one builder each, on its own widget — no registry, no
    /// id, no host-private type — and a host reads both back off the tree it already walks. While
    /// these lived as fields on a host registry entry, a plugin could declare them only by reaching
    /// a host call, so its overlay was a second-class version of one of heca's own.
    #[test]
    fn a_surface_declares_what_it_obscures_with_one_line_on_the_widget() {
        use crate::builders::ComponentExt as _;
        use crate::widgets::Button;

        let mut panel = Flex::column()
            .child(Button::new("in my panel").on_click(|| {}))
            .lock(true);
        // Taking the keyboard is NOT written: an overlay holds focus while it is open, and that is
        // how it takes keys. A bare `Flex` has no such story, so this test says so explicitly —
        // which is the one case the override exists for.
        panel.base_mut().focused.set(true);
        panel.base_mut().key = Some("myplugin.panel".into());
        panel.base_mut().surface = true;

        let mut root = Flex::column().child(panel);
        crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

        let group = collect_hints_by_surface(&root)
            .into_iter()
            .find(|g| g.key.as_deref() == Some("myplugin.panel"))
            .expect("the plugin's panel owns its own targets");
        assert!(
            group.lock,
            "what the widget declared is what the host reads back, with nothing registered",
        );
        assert!(
            group.holds_keyboard,
            "and taking the keyboard is READ from the tree — the surface holds focus, so it is \
             the active context without anyone declaring it",
        );
    }

    /// The counterpart, so the rule above cannot be satisfied by declaring everything: a surface
    /// that says nothing obscures nothing. That is the toast stack — it spans the window to
    /// position its cards in a corner, and suppresses no letters anywhere.
    #[test]
    fn a_surface_that_declares_nothing_obscures_nothing() {
        use crate::widgets::Button;

        let mut ambient = Flex::column().child(Button::new("a toast").on_click(|| {}));
        ambient.base_mut().key = Some("heca.notifications".into());
        ambient.base_mut().surface = true;

        let mut root = Flex::column().child(ambient);
        crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

        let group = collect_hints_by_surface(&root)
            .into_iter()
            .find(|g| !g.path.is_empty())
            .expect("the stack is a surface like any other");
        assert!(
            !group.lock && !group.holds_keyboard,
            "spanning the window is how it POSITIONS its cards, not a claim on the screen — and \
             holding no focus is how it says it is not taking the keyboard",
        );
    }

    /// **A named node's words can be rewritten without rebuilding anything** (F003/P097/T500).
    ///
    /// This is what lets a host change what a retained tree *says* while keeping the tree: its
    /// identity, its layout and every widget in it survive, because the text is a signal.
    ///
    /// The case it exists for: a pane header shows the foreground program and the branch. Both
    /// change while you work, and while they were part of the header's identity every change threw
    /// the bar away and built a new one — and a fresh widget paints nothing until the layout walk
    /// gives it a box, so the buttons blinked out and back.
    #[test]
    fn a_named_nodes_words_are_rewritten_in_place() {
        use crate::builders::ComponentExt as _;
        use crate::widgets::Label;

        use crate::reactive::SignalGet as _;

        let app = Label::new("zsh").key("hdr.seg:AppName");
        let branch = Label::new("main").key("hdr.seg:GitBranch");
        // The signals, taken before the labels move into the tree — what the widgets will say.
        let (app_text, branch_text) = (app.text_signal(), branch.text_signal());
        let root = Flex::column().child(app).child(branch);

        assert!(set_text_by_key(&root, "hdr.seg:AppName", "cargo"));
        assert_eq!(
            app_text.get_untracked(),
            "cargo",
            "the named node says the new words"
        );
        assert_eq!(
            branch_text.get_untracked(),
            "main",
            "and nothing else was touched",
        );
        assert!(
            !set_text_by_key(&root, "hdr.seg:Nothing", "x"),
            "a name nothing answers to writes nothing, and says so",
        );
    }
}

/// **A target you cannot see gets no letter** (F003/P082/T478) — the second half of the visibility
/// rule, beside the clip. A widget squeezed to nothing draws nothing, so there is nothing to aim at.
#[cfg(test)]
mod visibility {
    use crate::builders::ComponentExt as _;
    use crate::component::Component;
    use crate::hint::*;
    use crate::reactive::SignalGet as _;
    use crate::widgets::{Flex, Surface};
    use heca_core::layout::{Point, Rectangle, Size};

    /// A clickable widget laid out at `x`,`y` with size `w`x`h`.
    fn target_at(x: f64, y: f64, w: f64, h: f64) -> Box<dyn Component> {
        let mut s = Surface::new().on_click(|_| {});
        s.base_mut().bounds = Rectangle::new(Point::new(x, y), Size::new(w, h));
        Box::new(s)
    }

    /// A laid-out page holding one target — so each test below reads as its assertion.
    fn page_with(child: Box<dyn Component>) -> Flex {
        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 800.0));
        root.base_mut().children.push(child);
        root
    }

    /// The defect: two sidebars in a narrow window squeezed the scrolling area to nothing, and every
    /// pane in it laid out at 0x728 — still lettered, its keycap drawn half a cap to the left of a
    /// pane with no inside, several of them stacked in the gutter.
    #[test]
    fn a_target_squeezed_to_no_width_is_not_lettered() {
        let root = page_with(target_at(318.0, 40.0, 0.0, 728.0));

        assert!(
            collect_hints(&root).is_empty(),
            "a pane with no width has nothing to aim at, and must not spend one of the 52",
        );
    }

    /// The same for the other axis — a row collapsed to a line.
    #[test]
    fn a_target_squeezed_to_no_height_is_not_lettered() {
        let root = page_with(target_at(10.0, 40.0, 280.0, 0.0));

        assert!(collect_hints(&root).is_empty());
    }

    /// The counterpart, so the rule cannot be satisfied by lettering nothing: a target with real
    /// geometry is still a target.
    #[test]
    fn a_target_with_a_real_box_is_still_lettered() {
        let root = page_with(target_at(318.0, 40.0, 386.0, 728.0));

        assert_eq!(collect_hints(&root).len(), 1);
    }

    /// ⚠️ **A tree that has not been laid out yet is not "invisible"** — no position and no size is
    /// *no answer yet*. The chrome tree is rebuilt with zero bounds and laid out afterwards; judging
    /// it in between called every sidebar row hidden and took back its letter.
    #[test]
    fn a_target_that_has_not_been_laid_out_keeps_its_candidacy() {
        // Deliberately NOT `page_with`: nothing here has been laid out, root included.
        let mut root = Flex::column();
        root.base_mut().children.push(target_at(0.0, 0.0, 0.0, 0.0));

        assert_eq!(
            collect_hints(&root).len(),
            1,
            "no bounds at all is 'not laid out yet', never 'not visible'",
        );
    }

    /// The offer walk asks the **same** question, so a letter aimed at a squeezed view by name is
    /// refused exactly as the collector would have refused to spend one on it.
    #[test]
    fn a_letter_is_not_handed_to_a_squeezed_view_by_name() {
        let mut row = Surface::new().key("pane:7").on_click(|_| {});
        row.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));
        let root = page_with(Box::new(row));

        assert!(
            !offer_hint_by_key(&root, "pane:7", Some("a".into())),
            "nothing here can show it",
        );
        assert!(
            root.base().children[0]
                .base()
                .hint_label
                .get_untracked()
                .is_none(),
        );
    }

    /// **A pane you cannot see is still pickable — through its other views** (Antonio, 2026-09-09:
    /// *"a not visible pane on the scrolling area. Panes in sidebar and expose. Even if not visible
    /// gets a letter to pick"*).
    ///
    /// One pane is shown in several places: its own view in the scrolling area, its row in the
    /// sidebar, its card in the exposé. They all name it. The visibility rule filters the **view**,
    /// never the candidate — so the squeezed view loses the letter and the sidebar row wears it, and
    /// the pane stays reachable by keyboard however narrow the window.
    #[test]
    fn a_squeezed_view_loses_its_letter_and_the_other_views_keep_it() {
        // The pane itself, squeezed to nothing by two sidebars in a narrow window.
        let mut pane = Surface::new().key("pane:7").on_click(|_| {});
        pane.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));
        // …and its sidebar row, which is perfectly visible.
        let mut row = Surface::new().key("pane:7").on_click(|_| {});
        row.base_mut().bounds = Rectangle::new(Point::new(6.0, 90.0), Size::new(276.0, 28.0));

        let mut root = Flex::column();
        root.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 800.0));
        root.base_mut().children.push(Box::new(pane));
        root.base_mut().children.push(Box::new(row));

        assert!(
            offer_hint_by_key(&root, "pane:7", Some("a".into())),
            "the pane is still pickable — one of its views can show the letter",
        );
        assert!(
            root.base().children[0]
                .base()
                .hint_label
                .get_untracked()
                .is_none(),
            "not on the view squeezed to nothing, where the cap would land outside the pane",
        );
        assert_eq!(
            root.base().children[1]
                .base()
                .hint_label
                .get_untracked()
                .as_deref(),
            Some("a"),
            "on the sidebar row, which you can see and can aim at",
        );
    }

    /// **Withdrawal is never refused.** A view that was lettered and has since collapsed must still
    /// give the letter back, or the keycap outlives the picker that put it up.
    #[test]
    fn a_squeezed_view_still_gives_its_letter_back() {
        let mut row = Surface::new().key("pane:7").on_click(|_| {});
        row.base_mut().bounds = Rectangle::new(Point::new(318.0, 40.0), Size::new(386.0, 728.0));
        let mut root = page_with(Box::new(row));

        assert!(offer_hint_by_key(&root, "pane:7", Some("a".into())));
        // …and now the window narrows and the pane is squeezed away.
        root.base_mut().children[0].base_mut().bounds =
            Rectangle::new(Point::new(318.0, 40.0), Size::new(0.0, 728.0));

        assert!(
            offer_hint_by_key(&root, "pane:7", None),
            "taking a letter back must reach a view wherever it has gone",
        );
        assert!(
            root.base().children[0]
                .base()
                .hint_label
                .get_untracked()
                .is_none(),
        );
    }
}

/// **Two verbs over one tree** — the failure scopes exist to stop.
///
/// A surface can mean more than one thing by a letter: the exposé's cards mean *go there* and
/// their ⊠ icons mean *remove that*. One picker over both hands out twice the letters, and half of
/// them delete what you meant to jump to.
mod scoped_pickers {
    use crate::builders::{ComponentExt, Parent};
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
    /// and is not this task's to change — but with scopes in place the restriction is redundant,
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
        assert!(crate::hint::in_scope(&button, Some("close")));
        assert!(crate::hint::in_scope(&button, Some("other")));
        assert!(
            !crate::hint::in_scope(&button, None),
            "still out of the ordinary picker"
        );
    }
}
