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

/// **A pane's frame is drawn over its own children and under whatever comes after the pane.**
///
/// The frame is asked for in the pane's own paint but drawn after its children, so a terminal does
/// not cover it. It used to be deferred to the end of the whole scene, which put every pane's border
/// over the sidebar beside it and over a float laid on top; it lands where the pane's subtree ends.
#[test]
fn a_panes_frame_is_over_its_children_and_under_the_next_widget() {
    use crate::builders::StyleExt;
    use crate::color::Color;
    let child_fill = Color::new(1, 2, 3, 255);
    let next_fill = Color::new(9, 8, 7, 255);
    let mut root = Flex::column()
        .width(300.0)
        .height(300.0)
        .child(
            Pane::new()
                .width(100.0)
                .height(100.0)
                .child(
                    Pane::new()
                        .frameless()
                        .width(50.0)
                        .height(50.0)
                        .background(child_fill),
                ),
        )
        .child(
            Pane::new()
                .frameless()
                .width(100.0)
                .height(100.0)
                .background(next_fill),
        );
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 300.0));
    let theme = Theme::default();
    let mut scene = Scene::new();
    paint_child(&root, &mut PaintCx::new(&mut scene, &theme));

    let position = |want: &dyn Fn(&DrawCommand) -> bool| scene.iter().position(want);
    let child = position(&|c| matches!(c, DrawCommand::Rect(r) if r.fill == child_fill));
    let next = position(&|c| matches!(c, DrawCommand::Rect(r) if r.fill == next_fill));
    let frame = position(&|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some()));
    let (child, next, frame) = (child.unwrap(), next.unwrap(), frame.unwrap());
    assert!(child < frame, "the frame is over the pane's own child");
    assert!(frame < next, "and under the widget painted after the pane");
}
