use super::*;

#[test]
fn row_lays_children_left_to_right_with_gap() {
    let mut root = Flex::row()
        .gap(10.0)
        .width(Length::Px(300.0))
        .height(Length::Px(100.0))
        .child(fixed_box(50.0, 40.0))
        .child(fixed_box(50.0, 40.0));

    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));

    assert_eq!(root.base().bounds.size.w, 300.0);
    let first = &root.base().children[0];
    let second = &root.base().children[1];
    assert_eq!(first.base().bounds.loc.x, 0.0);
    // second sits after the first (50px) plus the 10px gap.
    assert_eq!(second.base().bounds.loc.x, 60.0);
}

#[test]
fn padding_offsets_child_origin() {
    let mut root = Flex::column()
        .padding(10.0)
        .width(Length::Px(200.0))
        .height(Length::Px(200.0))
        .child(fixed_box(50.0, 50.0));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 200.0));

    let child = &root.base().children[0];
    assert_eq!(child.base().bounds.loc.x, 10.0);
    assert_eq!(child.base().bounds.loc.y, 10.0);
}

#[test]
fn flex_grow_absorbs_remaining_space() {
    let mut root = Flex::row()
        .width(Length::Px(200.0))
        .height(Length::Px(50.0))
        .child(fixed_box(40.0, 50.0))
        .child(Flex::row().grow(1.0).height(Length::Px(50.0)));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    let grower = &root.base().children[1];
    // 200 total - 40 fixed = 160 absorbed by the growing child.
    assert_eq!(grower.base().bounds.size.w, 160.0);
    assert_eq!(grower.base().bounds.loc.x, 40.0);
}

#[test]
fn grid_places_children_in_named_areas_and_cells() {
    use heca_grid_ui::{Grid, Track};
    // 2 cols × 2 rows; areas: icon spans both rows in col 1, title top-right,
    // sub bottom-right. Fixed sizes so we can assert exact bounds.
    let mut grid = Grid::new()
        .template_column([Track::Px(40.0), Track::Px(100.0)])
        .template_row([Track::Px(20.0), Track::Px(20.0)])
        .template_area(["icon title", "icon sub"])
        .child(Flex::column().area("icon"))
        .child(Flex::column().area("title"))
        .child(Flex::column().area("sub"))
        // explicit placement: a 4th child pinning its own column and row.
        .child(Flex::column().column(2).row(2));

    LayoutEngine::new().compute(&mut grid, Size::new(140.0, 40.0));

    let icon = grid.base().children[0].base().bounds;
    let title = grid.base().children[1].base().bounds;
    let sub = grid.base().children[2].base().bounds;

    // icon: col 1 (x≈0), spans both rows (height≈40).
    assert!(icon.loc.x < 1.0, "icon in column 1");
    assert!((icon.size.h - 40.0).abs() < 1.0, "icon spans both rows");
    // title: col 2 (x≈40), top row (y≈0).
    assert!((title.loc.x - 40.0).abs() < 1.0, "title in column 2");
    assert!(title.loc.y < 1.0, "title in top row");
    // sub: col 2, bottom row (y≈20).
    assert!((sub.loc.x - 40.0).abs() < 1.0, "sub in column 2");
    assert!((sub.loc.y - 20.0).abs() < 1.0, "sub in bottom row");
}

#[test]
fn grid_areas_template_defines_the_rows_not_the_row_tracks() {
    use heca_grid_ui::{Grid, Track};

    // The template is what defines the structure; `rows(..)` only *sizes* the tracks it implies.
    // A template with more lines than there are row tracks therefore creates **implicit** rows —
    // and an item spanning them is centred over a taller area than its neighbours, so it silently
    // stops sharing their centre line. This is the mistake that reads as "the text is off-centre".
    let centres = |areas: &[&str]| {
        let mut grid = Grid::new()
            .template_column([Track::Px(30.0), Track::Fr(1.0)])
            .template_row([Track::Auto]) // one row track, whatever the template says
            .template_area(areas.to_vec())
            .align(Align::Center)
            .child(
                Surface::new()
                    .width(Length::Px(26.0))
                    .height(Length::Px(26.0))
                    .area("icon"),
            )
            .child(
                Surface::new()
                    .width(Length::Px(40.0))
                    .height(Length::Px(10.0))
                    .area("title"),
            );
        LayoutEngine::new().compute(&mut grid, Size::new(200.0, 60.0));
        let mid = |i: usize| {
            let b = grid.base().children[i].base().bounds;
            b.loc.y + b.size.h / 2.0
        };
        (mid(0), mid(1))
    };

    // One line in, one row out: the icon and the title share a centre line.
    let (icon, title) = centres(&["icon title"]);
    assert!(
        (icon - title).abs() < 0.5,
        "a one-line template centres both in the same row: icon {icon}, title {title}",
    );

    // Two lines in — even with a single row *track* — gives the icon an implicit second row to span,
    // and the two centres part company.
    let (icon, title) = centres(&["icon title", "icon ."]);
    assert!(
        (icon - title).abs() > 0.5,
        "the template's second line adds an implicit row the icon spans: icon {icon}, title {title}",
    );
}

#[test]
fn grid_items_align_in_their_cell_on_both_axes() {
    use heca_grid_ui::{Grid, Track};

    // One 100×40 cell holding a 20×10 item, so the alignment is unambiguous.
    let item = || {
        Surface::new()
            .width(Length::Px(20.0))
            .height(Length::Px(10.0))
    };
    let cell = |grid: Grid| {
        let mut grid = grid;
        LayoutEngine::new().compute(&mut grid, Size::new(100.0, 40.0));
        grid.base().children[0].base().bounds
    };

    // Default (Stretch on both axes): the item is pinned to the top-left of its cell — an explicit
    // size means there is nothing to stretch. This is why an Icon (h = font) and a Label
    // (h = font × 1.4) in the same row do NOT share a centre line by default.
    let default = cell(
        Grid::new()
            .template_column([Track::Px(100.0)])
            .template_row([Track::Px(40.0)])
            .child(item()),
    );
    assert!(
        default.loc.y < 0.01,
        "default: pinned to the top of the cell"
    );
    assert!(
        default.loc.x < 0.01,
        "default: pinned to the left of the cell"
    );

    // `.align(..)` is the VERTICAL knob: it centres the items in their cells.
    let centered = cell(
        Grid::new()
            .template_column([Track::Px(100.0)])
            .template_row([Track::Px(40.0)])
            .align(Align::Center)
            .child(item()),
    );
    assert!(
        (centered.loc.y - 15.0).abs() < 0.5,
        "align(Center) centres vertically: (40 - 10) / 2 = 15, got {}",
        centered.loc.y,
    );

    // `.justify_items(..)` is the HORIZONTAL one.
    let justified = cell(
        Grid::new()
            .template_column([Track::Px(100.0)])
            .template_row([Track::Px(40.0)])
            .justify_items(Align::Center)
            .child(item()),
    );
    assert!(
        (justified.loc.x - 40.0).abs() < 0.5,
        "justify_items(Center) centres horizontally: (100 - 20) / 2 = 40, got {}",
        justified.loc.x,
    );

    // The per-item overrides win over the grid's defaults, one axis each.
    let overridden = cell(
        Grid::new()
            .template_column([Track::Px(100.0)])
            .template_row([Track::Px(40.0)])
            .align(Align::Center)
            .justify_items(Align::Center)
            .child(item().align_self(Align::End).justify_self(Align::End)),
    );
    assert!(
        (overridden.loc.y - 30.0).abs() < 0.5 && (overridden.loc.x - 80.0).abs() < 0.5,
        "align_self / justify_self override the grid, got {overridden:?}",
    );

    // The trap this exists to avoid: on a grid, `justify` is `justify-content` — it distributes the
    // whole TRACK SET inside the container and does not move the item within its cell. With one
    // 100px track filling a 100px container there is nothing to distribute, so the item stays put.
    let justify_content = cell(
        Grid::new()
            .template_column([Track::Px(100.0)])
            .template_row([Track::Px(40.0)])
            .justify(Justify::Center)
            .child(item()),
    );
    assert!(
        justify_content.loc.x < 0.01,
        "`justify` does not align items in their cells — use `justify_items`",
    );
}

/// A margin can be set per **axis**, not only per side.
///
/// `padding_x` / `padding_y` existed and the margins had no counterpart, so a described tree could
/// say "padding on the y axis" but had to name both sides for a margin. A rule between two
/// containers wanting to breathe on one axis is the case that found it. The cascade matches
/// padding's: a side wins over its axis, which wins over the uniform value.
#[test]
fn a_margin_can_be_set_per_axis() {
    let mut root = Flex::column()
        .width(Length::Px(200.0))
        .height(Length::Px(200.0))
        .child(
            Flex::column()
                .height(Length::Px(20.0))
                .margin_y(10.0)
                .margin_x(4.0),
        )
        .child(Flex::column().height(Length::Px(20.0)));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 200.0));

    let first = root.base().children[0].base().bounds;
    let second = root.base().children[1].base().bounds;
    assert_eq!(
        first.loc.y, 10.0,
        "the y margin pushed it down from the top"
    );
    assert_eq!(first.loc.x, 4.0, "and the x margin in from the left");
    assert_eq!(
        second.loc.y, 40.0,
        "the next child clears the first's 20px height plus 10px of margin either side",
    );

    // A side still wins over its axis.
    let mut root = Flex::column().height(Length::Px(200.0)).child(
        Flex::column()
            .height(Length::Px(20.0))
            .margin_y(10.0)
            .margin_top(2.0),
    );
    LayoutEngine::new().compute(&mut root, Size::new(200.0, 200.0));
    assert_eq!(
        root.base().children[0].base().bounds.loc.y,
        2.0,
        "margin_top overrides margin_y",
    );
}

/// **`CardGrid` walks three axes.** Columns on `item_*`, the cell within a column on `menu_*`, and
/// the outer row on `menu_history_*` — all on the shared vocabulary, so the widget owns no keys.
#[test]
fn a_card_grid_walks_three_axes_and_returns_the_callers_key() {
    use heca_grid_ui::WidgetIntent;
    use heca_grid_ui::reactive::{SignalGet, signal};
    use heca_grid_ui::widgets::{CardGrid, Flex, GridCell, Label};

    let lit: Vec<Signal<bool>> = (0..5).map(|_| signal(false)).collect();
    let chosen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let dismissed = std::rc::Rc::new(std::cell::Cell::new(false));
    let (c, d) = (chosen.clone(), dismissed.clone());

    // Row 0: two columns, the first holding two stacked cells. Row 1: one column, one cell.
    let mut grid = CardGrid::new()
        .row(
            vec![
                vec![GridCell::new("a", lit[0]), GridCell::new("b", lit[1])],
                vec![GridCell::new("c", lit[2])],
            ],
            Flex::row().child(Label::new("row0")),
        )
        .row(
            vec![vec![GridCell::new("d", lit[3])]],
            Flex::row().child(Label::new("row1")),
        )
        .on_activate(move |key| *c.borrow_mut() = key.to_string())
        .on_dismiss(move || d.set(true))
        .selected("a");
    give_keyboard(&mut grid);

    assert_eq!(grid.selected_key(), Some("a"));

    // A column with depth: menu_down walks WITHIN it — the case that used to switch workspace.
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::MenuDown));
    assert_eq!(
        grid.selected_key(),
        Some("b"),
        "j moves to the next pane in the split column"
    );
    assert!(
        lit[1].get_untracked() && !lit[0].get_untracked(),
        "exactly one cell is lit"
    );

    // item_next crosses to the next COLUMN, clamping the cell index into the shorter column.
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(grid.selected_key(), Some("c"), "l moves a column right");

    // menu_history_* is the OUTER axis here — the workspace.
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::MenuHistoryDown));
    assert_eq!(
        grid.selected_key(),
        Some("d"),
        "n moves to the next workspace"
    );
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::MenuHistoryDown));
    assert_eq!(grid.selected_key(), Some("d"), "and stops at the last one");

    // **A row remembers where you were in it.** Coming back lands on the card you left, not on the
    // row's first — the position used to be destroyed by the trip (the cursor carried its column
    // index across, clamped it into the shorter row, and clamped it again on the way back).
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::MenuHistoryUp));
    assert_eq!(
        grid.selected_key(),
        Some("c"),
        "back to the column we left row 0 on"
    );
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::MenuDown));
    assert_eq!(
        grid.selected_key(),
        Some("d"),
        "no depth here, so j moved to the next row"
    );

    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(chosen.borrow().as_str(), "d");
    heca_grid_ui::dispatch(&mut grid, &Event::Widget(WidgetIntent::Dismiss));
    assert!(dismissed.get());
}
