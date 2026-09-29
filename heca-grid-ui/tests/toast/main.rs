#[path = "../common/mod.rs"]
mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

/// Lay a stack out the way a host does: settle its arrivals, then let the **engine** place the
/// cards. Nothing here positions anything — that is the whole point of F003/P096/T486.
fn settled_stack(stack: &mut heca_grid_ui::ToastStack, vp: Size) {
    use heca_grid_ui::Component;
    // Reconcile + play every arrival to its end.
    stack.tick(0.0);
    while stack.tick(1.0 / 60.0) {}
    stack.base_mut().style.layout.width = Length::Px(vp.w as f32);
    stack.base_mut().style.layout.height = Length::Px(vp.h as f32);
    LayoutEngine::new().compute(stack, vp);
}

mod card;
mod position;
mod stack;
