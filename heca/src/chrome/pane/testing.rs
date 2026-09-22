//! Shared fixtures for the pane surface's tests, so each test reads as its assertion rather than
//! its setup (AGENTS.md § 0b-bis rule 7).

use heca_core::layout::PaneId;

use super::model::PaneShellModel;
use super::shell::PaneCallbacks;

/// A pane at a plain rect with the default frame — the starting point every test varies one field
/// of.
pub(crate) fn model(pane_id: u64) -> PaneShellModel {
    PaneShellModel {
        pane_id: PaneId(pane_id),
        x: 100.0,
        y: 50.0,
        w: 400.0,
        h: 300.0,
        active: false,
        frame: heca_config::appearance::BorderStyle::Bordered,
        border_color: [0.1, 0.9, 0.8, 1.0],
        border_width: 1.5,
        border_radius: 4.0,
        content_inset: 6.0,
        accent: [0.1, 0.9, 0.8, 1.0],
    }
}

/// The same pane, at a rect a caller chooses — what a column's tests vary.
pub(crate) fn model_at(pane_id: PaneId, x: f32, y: f32, w: f32, h: f32) -> PaneShellModel {
    PaneShellModel {
        pane_id,
        x,
        y,
        w,
        h,
        ..model(pane_id.0)
    }
}

/// Callbacks that record which panes were picked, instead of reaching the event loop — the app's
/// edges and nothing else.
pub(crate) fn recording_callbacks() -> (PaneCallbacks, std::rc::Rc<std::cell::RefCell<Vec<PaneId>>>)
{
    let picked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = picked.clone();
    (
        PaneCallbacks {
            pick: std::rc::Rc::new(move |id| sink.borrow_mut().push(id)),
        },
        picked,
    )
}
