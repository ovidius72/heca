//! **Rows wanting attention out of sight are counted at the edges, and a click goes to them** — a
//! `ScrollRegion` with an `EdgeHint` pinned in it.

#[path = "common/mod.rs"]
mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{EdgeHint, Row, ScrollRegion};
use heca_grid_ui::{DrawCommand, Event, FlashHandle, LayoutEngine, PointerButton, Size, Theme};
use std::cell::Cell;
use std::rc::Rc;

const SIZE: Size = Size::new(300.0, 200.0);

fn frame(region: &mut ScrollRegion) -> Vec<(String, heca_grid_ui::Rectangle)> {
    LayoutEngine::new().compute(region, SIZE);
    region.tick(0.016);
    LayoutEngine::new().compute(region, SIZE);
    let scene = common::paint_in(region, &Theme::default(), SIZE);
    scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.rect)),
            _ => None,
        })
        .collect()
}

#[test]
fn rows_wanting_attention_out_of_sight_are_counted_and_a_click_reveals_and_activates_one() {
    let activated = Rc::new(Cell::new(None::<usize>));
    let flashes: Vec<FlashHandle> = (0..12).map(|_| FlashHandle::new()).collect();
    let mut region = ScrollRegion::new().height(Length::Px(200.0));
    for (i, flash) in flashes.iter().enumerate() {
        let activated = activated.clone();
        region = region.child(
            Row::new()
                .flash(*flash)
                .height(Length::Px(40.0))
                .on_activate(move || activated.set(Some(i)))
                .child(Label::new(format!("row {i}"))),
        );
    }
    let mut region = region.pinned(EdgeHint::new());
    let texts = frame(&mut region);
    assert!(
        !texts
            .iter()
            .any(|(t, _)| t.starts_with('↑') || t.starts_with('↓')),
        "nothing wants attention: no hint"
    );

    flashes[0].repeat(Tone::Warning);
    flashes[11].repeat(Tone::Warning);
    region.scroll_to_child(5);
    let texts = frame(&mut region);
    let below = texts
        .iter()
        .find(|(t, _)| t == "↓ 1")
        .map(|(_, r)| *r)
        .expect("one wanting attention below");
    assert!(
        texts.iter().any(|(t, _)| t == "↑ 1"),
        "one wanting attention above: {texts:?}"
    );
    assert!(
        texts.iter().any(|(t, _)| t == "row 5"),
        "the rows under the hint still draw"
    );

    // A finite burst is not attention.
    flashes[0].clear();
    flashes[0].trigger(Tone::Success);
    let texts = frame(&mut region);
    assert!(
        !texts.iter().any(|(t, _)| t.starts_with('↑')),
        "a burst is not attention: {texts:?}"
    );

    let at = heca_grid_ui::Point::new(
        below.loc.x + below.size.w / 2.0,
        below.loc.y + below.size.h / 2.0,
    );
    heca_grid_ui::dispatch(
        &mut region,
        &Event::pointer_pressed(at, PointerButton::Left),
    );
    heca_grid_ui::dispatch(
        &mut region,
        &Event::pointer_released(at, PointerButton::Left),
    );
    let texts = frame(&mut region);
    assert!(
        texts.iter().any(|(t, _)| t == "row 11"),
        "the row is brought into view: {texts:?}"
    );
    assert_eq!(
        activated.get(),
        Some(11),
        "and activated, as a click on it would"
    );
}
