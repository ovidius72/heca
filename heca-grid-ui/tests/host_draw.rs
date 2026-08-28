//! **Work only the host can do, recorded in scene order.**
//!
//! Some content cannot be expressed as rectangles and text: a terminal is rasterised into a texture
//! because its cell glyphs are the hottest path in the app, and a frosted backdrop is a pass over
//! what has already been drawn rather than a shape. Neither is a special case in this crate — a
//! widget says *what it wants* and where, the host owns the GPU and does it.
//!
//! These tests pin the half that lives here: the request is recorded, placed, clipped, faded and
//! scaled like any other command. What the host does with it is the host's business.

use heca_grid_ui::component::PaintCx;
use heca_grid_ui::scene::{DrawCommand, HostCmd, HostDraw, Scene};
use heca_grid_ui::theme::Theme;
use heca_core::layout::{Point, Rectangle, Size};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

/// Every host request in the scene, in order.
fn host_cmds(scene: &Scene) -> Vec<HostCmd> {
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Host(h) => Some(*h),
            _ => None,
        })
        .collect()
}

/// **A widget places a surface by saying where it goes, and nothing else.**
///
/// No texture, no format, no device crosses into this crate — the id is opaque here and the host
/// maps it to whatever it rasterised.
#[test]
fn a_surface_is_recorded_with_its_box_and_nothing_about_the_gpu() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.surface(rect(10.0, 20.0, 300.0, 200.0), 7);
    }

    assert_eq!(
        host_cmds(&scene),
        vec![HostCmd {
            draw: HostDraw::Surface { id: 7 },
            rect: rect(10.0, 20.0, 300.0, 200.0),
            alpha: 1.0,
        }],
    );
}

/// **A backdrop blurs what was drawn before it, so order is the whole meaning.**
///
/// Recorded in scene order: everything pushed earlier is behind it, everything later is in front.
/// A host that reordered these would blur the wrong frame.
#[test]
fn a_backdrop_is_recorded_after_what_it_blurs_and_before_what_it_does_not() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.rect(rect(0.0, 0.0, 100.0, 100.0), theme.colors.background, None, 0.0, None);
        cx.backdrop_blur(rect(0.0, 0.0, 800.0, 600.0), 12.0, 0.8);
        cx.rect(rect(5.0, 5.0, 50.0, 50.0), theme.colors.background, None, 0.0, None);
    }

    let kinds: Vec<&str> = scene
        .iter()
        .map(|c| match c {
            DrawCommand::Rect(_) => "rect",
            DrawCommand::Host(_) => "host",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, vec!["rect", "host", "rect"], "order is the meaning");
}

/// **A blur of nothing is not recorded.** A zero radius or a fully transparent frost is a request
/// for no work, and a host asked to do no work still costs a full-screen GPU pass.
#[test]
fn a_backdrop_with_no_strength_is_not_recorded_at_all() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.backdrop_blur(rect(0.0, 0.0, 800.0, 600.0), 0.0, 1.0);
        cx.backdrop_blur(rect(0.0, 0.0, 800.0, 600.0), 12.0, 0.0);
    }
    assert!(host_cmds(&scene).is_empty());
}

/// **Host work fades with the widget that asked for it** (F003/P082/T459 — a surface owns its
/// arrival and its exit).
///
/// A surface inside an overlay that is fading in must fade with it, and so must the frost it asked
/// for — left at full strength the frost holds the whole session out of focus for the length of the
/// fade and then snaps back sharp in one frame, which is the exact pop the fade exists to remove.
#[test]
fn host_work_fades_with_the_opacity_of_whoever_asked_for_it() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.with_opacity(0.5, |cx| {
            cx.surface(rect(0.0, 0.0, 10.0, 10.0), 1);
            cx.backdrop_blur(rect(0.0, 0.0, 10.0, 10.0), 8.0, 0.6);
        });
    }

    let alphas: Vec<f32> = host_cmds(&scene).iter().map(|h| h.alpha).collect();
    assert_eq!(alphas.len(), 2);
    assert!((alphas[0] - 0.5).abs() < 1e-6, "a surface fades: {:?}", alphas[0]);
    assert!(
        (alphas[1] - 0.3).abs() < 1e-6,
        "and a frost fades from its own strength, not to full: {:?}",
        alphas[1],
    );
}

/// **A blur radius is a distance, so it scales; an id and an alpha are not.**
///
/// The exposé opens by scaling its whole tree. A radius left unscaled would blur by a different
/// amount at every step of the animation.
#[test]
fn a_scaled_subtree_scales_the_blur_radius_and_leaves_the_rest_alone() {
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.with_scale(2.0, Point::new(0.0, 0.0), |cx| {
            cx.backdrop_blur(rect(0.0, 0.0, 10.0, 10.0), 8.0, 0.5);
            cx.surface(rect(0.0, 0.0, 10.0, 10.0), 3);
        });
    }

    let cmds = host_cmds(&scene);
    assert_eq!(cmds.len(), 2);
    match cmds[0].draw {
        HostDraw::Backdrop { radius } => {
            assert!((radius - 16.0).abs() < 1e-6, "the radius scales: {radius}")
        }
        other => panic!("expected a backdrop, got {other:?}"),
    }
    assert_eq!(cmds[1].draw, HostDraw::Surface { id: 3 }, "an id does not scale");
    assert!((cmds[0].alpha - 0.5).abs() < 1e-6, "and neither does an alpha");
}
