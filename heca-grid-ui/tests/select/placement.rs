//! **Where the open list goes** — hung from the trigger by its layer once layout is final: below
//! with room, flipped above near the window's bottom, and from where the trigger is on screen even
//! inside a page scrolled under it.

use super::*;

const WINDOW: Size = Size::new(800.0, 600.0);

/// A page holding `select` `top` px down, laid out in the window, then opened with a click and laid
/// out again (opening decides how many rows show). Returns the trigger and the list's rects.
fn open_at(top: f32) -> (Rectangle, Rectangle) {
    let mut page = Flex::column()
        .width(Length::Px(WINDOW.w as f32))
        .height(Length::Px(WINDOW.h as f32))
        .padding_top(top)
        .padding_left(40.0)
        .child(Select::new(["low", "medium", "high", "urgent"]));
    LayoutEngine::new().compute(&mut page, WINDOW);
    let trigger = page.base().children[0].base().bounds;
    let centre = Point::new(trigger.loc.x + 10.0, trigger.loc.y + trigger.size.h / 2.0);
    click_at(&mut page, centre, heca_grid_ui::PointerButton::Left);
    LayoutEngine::new().compute(&mut page, WINDOW);
    let trigger = page.base().children[0].base().bounds;
    let list = heca_grid_ui::area(&page, "list")
        .expect("the select's list")
        .base()
        .bounds;
    (trigger, list)
}

use heca_grid_ui::Rectangle;

/// With room below, the list hangs under the trigger: left-aligned to it, a gap down.
#[test]
fn the_open_list_sits_below_the_trigger_when_there_is_room() {
    let (trigger, list) = open_at(100.0);
    assert_eq!(list.loc.x, trigger.loc.x, "left-aligned to the trigger");
    let gap = list.loc.y - (trigger.loc.y + trigger.size.h);
    assert!(
        gap > 0.0 && gap < 10.0,
        "a small gap below the trigger: {gap}"
    );
}

/// Near the window's bottom the list flips **above** the trigger, still inside the window.
#[test]
fn the_open_list_flips_above_near_the_bottom_of_the_window() {
    let (trigger, list) = open_at(560.0);
    let gap = trigger.loc.y - (list.loc.y + list.size.h);
    assert!(
        gap > 0.0 && gap < 10.0,
        "its bottom a small gap above the trigger: {gap}"
    );
    assert!(list.loc.y >= 0.0, "still inside the window: {list:?}");
}

/// **Inside a page scrolled under it, the list hangs from where the trigger is on screen** — not
/// where it would be unscrolled, and not shifted twice. It was placed before the page moved its
/// content, so a scrolled page opened the list away from its control.
#[test]
fn inside_a_scrolled_page_the_list_hangs_from_the_trigger_on_screen() {
    let region = ScrollRegion::new()
        .grow(1.0)
        .child(Flex::column().height(Length::Px(300.0)))
        .child(Select::new(["low", "medium", "high"]))
        .child(Flex::column().height(Length::Px(900.0)));
    let offset = region.scroll_offset();
    let mut page = Flex::column()
        .width(Length::Px(WINDOW.w as f32))
        .height(Length::Px(WINDOW.h as f32))
        .child(region);
    LayoutEngine::new().compute(&mut page, WINDOW);
    offset.set(120.0);
    LayoutEngine::new().compute(&mut page, WINDOW);
    let trigger = page.base().children[0].base().children[1].base().bounds;
    let centre = Point::new(trigger.loc.x + 10.0, trigger.loc.y + trigger.size.h / 2.0);
    click_at(&mut page, centre, heca_grid_ui::PointerButton::Left);
    LayoutEngine::new().compute(&mut page, WINDOW);
    let trigger = page.base().children[0].base().children[1].base().bounds;
    let list = heca_grid_ui::area(&page, "list").unwrap().base().bounds;
    assert!(
        trigger.loc.y < 300.0,
        "the page has moved the trigger up by its scroll: {trigger:?}"
    );
    let gap = list.loc.y - (trigger.loc.y + trigger.size.h);
    assert!(
        gap > 0.0 && gap < 10.0,
        "the list hangs just under the trigger where it is drawn: trigger {trigger:?}, list {list:?}"
    );
}

/// **The open list keeps its frame** — the accent edge and its 16 px, 0.1 halo, drawn over the
/// page by its layer.
#[test]
fn the_open_list_wears_the_accent_edge_and_its_halo() {
    let mut sel = Select::new(["low", "medium", "high"]);
    LayoutEngine::new().compute(&mut sel, WINDOW);
    let b = sel.base().bounds;
    click_at(
        &mut sel,
        Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
        heca_grid_ui::PointerButton::Left,
    );
    LayoutEngine::new().compute(&mut sel, WINDOW);
    let theme = Theme::default();
    let accent = theme.colors.accent;
    let drawn = common::paint(&sel, &theme);
    assert!(
        drawn
            .overlay_layer()
            .iter()
            .any(|c| matches!(c, DrawCommand::Rect(r)
            if r.border.is_some_and(|e| e.color == accent)
                && r.glow.is_some_and(|g| (g.radius - 16.0).abs() < 0.01
                    && (g.intensity - 0.1).abs() < 0.01))),
        "the list's panel wears the accent edge and its halo"
    );
}

/// **An open list hanging outside a clipping holder still takes the press.** A select at the bottom
/// of a scrolling dock opens its list below the dock's edge — drawn over the page, unclipped — and
/// choosing an option there must choose it, though the dock cuts its own children at its edge.
#[test]
fn an_open_list_outside_a_clipping_holder_still_takes_the_press() {
    let region = ScrollRegion::new()
        .height(Length::Px(120.0))
        .child(Flex::column().height(Length::Px(70.0)))
        .child(Select::new(["low", "medium", "high"]));
    let mut page = Flex::column()
        .width(Length::Px(WINDOW.w as f32))
        .height(Length::Px(WINDOW.h as f32))
        .child(region);
    LayoutEngine::new().compute(&mut page, WINDOW);
    let trigger = page.base().children[0].base().children[1].base().bounds;
    let centre = Point::new(trigger.loc.x + 10.0, trigger.loc.y + trigger.size.h / 2.0);
    click_at(&mut page, centre, heca_grid_ui::PointerButton::Left);
    LayoutEngine::new().compute(&mut page, WINDOW);
    let rows = common::select_rows(&page);
    let last = rows[2].base().bounds;
    assert!(
        last.loc.y > 120.0,
        "the last option hangs below the dock's edge: {last:?}"
    );
    click_at(
        &mut page,
        Point::new(last.loc.x + 10.0, last.loc.y + last.size.h / 2.0),
        heca_grid_ui::PointerButton::Left,
    );
    let drawn = common::paint(&page, &Theme::default());
    let words: Vec<String> = drawn
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        words.iter().any(|w| w == "high"),
        "choosing the option below the dock's edge chose it: {words:?}"
    );
}
