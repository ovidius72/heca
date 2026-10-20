//! The layer stack's own tests: what is on screen, what is in charge, and what order they read.

use super::*;
use heca_grid_ui::animation::Animation;
use heca_grid_ui::widgets::{Flex, Overlay};

/// **One frame, in the order `handle_about_to_wait` runs it.**
///
/// Snapshot who is leaving, let the **tree** tick (the surfaces are its children), then let the
/// registry retire whatever finished. Written as a helper rather than a registry method because
/// that is the real shape: the registry does not advance anything any more.
///
/// The order is the test. When the registry ticked the trees *as well as* the tree ticking them,
/// every surface advanced twice a frame and the finishing transition was consumed before the
/// registry looked — the exposé stuck mid-fade and stayed modal. A test that called `reg.tick`
/// alone could not see that, because it never ran the tree's walk.
fn frame(reg: &mut LayerRegistry, window: &mut heca_grid_ui::widgets::Flex, dt: f32) -> bool {
    let leaving = reg.leaving_before_tick(window);
    let ticked = window.tick(dt);
    let retired = reg.retire_finished_exits(window, &leaving);
    ticked || retired
}

/// A layer whose surface declares nothing: it appears and goes between two frames.
fn empty_root() -> Box<dyn Component> {
    Box::new(Flex::row())
}

/// **A surface that declares what it is** — the two things an overlay says about the application
/// underneath it, said the way a plugin author says them: one builder each, on the widget.
/// The registry is told neither.
fn declaring(
    mut root: Box<dyn Component>,
    captures_keyboard: bool,
    lock: bool,
) -> Box<dyn Component> {
    // **Taking the keyboard is read from the tree**, not declared — a surface that wants keys holds
    // focus. These fixtures are bare `Flex`es with no focus of their own, so they override it; a
    // real overlay writes nothing and answers by being open.
    root.base_mut().captures_keyboard = Some(captures_keyboard);
    root.base_mut().lock = lock;
    root
}

/// **A real layer root**: a surface that names its own gesture, exactly as the exposé and a
/// plugin's panel do — one builder on the widget, nothing said to the registry.
fn fading_root() -> Box<dyn Component> {
    Box::new(Overlay::new().panel(Flex::row()).animation(Animation::Fade))
}

/// The same, for a surface that arrives by pulling back — the exposé's own gesture.
fn zooming_root() -> Box<dyn Component> {
    Box::new(
        Overlay::new()
            .panel(Flex::row())
            .animation(Animation::Zoom.from(1.3)),
    )
}

/// What scale a layer's surface is drawing itself at this frame.
fn scale_of(window: &heca_grid_ui::widgets::Flex, id: LayerId) -> f32 {
    crate::chrome::surface_node(window, id)
        .and_then(|n| n.presence())
        .map_or(1.0, |p| p.frame().scale)
}

/// An open layer, built the way `fading_root` builds one.
fn fading_root_open() -> Overlay {
    let mut o = Overlay::new().panel(Flex::row());
    o.show();
    o
}

mod appearing;
mod coverage;
mod naming;
mod order;
