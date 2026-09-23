//! However many widgets ask for a frame, the host is asked once.

use std::cell::Cell;
use std::rc::Rc;

/// Install a hook that counts how many times the host was actually asked.
fn counting_host() -> Rc<Cell<u32>> {
    let asks = Rc::new(Cell::new(0));
    let seen = asks.clone();
    heca_grid_ui::install_frame_request(move || seen.set(seen.get() + 1));
    asks
}

/// **The whole point: a busy tree costs one wake-up, not one per widget.**
///
/// Every widget that changes anything calls `request_frame` (through `mark_needs_paint` /
/// `mark_needs_layout`), so hundreds of calls between two frames is the normal case, not a misuse.
/// A host whose hook posts to an event queue was woken once per call — measured at 101, 133, 176
/// and 185 wake-ups in a single second on an idle window, against 3 to 15 frames drawn.
#[test]
fn a_hundred_widgets_asking_wake_the_host_once() {
    let asks = counting_host();
    heca_grid_ui::frame_served();
    for _ in 0..100 {
        heca_grid_ui::request_frame();
    }
    assert_eq!(
        asks.get(),
        1,
        "the host was asked {} times for one frame",
        asks.get(),
    );
}

/// **And the next frame is still reachable.** Coalescing that never re-arms is worse than none:
/// a widget that marks itself after the request was taken would never be drawn at all.
#[test]
fn the_next_frame_can_still_be_asked_for() {
    let asks = counting_host();
    heca_grid_ui::frame_served();
    heca_grid_ui::request_frame();
    heca_grid_ui::request_frame();
    assert_eq!(asks.get(), 1, "two asks, one wake");

    heca_grid_ui::frame_served();
    heca_grid_ui::request_frame();
    assert_eq!(
        asks.get(),
        2,
        "after the host took the request, a fresh ask must reach it",
    );
}

/// A widget marking itself is a request — the coalescing has to sit under the real door, not
/// beside it, or the call sites that matter bypass it.
#[test]
fn marking_a_widget_goes_through_the_same_door() {
    use heca_grid_ui::component::Base;
    let asks = counting_host();
    heca_grid_ui::frame_served();

    let a = Base::new();
    let b = Base::new();
    a.mark_needs_paint();
    b.mark_needs_paint();
    a.mark_needs_layout();
    assert_eq!(asks.get(), 1, "three marks, one wake");
    assert!(a.needs_paint(), "the widget still knows it must repaint");
    assert!(a.needs_layout(), "and that it must lay out again");
    assert!(b.needs_paint());
}
