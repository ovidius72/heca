//! Tests for the pane frame: what it shows, and what the pointer does to it.

use super::*;

use crate::chrome::panes::pane::testing::{model, recording_callbacks};
use heca_grid_ui::Component;
use heca_grid_ui::{LayoutEngine, Size};

mod frame;
mod pointer;

/// Give a standalone pane the box a workspace would place it in.
pub(super) fn size_to(root: &mut dyn heca_grid_ui::Component, w: f32, h: f32) {
    let style = &mut root.base_mut().style.layout;
    style.width = heca_grid_ui::Length::Px(w);
    style.height = heca_grid_ui::Length::Px(h);
}

/// **What a click on a pane means, asserted through the real path.** ⚠️ Ran red first.
///
/// The pane names an action; the framework routes it. So this installs the sink a host installs
/// and reads back what the pane asked for — no callbacks, because the pane has none any more.
/// It carried a struct of host functions until now, cloned at the call site, which a plugin
/// could not have built.
pub(super) fn dispatched(build: impl FnOnce(&mut UiPane)) -> Vec<String> {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = seen.clone();
    heca_grid_ui::intent::install_intent_sink(move |i| {
        sink.borrow_mut().push(format!(
            "{}({:?})",
            i.action,
            i.args.get("pane_id").cloned()
        ))
    });

    let cb = crate::chrome::panes::pane::testing::recording_callbacks().0;
    let mut pane = PaneShell {
        model: &model(3),
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    size_to(&mut pane, 400.0, 300.0);
    LayoutEngine::new().compute(&mut pane, Size::new(400.0, 300.0));
    build(&mut pane);
    seen.borrow().clone()
}
