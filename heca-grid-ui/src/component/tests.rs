//! The component model's own tests.

use super::*;
use std::rc::Rc;

#[test]
fn mark_needs_paint_sets_the_flag_and_requests_a_frame() {
    let frames = Rc::new(Cell::new(0u32));
    let f = frames.clone();
    install_frame_request(move || f.set(f.get() + 1));

    let base = Base::new();
    assert!(base.needs_paint(), "a fresh widget needs its first paint");
    base.clear_needs_paint();
    assert!(!base.needs_paint(), "clearing drops the flag");

    base.mark_needs_paint();
    assert!(base.needs_paint(), "marking re-sets the repaint flag");
    assert_eq!(
        frames.get(),
        1,
        "marking asks the host for exactly one frame"
    );
}
