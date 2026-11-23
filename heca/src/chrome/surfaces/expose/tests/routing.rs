//! Activate and dismiss reach the grid through the overlay.

use super::*;

/// **Activate must reach the grid through the overlay**, and choose the card the cursor is on.
/// Enter and Space resolve to `WidgetIntent::Activate`, which travels Overlay → Surface →
/// CardGrid before anything happens; any widget on that path swallowing it is
/// indistinguishable from a wrong keybinding when you are looking at the app.
#[test]
fn an_activate_reaches_the_grid_and_chooses_the_card_under_the_cursor() {
    use heca_grid_ui::component::{Event, WidgetIntent as W};
    let (mut root, seen) = built(Some(PaneId(2)));
    heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Activate));
    let got = seen.borrow().join(" ");
    // Activate-and-leave, the same three parts the sidebar performs: focus the pane, hand the
    // keyboard back, put the surface away. The middle one is invisible until it is missing.
    assert!(
        got.contains("\"focus_pane\"") && got.contains("Int(2)"),
        "it focuses the card the cursor was on: {got:?}",
    );
    assert!(
        got.contains("\"unfocus_dock\""),
        "and hands the keyboard back: {got:?}"
    );
    assert!(
        got.contains("\"close_overlay\""),
        "and closes the map: {got:?}"
    );
}

/// **Dismiss must reach the grid through the overlay** — the same routing chain, for Esc.
#[test]
fn a_dismiss_reaches_the_grid_through_the_overlay() {
    use heca_grid_ui::component::{Event, WidgetIntent as W};
    let (mut root, seen) = built(None);
    heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
    let got = seen.borrow().join(" ");
    assert!(
        got.contains("\"close_overlay\""),
        "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
    );
}

/// **Enter on the map, seated in its layer as the app seats it, goes out as the map's own
/// gesture** — `focus_pane` included. Found in the app: a click chose the card but Enter was
/// refused, "blocked ActivateAction(FocusPane) from MouseContent", because the intent lost the
/// layer's source on the keyboard's path.
#[test]
fn an_activate_on_the_seated_map_goes_out_as_the_maps_gesture() {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use crate::chrome::surfaces::layers::{LayerKind, LayerRegistry};
    use heca_grid_ui::component::{Event, WidgetIntent as W};
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = sent.clone();
    let mut reg = LayerRegistry::default();
    reg.emit_through(move |surface| {
        let seen = seen.clone();
        crate::chrome::ChromeIntentEmitter::of(
            InteractionSource::Surface(surface),
            move |source, intent| seen.borrow_mut().push((source, intent)),
        )
    });
    let s = session();
    let rows = model(s.l(), |p| p.title.clone(), false);
    let mut root = map(
        &rows,
        Some(PaneId(2)),
        &LayoutOptions::default(),
        &shipped_keys(),
        None,
    );
    root.show();
    let mut window = crate::chrome::new_window_root();
    let id = reg.add(None, LayerKind::OnDemand, root, &mut window);
    heca_grid_ui::LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(1900.0, 1200.0));

    let fell_through = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = fell_through.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| sink.borrow_mut().push(intent.action));
    heca_grid_ui::dispatch(&mut window, &Event::Widget(W::Activate));
    assert!(
        fell_through.borrow().is_empty(),
        "nothing falls through to the host's sink, which the app reads as MouseContent: {:?}",
        fell_through.borrow()
    );
    let sent = sent.borrow();
    let focus = sent.iter().find(|(_, intent)| match intent {
        InteractionIntent::View(intent) => intent.action == "focus_pane",
        _ => false,
    });
    let (source, _) =
        focus.unwrap_or_else(|| panic!("focus_pane went out through the layer: {sent:?}"));
    assert_eq!(*source, InteractionSource::Surface(reg.surface_key(id)));
}
