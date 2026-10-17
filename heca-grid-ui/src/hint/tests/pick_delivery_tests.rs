//! **The pick is an event, and it behaves like one**.

use crate::builders::{EventExt, HintExt, IdentityExt, Parent};
use crate::event::EventKind;
use crate::hint::*;
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
    assert_eq!(
        targets.len(),
        2,
        "the listener declares nothing, so it gets no letter"
    );

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
