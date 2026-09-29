use super::*;

/// **Collapsing a dock or a group asks for the layout pass that re-places its rows.**
///
/// The toggle runs on a CLICK, outside any layout pass, so applying `hidden` moves nothing on its
/// own — every row keeps the bounds it already had and is painted where it used to be. That is
/// what put a dock's rows on top of each other, and left nothing on screen when it was reopened,
/// until an unrelated window resize happened to run a pass (F003/P096).
#[test]
fn collapsing_a_dock_or_a_group_asks_for_a_layout() {
    use heca_grid_ui::{Component, DockFrame, Item, ItemGroup, PointerButton};

    // A dock and a group, each collapsed by the same gesture a user makes: a click on its header.
    let mut dock = DockFrame::new("PANES")
        .child(Item::new("zsh"))
        .child(Item::new("nvim"));
    dock.base_mut().style.layout.width = Length::Px(300.0);
    LayoutEngine::new()
        .base_font(14.0)
        .compute(&mut dock, Size::new(300.0, 400.0));
    let _ = heca_grid_ui::needs_layout(&dock);

    let header = dock.base().children[0].base().bounds;
    let at = Point::new(
        header.loc.x + header.size.w / 2.0,
        header.loc.y + header.size.h / 2.0,
    );
    heca_grid_ui::dispatch(&mut dock, &Event::pointer_pressed(at, PointerButton::Left));
    heca_grid_ui::dispatch(&mut dock, &Event::pointer_released(at, PointerButton::Left));
    assert!(
        heca_grid_ui::needs_layout(&dock),
        "a dock collapsed and nobody asked for a layout — its rows keep their old bounds",
    );

    let mut group = ItemGroup::new("COLUMN")
        .child(Item::new("a"))
        .child(Item::new("b"));
    group.base_mut().style.layout.width = Length::Px(300.0);
    LayoutEngine::new()
        .base_font(14.0)
        .compute(&mut group, Size::new(300.0, 400.0));
    let _ = heca_grid_ui::needs_layout(&group);

    let header = group.base().children[0].base().bounds;
    let at = Point::new(
        header.loc.x + header.size.w / 2.0,
        header.loc.y + header.size.h / 2.0,
    );
    heca_grid_ui::dispatch(&mut group, &Event::pointer_pressed(at, PointerButton::Left));
    heca_grid_ui::dispatch(
        &mut group,
        &Event::pointer_released(at, PointerButton::Left),
    );
    assert!(
        heca_grid_ui::needs_layout(&group),
        "a group collapsed and nobody asked for a layout — its rows keep their old bounds",
    );
}

/// **Showing or hiding a wrapped child asks for a layout pass**, because `hidden` is the engine's
/// `display: none` — flipping it moves every sibling, and a repaint alone redraws them all where
/// they used to be.
#[test]
fn revealing_a_hidden_child_asks_for_the_layout_that_moves_its_siblings() {
    use heca_grid_ui::{Component, Label, Visibility};

    let row = Visibility::new(Label::new("~/projects/heca"), false);
    let shown = row.visible_signal();
    let mut page = Flex::column().child(Label::new("zsh")).child(row);
    LayoutEngine::new()
        .base_font(14.0)
        .compute(&mut page, Size::new(300.0, 200.0));
    let _ = heca_grid_ui::needs_layout(&page);

    page.tick(1.0 / 60.0);
    assert!(
        !heca_grid_ui::needs_layout(&page),
        "nothing changed, nothing to lay out"
    );

    // The path arrives.
    shown.set(true);
    page.tick(1.0 / 60.0);
    assert!(
        heca_grid_ui::needs_layout(&page),
        "a row appeared and nobody asked for a layout — it stays collapsed until something else does",
    );
}
