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
        got.contains("FocusPaneThenAction") && got.contains("PaneId(2)"),
        "it focuses the card the cursor was on: {got:?}",
    );
    assert!(
        got.contains("UnfocusDock"),
        "and hands the keyboard back: {got:?}"
    );
    assert!(got.contains("CloseOverlay"), "and closes the map: {got:?}");
}

/// **Dismiss must reach the grid through the overlay** — the same routing chain, for Esc.
#[test]
fn a_dismiss_reaches_the_grid_through_the_overlay() {
    use heca_grid_ui::component::{Event, WidgetIntent as W};
    let (mut root, seen) = built(None);
    heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(W::Dismiss));
    let got = seen.borrow().join(" ");
    assert!(
        got.contains("CloseOverlay"),
        "Esc must run the close_overlay ACTION, not a bespoke hide; the emitter saw: {got:?}",
    );
}
