//! **A drag held past a widget's edge scrolls it, every frame, with the mouse still** — and the
//! press is not taken from the window: `capture_pointer` keeps the pointer without consuming it.

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::Surface;
use heca_grid_ui::{EdgeScroll, Event, EventKind, LayoutEngine, Point, PointerButton, Size};
use std::cell::RefCell;
use std::rc::Rc;

/// 40 px outside for full speed, 100 px a second at full speed.
const SPEED: EdgeScroll = EdgeScroll {
    distance: 40.0,
    speed: 100.0,
};

fn tree(steps: Rc<RefCell<Vec<(f32, f32)>>>) -> Flex {
    Flex::column().child(
        Surface::new()
            .width(Length::Px(100.0))
            .height(Length::Px(100.0))
            .on(EventKind::PointerDown, |ev| ev.capture_pointer())
            .on_edge_scroll(move |ev| {
                if let Some(step) = ev.pointer().and_then(|p| p.delta_pixels) {
                    steps.borrow_mut().push(step);
                }
            }),
    )
}

#[test]
fn a_held_drag_past_the_edge_scrolls_each_frame_until_the_button_is_up() {
    let steps = Rc::new(RefCell::new(Vec::new()));
    let mut root = tree(steps.clone());
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 300.0));

    let pressed = heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_pressed(Point::new(50.0, 50.0), PointerButton::Left),
    );
    assert_eq!(
        pressed,
        heca_grid_ui::Handled::No,
        "the press is not taken: a window gesture can still claim it"
    );
    assert!(
        heca_grid_ui::pointer_held(&root),
        "held: leaving the window must not drop the gesture"
    );
    assert!(
        !heca_grid_ui::edge_scroll(&mut root, 0.1, SPEED),
        "inside the widget nothing scrolls"
    );

    heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(50.0, -20.0)));
    assert!(heca_grid_ui::edge_scroll(&mut root, 0.1, SPEED));
    assert!(
        heca_grid_ui::edge_scroll(&mut root, 0.1, SPEED),
        "again, with no move in between"
    );
    assert_eq!(
        steps.borrow().as_slice(),
        [(0.0, -5.0), (0.0, -5.0)],
        "20 px above, half of the 40 px distance: half speed, upwards"
    );

    heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(50.0, 200.0)));
    heca_grid_ui::edge_scroll(&mut root, 0.1, SPEED);
    assert_eq!(
        steps.borrow().last(),
        Some(&(0.0, 10.0)),
        "far below: full speed, downwards"
    );

    heca_grid_ui::dispatch(
        &mut root,
        &Event::pointer_released(Point::new(50.0, 200.0), PointerButton::Left),
    );
    assert!(
        !heca_grid_ui::edge_scroll(&mut root, 0.1, SPEED),
        "the button is up: it stops"
    );
    assert!(
        !heca_grid_ui::pointer_held(&root),
        "and nothing holds the pointer any more"
    );
}
