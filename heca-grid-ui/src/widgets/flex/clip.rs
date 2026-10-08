//! What a widget that clips its children does.
use super::Flex;
use crate::builders::{ComponentExt, LayoutExt, Parent};
use crate::component::Component as _;
use crate::component::{PaintCx, paint_child};
use crate::scene::{DrawCommand, Scene};
use crate::theme::Theme;
use crate::{LayoutEngine, Point, Size, hit_test};

/// A box 100 × 100 holding a child placed 300 × 300 — hanging out of it, as a placed child can.
fn holding_a_big_child(clip: bool) -> Flex {
    Flex::column()
        .width(100.0)
        .height(100.0)
        .clip_children(clip)
        .child(Flex::row().at_rect(0.0, 0.0, 300.0, 300.0).on_click(|_| {}))
}

fn painted(root: &Flex) -> Scene {
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(root, &mut PaintCx::new(&mut scene, &theme));
    scene
}

/// **A widget that clips what it holds draws nothing past its edge and cannot be hit there** —
/// `overflow: hidden`, said on any widget, not only on a scroll region.
#[test]
fn a_widget_that_clips_hides_and_ignores_what_hangs_past_its_edge() {
    let mut clipping = holding_a_big_child(true);
    LayoutEngine::new().compute(&mut clipping, Size::new(400.0, 400.0));
    assert_eq!(
        clipping.base().children[0].base().bounds.size.w,
        300.0,
        "what it holds keeps its own size: a clipping box does not squeeze it",
    );
    assert!(
        painted(&clipping).iter().any(
            |c| matches!(c, DrawCommand::PushClip(r) if r.size.w == 100.0 && r.size.h == 100.0)
        ),
        "what it holds is drawn inside a clip of its own box",
    );
    assert!(
        hit_test(&clipping, Point::new(200.0, 200.0)).is_none(),
        "past the edge is not hit"
    );
    assert!(
        hit_test(&clipping, Point::new(50.0, 50.0)).is_some(),
        "inside it is"
    );

    let mut open = holding_a_big_child(false);
    LayoutEngine::new().compute(&mut open, Size::new(400.0, 400.0));
    assert!(
        !painted(&open)
            .iter()
            .any(|c| matches!(c, DrawCommand::PushClip(_)))
    );
}
