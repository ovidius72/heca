use super::*;

#[test]
fn select_rows_outside_the_visible_window_are_not_clickable() {
    let opts: Vec<String> = (0..20).map(|n| format!("OPT{n}")).collect();
    let mut sel = Select::new(opts);
    give_keyboard(&mut sel);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    let b = sel.base().bounds;
    click_at(
        &mut sel,
        Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
        PointerButton::Left,
    ); // open — 6 rows visible of 20
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    // A row past the visible window lies outside the panel: it is clipped, so a press where it
    // would be is not a press on it.
    let panel = heca_grid_ui::area(&sel, "list").unwrap().base().bounds;
    let past = common::select_rows(&sel)[10].base().bounds;
    assert!(
        past.loc.y >= panel.loc.y + panel.size.h,
        "row 10 is below the panel's window"
    );
    click_at(
        &mut sel,
        Point::new(past.loc.x + 10.0, past.loc.y + past.size.h / 2.0),
        PointerButton::Left,
    );
    assert_ne!(sel.index(), 10, "a clipped row cannot be chosen");

    // Closed, the list is not drawn and takes no press: only the trigger shows.
    heca_grid_ui::dispatch(
        &mut sel,
        &Event::Widget(heca_grid_ui::WidgetIntent::Dismiss),
    );
    assert!(!sel.overlay_active(), "the list is closed");
    let drawn = common::paint(&sel, &Theme::default());
    assert!(
        drawn.overlay_layer().is_empty(),
        "the closed list draws nothing over the page"
    );
}

#[test]
fn select_flips_above_the_trigger_when_there_is_no_room_below() {
    let theme = Theme::default();
    // The Select sits at the bottom of the viewport: the panel cannot open downward.
    let mut ui = Flex::column()
        .height(Length::Px(300.0))
        .justify(Justify::End)
        .child(Select::new(["A", "B", "C"]));
    LayoutEngine::new().compute(&mut ui, Size::new(300.0, 300.0));

    // Paint once so the widget learns the viewport height (that is what it flips against).
    let _ = common::paint_in(&ui, &theme, Size::new(300.0, 300.0));

    let trigger = ui.base().children[0].base().bounds;
    heca_grid_ui::dispatch(
        ui.base_mut().children[0].as_mut(),
        &Event::pointer_pressed(
            Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0),
            PointerButton::Left,
        ),
    );

    let first_row = common::select_rows(ui.base().children[0].as_ref())[0]
        .base()
        .bounds;
    assert!(
        first_row.loc.y + first_row.size.h <= trigger.loc.y,
        "no room below → the rows are placed above the trigger",
    );
}

#[test]
fn select_long_list_caps_visible_rows_and_scrolls() {
    let theme = Theme::default();
    let opts: Vec<String> = (0..20).map(|n| format!("OPT{n}")).collect();
    let mut sel = Select::new(opts);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    let row_texts = |s: &Select| -> Vec<String> {
        let scene = common::paint(s, &theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let b = sel.base().bounds;
    click_at(
        &mut sel,
        Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
        PointerButton::Left,
    ); // open

    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    // The panel is as tall as MAX_VISIBLE (6) rows: the seventh starts where the panel ends.
    let panel = heca_grid_ui::area(&sel, "list").unwrap().base().bounds;
    let rows = common::select_rows(&sel);
    let seventh = rows[6].base().bounds;
    assert!(
        (seventh.loc.y - (panel.loc.y + panel.size.h)).abs()
            <= rows[0].base().bounds.loc.y - panel.loc.y + 0.5,
        "six rows fit the panel: {panel:?} vs the seventh at {seventh:?}"
    );
    assert!(
        row_texts(&sel).contains(&"OPT0".to_string()),
        "starts at the top"
    );

    // The wheel over the list scrolls it.
    let over_list = Point::new(panel.loc.x + 5.0, panel.loc.y + 5.0);
    let first_before = common::select_rows(&sel)[0].base().bounds.loc.y;
    heca_grid_ui::dispatch(&mut sel, &Event::wheel(over_list, 0.0, 5.0));
    let first_after = common::select_rows(&sel)[0].base().bounds.loc.y;
    assert!(first_after < first_before, "the wheel scrolls the rows up");

    // Scrolling past the end stops with the last row at the panel's bottom.
    heca_grid_ui::dispatch(&mut sel, &Event::wheel(over_list, 0.0, 999.0));
    let last = common::select_rows(&sel)[19].base().bounds;
    assert!(
        last.loc.y + last.size.h <= panel.loc.y + panel.size.h + 0.5 && last.loc.y >= panel.loc.y,
        "the last row is in view at the end: {last:?} in {panel:?}"
    );
}

#[test]
fn select_keyboard_navigates_and_escape_closes() {
    use heca_grid_ui::WidgetIntent;
    let mut sel = Select::new(["A", "B", "C"]);
    give_keyboard(&mut sel);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));
    let raw = |s: &mut Select, k: GridKey| {
        heca_grid_ui::dispatch(
            &mut *s,
            &Event::Key {
                key: k,
                pressed: true,
            },
        )
    };
    let nav = |s: &mut Select, i: WidgetIntent| heca_grid_ui::dispatch(&mut *s, &Event::Widget(i));

    // A closed Select opens on a raw activation key (Enter/Space/↓), like a button.
    raw(&mut sel, GridKey::Enter);
    assert!(sel.overlay_active());
    // While open it is a vertical overlay: the host drives it with `MenuUp`/`MenuDown`.
    nav(&mut sel, WidgetIntent::MenuDown);
    nav(&mut sel, WidgetIntent::MenuDown);
    nav(&mut sel, WidgetIntent::Activate); // commit highlight (index 2)
    assert_eq!(sel.index(), 2);
    assert!(!sel.overlay_active(), "Activate commits and closes");

    raw(&mut sel, GridKey::Enter); // reopen
    assert!(sel.overlay_active());
    nav(&mut sel, WidgetIntent::Dismiss);
    assert!(
        !sel.overlay_active(),
        "Dismiss closes without changing selection"
    );
    assert_eq!(sel.index(), 2);
}

/// **A closed select is one hover target, not two.** The chosen option is echoed inside the
/// trigger, and while it stands there it is decoration: left hittable it hovered on its own, and
/// its pill stops at the chevron gutter — so the text lit up and the caret beside it did not.
#[test]
fn a_closed_select_hovers_as_one_control() {
    use heca_grid_ui::{DrawCommand, Select};

    let mut sel = Select::new(["NORMAL", "PREFIX"]);
    LayoutEngine::new().compute(&mut sel, Size::new(400.0, 200.0));
    let trigger = sel.base().bounds;
    heca_grid_ui::dispatch(
        &mut sel,
        &Event::pointer_moved(Point::new(
            trigger.loc.x + 8.0,
            trigger.loc.y + trigger.size.h / 2.0,
        )),
    );
    assert!(
        sel.base().hovered(),
        "the trigger is what the pointer found"
    );
    for (i, option) in common::select_rows(&sel).iter().enumerate() {
        assert!(
            !option.base().hovered(),
            "option {i} must not hover while it is standing in the trigger",
        );
    }

    // And the highlight covers the whole control, chevron included.
    let theme = Theme::default();
    let scene = common::paint(&sel, &theme);
    let widest = scene
        .base_layer()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.border.is_none() => Some(r.rect.size.w),
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    assert!(
        (widest - trigger.size.w).abs() < 0.5,
        "the hover fill spans {widest} of the trigger's {}",
        trigger.size.w,
    );
}

/// **An open dropdown does not drag the row it sits in.** The row's baseline rule hunts for text
/// inside each child so runs of different sizes line up; an open `Select` keeps its option rows in
/// a panel *below* itself, and that text was being counted as the line's deepest baseline — so
/// every label beside the select was dropped 40px to meet it, landing below the row and drawing
/// over whatever was underneath.
#[test]
fn an_open_select_leaves_the_row_around_it_alone() {
    use heca_grid_ui::{Align, Component, Flex, Label, LayoutExt, Length, Parent, Select};

    let mut row = Flex::row()
        .width(Length::Px(900.0))
        .gap(16.0)
        .align(Align::Center)
        .child(Label::new("MODE"))
        .child(Select::new(["NORMAL", "PREFIX", "PASSTHROUGH"]))
        .child(Label::new("WORKSPACE"));
    LayoutEngine::new().compute(&mut row, Size::new(900.0, 400.0));
    let closed: Vec<f64> = row
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.loc.y)
        .collect();

    let trigger = row.base().children[1].base().bounds;
    click_at(
        &mut row,
        Point::new(trigger.loc.x + 20.0, trigger.loc.y + trigger.size.h / 2.0),
        PointerButton::Left,
    );
    LayoutEngine::new().compute(&mut row, Size::new(900.0, 400.0));
    let open: Vec<f64> = row
        .base()
        .children
        .iter()
        .map(|c| c.base().bounds.loc.y)
        .collect();

    for (i, (before, after)) in closed.iter().zip(open.iter()).enumerate() {
        assert!(
            (before - after).abs() < 1.5,
            "child {i} moved from {before} to {after} when the select opened",
        );
    }
}

/// **The keyboard cursor stays in view.** Moving it past the panel's last row scrolls the list so
/// the row under it is shown — the option asks for it and the list's `ScrollRegion` follows, as it
/// does for any list's cursor.
#[test]
fn moving_the_cursor_past_the_panel_scrolls_the_row_into_view() {
    use heca_grid_ui::WidgetIntent;
    let opts: Vec<String> = (0..20).map(|n| format!("OPT{n}")).collect();
    let mut sel = Select::new(opts);
    give_keyboard(&mut sel);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));
    heca_grid_ui::dispatch(
        &mut sel,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    for _ in 0..10 {
        heca_grid_ui::dispatch(&mut sel, &Event::Widget(WidgetIntent::MenuDown));
        LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));
    }
    let panel = heca_grid_ui::area(&sel, "list").unwrap().base().bounds;
    let row = common::select_rows(&sel)[10].base().bounds;
    assert!(
        row.loc.y >= panel.loc.y - 0.5
            && row.loc.y + row.size.h <= panel.loc.y + panel.size.h + 0.5,
        "the cursor's row is in the panel: {row:?} in {panel:?}"
    );
}

/// **The option under the cursor shows it** — a highlight over that row's own box, moving with the
/// cursor.
#[test]
fn the_row_under_the_cursor_is_highlighted() {
    use heca_grid_ui::WidgetIntent;
    let theme = Theme::default();
    let mut sel = Select::new(["A", "B", "C"]);
    give_keyboard(&mut sel);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 300.0));
    heca_grid_ui::dispatch(
        &mut sel,
        &Event::Key {
            key: GridKey::Enter,
            pressed: true,
        },
    );
    heca_grid_ui::dispatch(&mut sel, &Event::Widget(WidgetIntent::MenuDown));
    let row = common::select_rows(&sel)[1].base().bounds;
    let painted = common::rects(&common::paint(&sel, &theme));
    assert!(
        painted.iter().any(|r| r.rect == row),
        "a highlight covers the cursor's row {row:?}"
    );
}

/// **Every row spans the whole list** — its pill, its hover and the cursor reach the panel's edges,
/// whatever its text — while the text starts where the trigger's label does.
#[test]
fn every_row_spans_the_whole_list() {
    let mut sel = Select::new(["A", "A MUCH LONGER OPTION"]);
    LayoutEngine::new().compute(&mut sel, Size::new(400.0, 300.0));
    let rows = common::select_rows(&sel);
    let (short, long) = (rows[0].base().bounds, rows[1].base().bounds);
    assert!(
        (short.size.w - long.size.w).abs() < 0.5,
        "a short option's row is as wide as a long one's: {short:?} vs {long:?}"
    );
}
