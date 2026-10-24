//! The component model's own tests.

use super::*;

#[test]
fn mark_needs_paint_sets_the_flag_and_requests_a_frame() {
    let frames = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let f = frames.clone();
    install_frame_request(move || { f.fetch_add(1, std::sync::atomic::Ordering::Relaxed); });

    let base = Base::new();
    assert!(base.needs_paint(), "a fresh widget needs its first paint");
    base.clear_needs_paint();
    assert!(!base.needs_paint(), "clearing drops the flag");

    base.mark_needs_paint();
    assert!(base.needs_paint(), "marking re-sets the repaint flag");
    assert_eq!(
        frames.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "marking asks the host for exactly one frame"
    );
}
