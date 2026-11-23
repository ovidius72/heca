//! The assembled map: its seams, its delete and pick keys, where its cursor opens and stays, and its fit.

use super::testing::{card_of, session, shipped_keys};
use super::wiring::open_on;
use super::*;
use heca_core::layout::LayoutOptions;

/// The whole layer, as [`register`] assembles it.
fn built(
    start: Option<PaneId>,
) -> (
    heca_grid_ui::WidgetModel,
    std::rc::Rc<std::cell::RefCell<Vec<String>>>,
) {
    let s = session();
    let rows = model(s.l(), |p| p.title.clone(), false);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    // Built alone, the map sits in no layer, so what it sends reaches the host's sink — this one.
    let sink = seen.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| {
        sink.borrow_mut().push(format!("{intent:?}"))
    });
    let mut root = map(
        &rows,
        start,
        &LayoutOptions::default(),
        &shipped_keys(),
        None,
    );
    // **Shown, as the layer stack shows it.** The map is built closed and opened by whoever
    // mounts it (`LayerRegistry::show`), which is also what plays its arrival — so a test that
    // never opens it is testing a surface nobody has raised.
    root.show();
    heca_grid_ui::LayoutEngine::new()
        .compute(root.as_mut(), heca_grid_ui::Size::new(1900.0, 1200.0));
    (root, seen)
}

mod cursor;
mod keys;
mod picture;
mod routing;
