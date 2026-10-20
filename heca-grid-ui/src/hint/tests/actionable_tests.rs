use crate::builders::{HintExt, Parent};
use crate::hint::*;
use crate::widgets::{Button, Flex, Label};
use std::cell::RefCell;
use std::rc::Rc;

/// **Being pickable is not opt-in**. A button gets a letter for having a click
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
