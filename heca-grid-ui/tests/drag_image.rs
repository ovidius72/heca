//! A carried widget can show a picture of itself under the pointer instead of the small chip.

mod common;

use common::{paint_via_child, press_at, release_at};
use heca_grid_ui::builders::StyleExt;
use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{HostCmd, HostDraw};
use heca_grid_ui::widgets::Surface;
use heca_grid_ui::{Color, DrawCommand, Event, LayoutEngine, Point, Size, Theme};

/// A 100 × 50 red source at the origin, picked up at (10, 10) and carried to (60, 30).
fn carried(chip: bool) -> (Flex, Theme) {
    let mut source = Surface::row()
        .width(100.0)
        .height(50.0)
        .background(Color::rgb(200, 0, 0))
        .key("src")
        .draggable_as("pane");
    if chip {
        source = source.drag_chip();
    }
    let mut root = Flex::row().width(300.0).height(100.0).child(source);
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
    press_at(&mut root, Point::new(10.0, 10.0), PointerButton::Left);
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(60.0, 30.0)));
    (root, Theme::default())
}

fn reds(root: &Flex, theme: &Theme) -> Vec<(f64, f64, f64, f64, u8)> {
    paint_via_child(root, theme)
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.fill.r == 200 && r.fill.g == 0 => Some((
                r.rect.loc.x,
                r.rect.loc.y,
                r.rect.size.w,
                r.rect.size.h,
                r.fill.a,
            )),
            _ => None,
        })
        .collect()
}

/// **A source is carried as a picture of itself, without asking** — the widget itself, moved so the
/// point grabbed is under the pointer, shrunk toward that point by the theme's scale and faded by
/// its opacity. Any widget that can be dragged gets it, as in the browser.
#[test]
fn a_source_is_carried_as_a_faded_smaller_picture_of_itself() {
    let (root, theme) = carried(false);
    let alpha = (255.0 * theme.colors.drag_image_alpha).round() as u8;
    assert_eq!(theme.colors.drag_image_scale, 0.5);
    let drawn = reds(&root, &theme);
    // Where it sits: (0, 0) 100 × 50. The picture: moved by (50, 20), then halved about the
    // pointer (60, 30), so (55, 25) 50 × 25.
    assert!(
        drawn.contains(&(55.0, 25.0, 50.0, 25.0, alpha)),
        "the picture under the pointer: {drawn:?}"
    );
    assert!(
        drawn
            .iter()
            .any(|d| (d.0, d.1, d.2, d.3) == (0.0, 0.0, 100.0, 50.0)),
        "and the widget is still where it was: {drawn:?}"
    );
}

/// A source that asks for the chip gets it: no picture of it anywhere.
#[test]
fn a_source_that_asks_for_the_chip_shows_no_picture() {
    let (root, theme) = carried(true);
    let picture = reds(&root, &theme)
        .into_iter()
        .filter(|d| (d.0, d.1, d.2, d.3) != (0.0, 0.0, 100.0, 50.0))
        .count();
    assert_eq!(picture, 0);
}

/// **A picture of a surface is not where the surface is**: the host work the picture records is
/// marked an echo, and the real one is not.
#[test]
fn the_host_work_of_a_picture_is_marked_an_echo() {
    use heca_grid_ui::component::{Base, Component, PaintCx};
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
    let mut base = Base::new();
    base.style.layout.width = Length::Px(100.0);
    base.style.layout.height = Length::Px(50.0);
    base.draggable = true;
    base.drag_kind = Some("pane".into());
    base.key = Some("screen".into());
    let mut root = Flex::row()
        .width(300.0)
        .height(100.0)
        .child(Screen { base });
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
    press_at(&mut root, Point::new(10.0, 10.0), PointerButton::Left);
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(60.0, 30.0)));
    let theme = Theme::default();
    let scene = paint_via_child(&root, &theme);
    let surfaces: Vec<(bool, f32)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Host(HostCmd {
                draw: HostDraw::Surface { id: 9 },
                echo,
                alpha,
                ..
            }) => Some((*echo, *alpha)),
            _ => None,
        })
        .collect();
    assert_eq!(
        surfaces.len(),
        2,
        "the surface itself and its picture: {surfaces:?}"
    );
    assert!(
        surfaces.iter().any(|(echo, a)| !echo && *a == 1.0),
        "the real one: {surfaces:?}"
    );
    assert!(
        surfaces
            .iter()
            .any(|(echo, a)| *echo && (*a - theme.colors.drag_image_alpha).abs() < 1e-6),
        "the picture, faded: {surfaces:?}"
    );
    release_at(&mut root, Point::new(60.0, 30.0), PointerButton::Left);
}

/// A widget that only places surface 9 in its box, as a terminal does.
struct Screen {
    base: heca_grid_ui::component::Base,
}

impl heca_grid_ui::component::Component for Screen {
    fn base(&self) -> &heca_grid_ui::component::Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut heca_grid_ui::component::Base {
        &mut self.base
    }
    fn paint(&self, cx: &mut heca_grid_ui::component::PaintCx) {
        cx.surface(self.base.bounds, 9);
    }
}

/// **The picture of a round, clipping container cuts what it holds where the picture is**: the
/// surface inside is drawn under the round corners of the picture — moved and shrunk with it — and
/// the square clip shrinks too. Cut to the container where it lives instead, the picture's
/// content showed only where it overlapped the original.
#[test]
fn the_picture_of_a_round_container_cuts_its_surfaces_where_the_picture_is() {
    use heca_grid_ui::scene::Rounded;
    let mut base = heca_grid_ui::component::Base::new();
    base.style.layout.width = Length::Percent(1.0);
    base.style.layout.height = Length::Percent(1.0);
    let source = Pane::new()
        .width(100.0)
        .height(50.0)
        .radius(8.0)
        .clip_children(true)
        .key("src")
        .draggable_as("pane")
        .child(Screen { base });
    let mut root = Flex::row().width(300.0).height(100.0).child(source);
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
    press_at(&mut root, Point::new(10.0, 10.0), PointerButton::Left);
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(60.0, 30.0)));
    let theme = Theme::default();
    let scene = paint_via_child(&root, &theme);
    let rounded: Vec<(bool, Option<Rounded>)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Host(HostCmd {
                draw: HostDraw::Surface { id: 9 },
                echo,
                rounded,
                ..
            }) => Some((*echo, *rounded)),
            _ => None,
        })
        .collect();
    let at = |x, y, w, h| heca_grid_ui::Rectangle::new(Point::new(x, y), Size::new(w, h));
    assert!(
        rounded.contains(&(
            false,
            Some(Rounded {
                rect: at(0.0, 0.0, 100.0, 50.0),
                radius: 8.0
            })
        )),
        "the real one, under the container's corners: {rounded:?}"
    );
    // Moved by (50, 20), then halved about the pointer (60, 30).
    assert!(
        rounded.contains(&(
            true,
            Some(Rounded {
                rect: at(55.0, 25.0, 50.0, 25.0),
                radius: 4.0
            })
        )),
        "the picture, under the picture's corners: {rounded:?}"
    );
    assert!(
        scene
            .iter()
            .any(|c| *c == DrawCommand::PushClip(at(55.0, 25.0, 50.0, 25.0))),
        "and the square clip is the picture's box"
    );
    release_at(&mut root, Point::new(60.0, 30.0), PointerButton::Left);
}
