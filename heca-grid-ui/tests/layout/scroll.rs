use super::*;

/// Whole-page scroll premise (T009): a root `ScrollRegion` sized to the viewport,
/// holding a natural-width page column (`align(Start)`, width `Auto`), must report
/// horizontal overflow measured from that DIRECT child when a grandchild row is
/// wider than the viewport — with `flex_shrink: 0` the column adopts its widest
/// child instead of being clamped to the available width.
#[test]
fn root_scroll_region_sees_horizontal_overflow_through_a_natural_width_page() {
    let page = Flex::column()
        .align(Align::Center)
        .child(fixed_box(300.0, 20.0)) // wider than the 100px viewport
        .child(fixed_box(50.0, 20.0));
    let mut root = ScrollRegion::new()
        .both()
        .align(Align::Start) // don't stretch the page to the viewport width
        .width(Length::Px(100.0))
        .height(Length::Px(100.0))
        .child(page);

    LayoutEngine::new().compute(&mut root, Size::new(100.0, 100.0));

    let page_w = root.base().children[0].base().bounds.size.w;
    assert!(
        page_w >= 300.0,
        "page column adopts its widest child (got {page_w}), not the viewport width"
    );
    // The region measures overflow from its direct child → horizontal scrolling works.
    assert!(
        root.scroll_to_x(10_000.0) > 0.0,
        "root region reports a positive max horizontal offset"
    );
}
