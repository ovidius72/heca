//! What a widget that clips its children does.
use super::Flex;
use crate::builders::{EventExt, LayoutExt, Parent, PresenceExt};
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

/// **The cut holds however a widget draws its children** — here one that draws them itself, under
/// a colour of its own, as a button or a row does. It writes nothing about clipping; the framework
/// cuts each child it draws to the widget's box and round corners.
#[test]
fn a_widget_that_draws_its_own_children_is_still_cut() {
    use crate::component::{Base, Component};
    use crate::scene::{HostCmd, HostDraw, Rounded};

    struct Own {
        base: Base,
    }
    impl Component for Own {
        fn base(&self) -> &Base {
            &self.base
        }
        fn base_mut(&mut self) -> &mut Base {
            &mut self.base
        }
        fn paint(&self, cx: &mut PaintCx) {
            cx.with_content_color(crate::color::Color::rgb(1, 2, 3), |cx| {
                for child in &self.base.children {
                    paint_child(child.as_ref(), cx);
                }
            });
        }
    }
    struct Screen {
        base: Base,
    }
    impl Component for Screen {
        fn base(&self) -> &Base {
            &self.base
        }
        fn base_mut(&mut self) -> &mut Base {
            &mut self.base
        }
        fn paint(&self, cx: &mut PaintCx) {
            cx.surface(self.base.bounds, 9);
        }
    }
    let mut own = Own { base: Base::new() };
    own.base.style.layout.width = crate::Length::Px(100.0);
    own.base.style.layout.height = crate::Length::Px(100.0);
    own.base.style.visual.radius = 6.0;
    own.base.clip_children = true;
    let mut screen = Screen { base: Base::new() };
    screen.base.style.layout.width = crate::Length::Px(300.0);
    screen.base.style.layout.height = crate::Length::Px(300.0);
    own.base.children.push(crate::WidgetModel::new(screen));
    let mut root = Flex::column().child(own);
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 400.0));

    let scene = painted(&root);
    let at = |c: &DrawCommand| scene.iter().position(|d| d == c);
    let clip = scene
        .iter()
        .find(|c| matches!(c, DrawCommand::PushClip(r) if r.size.w == 100.0 && r.size.h == 100.0))
        .cloned();
    let surface = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Host(
                h @ HostCmd {
                    draw: HostDraw::Surface { id: 9 },
                    ..
                },
            ) => Some(h.clone()),
            _ => None,
        })
        .expect("the child drew its surface");
    let clip = clip.expect("the child is drawn inside a clip of the widget's box");
    assert!(
        at(&clip) < at(&DrawCommand::Host(surface.clone())),
        "the clip is open when the child draws"
    );
    assert_eq!(
        surface.rounded.map(|r: Rounded| r.radius),
        Some(6.0),
        "and under the widget's round corners"
    );
}
