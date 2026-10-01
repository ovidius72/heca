use super::*;

// ── the wheel ─────────────────────────────────────────────────────────────────────────────────

/// The wheel carries a position, so it goes to what is under the pointer — and to nothing else.
#[test]
fn the_wheel_goes_to_what_is_under_it() {
    let (mut root, a, b) = two_probes();
    let _ = heca_grid_ui::dispatch(root.as_mut(), &Event::wheel(RIGHT, 0.0, 1.0));
    assert!(!kinds(&a).contains(&"Scroll".to_string()));
    assert!(kinds(&b).contains(&"Scroll".to_string()));
}

/// A region that cannot scroll the axis it was asked for **declines**, and the wheel carries on to
/// the region outside it. That is what makes nesting work with nothing declared.
#[test]
fn an_unscrollable_inner_region_lets_the_wheel_reach_the_outer_one() {
    let inner = ScrollRegion::new()
        .width(Length::Px(100.0))
        .height(Length::Px(50.0))
        .child(
            Flex::column()
                .width(Length::Px(80.0))
                .height(Length::Px(20.0)),
        );
    let mut outer = ScrollRegion::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0))
        .child(inner)
        .child(
            Flex::column()
                .width(Length::Px(180.0))
                .height(Length::Px(600.0)),
        );
    let offset = outer.scroll_offset();
    LayoutEngine::new().compute(&mut outer, Size::new(200.0, 100.0));

    let _ = heca_grid_ui::dispatch(&mut outer, &Event::wheel(Point::new(10.0, 10.0), 0.0, 1.0));
    assert!(
        offset.get_untracked() > 0.0,
        "the inner region had nothing to scroll, so the outer one did",
    );
}

/// **A wheel says whether its device counts pixels or notches** — the DOM's `deltaMode`. A trackpad
/// swipe arrives with its pixels; a mouse wheel does not. Both always carry the line deltas.
#[test]
fn a_wheel_carries_the_unit_its_device_counts_in() {
    type Pixels = Option<(f32, f32)>;
    let seen: Rc<RefCell<Vec<Pixels>>> = Rc::default();
    let record = seen.clone();
    let mut root = Flex::column()
        .width(Length::Px(100.0))
        .height(Length::Px(100.0))
        .on_scroll(move |cx| {
            record
                .borrow_mut()
                .push(cx.pointer().expect("a pointer event").delta_pixels)
        });
    LayoutEngine::new().compute(&mut root, Size::new(100.0, 100.0));
    let at = Point::new(10.0, 10.0);

    let _ = heca_grid_ui::dispatch(&mut root, &Event::wheel(at, 0.0, 1.0));
    let mut trackpad = heca_grid_ui::RawPointer::new(heca_grid_ui::RawPointerKind::Wheel, at);
    trackpad.delta_y = 0.7;
    trackpad.delta_pixels = Some((0.0, 14.0));
    let _ = heca_grid_ui::dispatch(&mut root, &Event::Raw(trackpad));

    assert_eq!(*seen.borrow(), vec![None, Some((0.0, 14.0))]);
}
