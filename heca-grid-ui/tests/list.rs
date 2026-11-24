//! `List`: builds only what is on screen; the keyboard moves and chooses.

mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Label, List};
use heca_grid_ui::{Event, LayoutEngine, Size, Theme, WidgetIntent};

fn names(n: usize) -> Signal<Vec<String>> {
    signal((0..n).map(|i| format!("Item {i:03}")).collect())
}

/// A frame as a host draws one: tick, lay out, paint — returning the texts drawn.
fn frame(list: &mut dyn Component, viewport: Size) -> Vec<String> {
    list.tick(0.016);
    LayoutEngine::new().compute(list, viewport);
    common::texts(&common::paint_in(list, &Theme::default(), viewport))
        .into_iter()
        .map(|t| t.text)
        .collect()
}

/// **Three hundred items, a screenful built.** The rows on screen are drawn, and the list builds
/// only those and a few beyond — a palette that built every row took a third of a second to open.
#[test]
fn a_long_list_builds_only_what_is_on_screen() {
    let built = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let counted = built.clone();
    let mut list = List::new(names(300))
        .height(Length::FULL)
        .render_item(move |name, _| {
            counted.set(counted.get() + 1);
            Label::new(name.clone())
        });
    let viewport = Size::new(400.0, 300.0);
    for _ in 0..3 {
        frame(&mut list, viewport);
    }
    let drawn = frame(&mut list, viewport);
    assert!(
        drawn.contains(&"Item 000".to_string()),
        "the first rows are drawn: {drawn:?}"
    );
    assert!(built.get() < 60, "built {} of 300 items", built.get());
}

/// **The arrows move the selection and the list follows it; Enter chooses it.** Moving down past the
/// bottom brings the row into view, even one that was not built when the move began.
#[test]
fn the_keyboard_moves_the_selection_into_view_and_enter_chooses_it() {
    let chosen = std::rc::Rc::new(std::cell::Cell::new(None));
    let got = chosen.clone();
    let selected = signal(Some(0));
    let mut list = List::new(names(300))
        .height(Length::FULL)
        .render_item(|name, _| Label::new(name.clone()))
        .selected(selected)
        .on_choose(move |at| got.set(Some(at)));
    common::give_keyboard(&mut list);
    let viewport = Size::new(400.0, 300.0);
    frame(&mut list, viewport);
    for _ in 0..40 {
        heca_grid_ui::dispatch(&mut list, &Event::Widget(WidgetIntent::MenuDown));
        frame(&mut list, viewport);
    }
    let drawn = frame(&mut list, viewport);
    assert_eq!(selected.get_untracked(), Some(40));
    assert!(
        drawn.contains(&"Item 040".to_string()),
        "row 40 was brought into view: {drawn:?}"
    );
    assert!(
        !drawn.contains(&"Item 000".to_string()),
        "and the top scrolled away"
    );

    heca_grid_ui::dispatch(&mut list, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(chosen.get(), Some(40), "Enter chose the selected row");
}

/// **Writing the selection selects that row and brings it into view** — far down the list, where
/// nothing was built.
#[test]
fn writing_the_selection_brings_a_far_row_into_view() {
    let selected = signal(None);
    let mut list = List::new(names(300))
        .height(Length::FULL)
        .render_item(|name, _| Label::new(name.clone()))
        .selected(selected);
    let viewport = Size::new(400.0, 300.0);
    frame(&mut list, viewport);
    frame(&mut list, viewport);
    selected.set(Some(250));
    for _ in 0..4 {
        frame(&mut list, viewport);
    }
    let drawn = frame(&mut list, viewport);
    assert!(
        drawn.contains(&"Item 250".to_string()),
        "row 250 is on screen: {drawn:?}"
    );
}

/// **`visible_items(n)` makes the list exactly `n` of its items tall** — from the first frame, before
/// anything was measured, and after; in a box far taller than that.
#[test]
fn visible_items_makes_the_list_that_many_items_tall_from_the_first_frame() {
    use heca_grid_ui::widgets::Flex;
    let mut root = Flex::column().height(Length::FULL).child(
        List::new(names(300))
            .render_item(|name, _| Label::new(name.clone()))
            .visible_items(5),
    );
    let viewport = Size::new(400.0, 800.0);
    let list_h = |root: &mut Flex| {
        root.tick(0.016);
        LayoutEngine::new().compute(root, viewport);
        root.base().children[0].base().bounds.size.h
    };
    let first = list_h(&mut root);
    let item = {
        let list = &root.base().children[0];
        let region = &list.base().children[0];
        let content = &region.base().children[0];
        content.base().children[1].base().bounds.size.h
    };
    assert!(item > 0.0);
    assert!(
        (first - 5.0 * item).abs() < 1.0,
        "first frame: {first} for items of {item}"
    );
    for _ in 0..3 {
        list_h(&mut root);
    }
    let later = list_h(&mut root);
    assert!(
        (later - 5.0 * item).abs() < 1.0,
        "measured: {later} for items of {item}"
    );
}
