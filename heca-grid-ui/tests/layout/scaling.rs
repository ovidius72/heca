use super::*;

#[test]
fn widget_size_scales_font_and_box_proportionally() {
    // Big is the reference look; Normal (the default) and Small scale down — font
    // and the whole box shrink together so the control stays balanced.
    let measure = |size: WidgetSize| -> (f32, Rectangle) {
        let mut b = Button::new("RUN").size(size);
        LayoutEngine::new().compute(&mut b, Size::new(400.0, 100.0));
        (b.base().font, b.base().bounds)
    };
    let (small_f, small_b) = measure(WidgetSize::Small);
    let (normal_f, normal_b) = measure(WidgetSize::Normal);
    let (big_f, big_b) = measure(WidgetSize::Large);

    assert!(
        small_f < normal_f && normal_f < big_f,
        "font grows Small < Normal < Big"
    );
    assert!(
        small_b.size.h < normal_b.size.h && normal_b.size.h < big_b.size.h,
        "box height grows with the size variant"
    );
    assert!(
        small_b.size.w < big_b.size.w,
        "box width grows with the size variant"
    );

    // The default is Normal.
    let mut default_btn = Button::new("RUN");
    LayoutEngine::new().compute(&mut default_btn, Size::new(400.0, 100.0));
    assert_eq!(default_btn.base().font, normal_f, "default size is Normal");
}

#[test]
fn widget_size_scales_text_only_widgets_via_font() {
    // A Label has no padding, so the size variant shows purely as a smaller font.
    let font = |size: WidgetSize| {
        let mut l = Label::new("status").size(size);
        LayoutEngine::new().compute(&mut l, Size::new(200.0, 50.0));
        l.base().font
    };
    assert!(
        font(WidgetSize::Small) < font(WidgetSize::Normal)
            && font(WidgetSize::Normal) < font(WidgetSize::Large),
        "text widgets inherit the size variant through the resolved font"
    );
}

/// **A zoom scales the picture, not just the box** (F003/P082/T327 item 1).
///
/// An overview opens by zooming out from life size, which a fade cannot express. The trap is
/// scaling geometry alone: a half-size card whose text is still 14px, whose radius is still 6px and
/// whose 1px border is still 1px is not the same picture further away. So every pixel-measured
/// value rides the same transform.
#[test]
fn a_scaled_subtree_shrinks_its_text_radius_and_border_with_its_box() {
    use heca_grid_ui::component::PaintCx;
    use heca_grid_ui::scene::{DrawCommand, Scene};
    use heca_grid_ui::{Color, Point, Rectangle, Size, Theme};

    let theme = Theme::default();
    let draw = |scale: f32| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            let paint = |cx: &mut PaintCx<'_>| {
                cx.rect(
                    Rectangle::new(Point::new(100.0, 100.0), Size::new(200.0, 80.0)),
                    Color::rgb(10, 20, 30),
                    Some(heca_grid_ui::scene::Border {
                        color: Color::rgb(1, 2, 3),
                        width: 2.0,
                    }),
                    8.0,
                    None,
                );
                cx.text(
                    Rectangle::new(Point::new(100.0, 100.0), Size::new(200.0, 80.0)),
                    "zsh",
                    Color::rgb(200, 200, 200),
                    16.0,
                    heca_grid_ui::scene::TextAlign::Start,
                    heca_grid_ui::scene::TextStyle::default(),
                );
            };
            match scale {
                1.0 => paint(&mut cx),
                s => cx.with_scale(s, Point::new(0.0, 0.0), paint),
            }
        }
        scene
    };

    let life = draw(1.0);
    let half = draw(0.5);

    let rect_of = |s: &Scene| {
        s.iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some(*r),
                _ => None,
            })
            .expect("a rect")
    };
    let text_of = |s: &Scene| {
        s.iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) => Some(t.clone()),
                _ => None,
            })
            .expect("a text run")
    };

    let (a, b) = (rect_of(&life), rect_of(&half));
    assert_eq!(b.rect.size.w, a.rect.size.w / 2.0, "the box halves");
    assert_eq!(
        b.rect.loc.x,
        a.rect.loc.x / 2.0,
        "and moves toward the origin"
    );
    assert_eq!(b.radius, a.radius / 2.0, "the corner radius halves with it");
    assert_eq!(
        b.border.expect("border").width,
        a.border.expect("border").width / 2.0,
        "and so does the border, or a hairline becomes a slab",
    );

    let (ta, tb) = (text_of(&life), text_of(&half));
    assert_eq!(tb.size, ta.size / 2.0, "the text shrinks with its box");
    assert_eq!(tb.rect.loc.y, ta.rect.loc.y / 2.0);

    // Life size must be byte-for-byte what it always was: a zoom nobody asked for costs nothing.
    assert_eq!(life.len(), half.len());
}
