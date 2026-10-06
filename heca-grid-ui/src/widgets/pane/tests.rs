use super::Pane;
use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{PaintCx, paint_child};
use crate::scene::{DrawCommand, HostDraw, Scene};
use crate::theme::Theme;
use crate::widgets::Flex;
use crate::{LayoutEngine, Size};

/// **A frosted pane asks for its blur before anything it draws**, over exactly its own box and with
/// its own corner radius — so the host blurs what lies under it, rounded like the frame, and not the
/// pane's own content.
#[test]
fn a_frosted_pane_records_its_blur_before_its_own_drawing() {
    let mut pane = Pane::new()
        .width(200.0)
        .height(100.0)
        .frosted(12.0)
        .radius(6.0)
        .child(Flex::row().width(50.0).height(50.0));
    LayoutEngine::new().compute(&mut pane, Size::new(300.0, 300.0));
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(&pane, &mut PaintCx::new(&mut scene, &theme));

    let first_host = scene
        .iter()
        .position(|c| {
            matches!(c, DrawCommand::Host(h) if matches!(
                h.draw,
                HostDraw::Backdrop { radius, corner } if radius == 12.0 && corner == 6.0
            ))
        })
        .expect("the blur is recorded, rounded to the pane's own corner");
    let first_rect = scene.iter().position(|c| matches!(c, DrawCommand::Rect(_)));
    assert!(
        first_rect.is_none_or(|r| first_host < r),
        "before anything the pane draws"
    );

    let plain = Pane::new().width(200.0).height(100.0);
    let mut plain = plain;
    LayoutEngine::new().compute(&mut plain, Size::new(300.0, 300.0));
    let mut scene = Scene::new();
    paint_child(&plain, &mut PaintCx::new(&mut scene, &theme));
    assert!(
        !scene.iter().any(|c| matches!(c, DrawCommand::Host(_))),
        "no frost, no request"
    );
}
