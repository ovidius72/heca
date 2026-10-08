use super::*;
use heca_grid_ui::Modifiers;

fn hold(root: &mut dyn Component, meta: bool) {
    let m = Modifiers {
        meta,
        ..Modifiers::default()
    };
    let _ = heca_grid_ui::dispatch(root, &Event::ModifiersChanged(m));
}

/// A 100 × 50 source that can be picked up only while Cmd is held, laid out at the origin.
fn gated() -> (Box<dyn Component>, Seen) {
    let (mut probe, seen) = Probe::new(100.0, 50.0);
    probe.base.draggable = true;
    probe.base.key = Some("item".into());
    probe.base.drag_gate = Some(Rc::new(|m: Modifiers| m.meta));
    let mut root: Box<dyn Component> = Box::new(Flex::row().child(probe));
    LayoutEngine::new().compute(root.as_mut(), Size::new(100.0, 50.0));
    (root, seen)
}

fn far() -> Point {
    Point::new(90.0, 40.0)
}

/// **A drag with a pick-up rule starts only while the rule holds** — a press and a move without the
/// key is not a drag, whatever the distance.
#[test]
fn a_gated_source_does_not_start_a_drag_without_its_key() {
    let (mut root, seen) = gated();
    hold(root.as_mut(), false);
    press_at(root.as_mut(), LEFT, PointerButton::Left);
    mv(root.as_mut(), far());
    release_at(root.as_mut(), far(), PointerButton::Left);
    assert!(
        !kinds(&seen).contains(&"DragStart".to_string()),
        "{:?}",
        kinds(&seen)
    );
}

/// With the key it is picked up, and letting go of the key mid-drag does not cancel it: the drag
/// still ends, so its source is told.
#[test]
fn a_gated_source_starts_with_its_key_and_the_key_coming_up_does_not_cancel_it() {
    let (mut root, seen) = gated();
    hold(root.as_mut(), true);
    press_at(root.as_mut(), LEFT, PointerButton::Left);
    mv(root.as_mut(), far());
    assert!(kinds(&seen).contains(&"DragStart".to_string()), "{:?}", kinds(&seen));
    hold(root.as_mut(), false);
    mv(root.as_mut(), Point::new(95.0, 45.0));
    release_at(root.as_mut(), Point::new(95.0, 45.0), PointerButton::Left);
    let seen = kinds(&seen);
    assert!(seen.contains(&"DragEnd".to_string()), "{seen:?}");
    assert!(
        seen.iter().filter(|k| *k == "Drag").count() >= 2,
        "the drag kept following after the key came up: {seen:?}"
    );
}

/// **A drop target hears a drag it accepts begin and end** — wherever it is, hidden or not — and a
/// target that refuses the kind, and the source itself, do not.
#[test]
fn a_drop_target_that_accepts_the_kind_hears_the_drag_begin_and_end() {
    let (mut source, source_seen) = Probe::new(100.0, 50.0);
    source.base.draggable = true;
    source.base.drag_kind = Some("pane".into());
    source.base.key = Some("src".into());
    let (mut takes, takes_seen) = Probe::new(100.0, 50.0);
    takes.base.drop_target = true;
    takes.base.accepts = vec!["pane".into()];
    takes.base.key = Some("takes".into());
    let (mut hidden, hidden_seen) = Probe::new(100.0, 50.0);
    hidden.base.drop_target = true;
    hidden.base.accepts = vec!["pane".into()];
    hidden.base.visible.set(false);
    let (mut refuses, refuses_seen) = Probe::new(100.0, 50.0);
    refuses.base.drop_target = true;
    refuses.base.accepts = vec!["column".into()];
    let mut root: Box<dyn Component> = Box::new(
        Flex::row()
            .child(source)
            .child(takes)
            .child(hidden)
            .child(refuses),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 50.0));

    press_at(root.as_mut(), LEFT, PointerButton::Left);
    mv(root.as_mut(), Point::new(60.0, 40.0));
    let heard = |s: &Seen| kinds(s).into_iter().filter(|k| k.starts_with("DragInFlight") || k.starts_with("DragSettled")).collect::<Vec<_>>();
    assert_eq!(heard(&takes_seen), ["DragInFlight"]);
    assert_eq!(heard(&hidden_seen), ["DragInFlight"], "hidden targets are told too");
    assert!(heard(&refuses_seen).is_empty(), "it does not accept a pane");
    assert!(heard(&source_seen).is_empty(), "the source has its own events");

    release_at(root.as_mut(), Point::new(60.0, 40.0), PointerButton::Left);
    assert_eq!(heard(&takes_seen), ["DragInFlight", "DragSettled"]);
    assert_eq!(heard(&hidden_seen), ["DragInFlight", "DragSettled"]);
}

/// **Where the carried widget sits** is its own box, not the pointer's — and nothing once the drag
/// is over.
#[test]
fn the_carried_widget_reports_where_it_sits_and_nothing_after() {
    let (mut root, _seen) = gated();
    assert_eq!(heca_grid_ui::dragged_bounds(root.as_ref()), None);
    hold(root.as_mut(), true);
    press_at(root.as_mut(), LEFT, PointerButton::Left);
    mv(root.as_mut(), Point::new(90.0, 40.0));
    let at = heca_grid_ui::dragged_bounds(root.as_ref()).expect("one is being carried");
    assert_eq!((at.loc.x, at.loc.y, at.size.w, at.size.h), (0.0, 0.0, 100.0, 50.0));
    release_at(root.as_mut(), Point::new(90.0, 40.0), PointerButton::Left);
    assert_eq!(heca_grid_ui::dragged_bounds(root.as_ref()), None);
}
