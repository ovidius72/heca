//! **A builder that may have nothing to say takes an `Option`** — `.on_hint(..)`, `.tooltip(..)`
//! and `.key(..)` — so a row whose pick, tip or name is sometimes missing is written in one chain,
//! with no branch around the builder. `None` leaves the widget as it was.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{Component, IdentityExt};

#[test]
fn none_declares_nothing_and_some_declares_it() {
    let pick: Option<fn()> = None;
    let tip: Option<&str> = None;
    let name: Option<String> = None;
    let row = Row::new().on_hint(pick).tooltip(tip).key(name);
    assert!(row.base().hint.is_none() && row.base().tooltip.is_none() && row.base().key.is_none());

    let row = Row::new()
        .on_hint(Some(|| {}))
        .tooltip(Some("Open"))
        .key(Some("room:1"));
    assert!(row.base().hint.is_some() && row.base().tooltip.is_some());
    assert_eq!(row.base().key.as_deref(), Some("room:1"));

    // `None` after a value keeps the value: it says nothing, it does not erase.
    let row = Row::new().key("kept").key(None::<&str>);
    assert_eq!(row.base().key.as_deref(), Some("kept"));
}

/// `.hidden(..)` and `.visible(..)` — CSS `display` and `visibility` — follow a value that changes,
/// on any widget: a signal or a value derived from one, with the tree never rebuilt.
#[test]
fn hidden_and_visible_follow_a_changing_value() {
    use heca_grid_ui::reactive::{SignalUpdate, SignalWith, signal};
    use heca_grid_ui::{LayoutEngine, PresenceExt, Size};
    let rooms = signal(Vec::<String>::new());
    let failed = signal(false);
    let mut root = Flex::column()
        .child(Label::new("No rooms yet").hidden(move || !rooms.with(Vec::is_empty)))
        .child(Button::new("Retry").visible(failed));
    let shown = |root: &Flex, i: usize| root.base().children[i].is_shown();
    let in_layout = |root: &Flex, i: usize| !root.base().children[i].base().style.layout.hidden;
    let lay = |root: &mut Flex| LayoutEngine::new().compute(root, Size::new(200.0, 100.0));

    lay(&mut root);
    assert!(
        shown(&root, 0) && !shown(&root, 1),
        "as built: no rooms, nothing failed"
    );
    rooms.update(|r| r.push("lobby".into()));
    failed.set(true);
    lay(&mut root);
    assert!(
        !shown(&root, 0) && shown(&root, 1),
        "the next layout follows both values"
    );
    assert!(
        !in_layout(&root, 0),
        "hidden is display: none — out of the layout"
    );
    failed.set(false);
    assert!(
        in_layout(&root, 1) && !shown(&root, 1),
        "visible is visibility: the box stays"
    );
}
