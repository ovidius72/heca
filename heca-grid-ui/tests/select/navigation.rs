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

    // A row past the window is collapsed: it is not drawn, so it cannot be hit either. (Were its
    // stale bounds left behind, they would sit under the trigger and swallow clicks.)
    for i in 6..20 {
        assert_eq!(
            sel.base().children[i].base().bounds.size,
            Size::new(0.0, 0.0),
            "row {i} is outside the visible window",
        );
    }
    // Closing collapses every row **except the chosen one**, which goes back to standing in the
    // trigger (that is how the trigger shows the option's own content).
    heca_grid_ui::dispatch(
        &mut sel,
        &Event::Widget(heca_grid_ui::WidgetIntent::Dismiss),
    );
    let chosen = sel.index();
    for i in 0..20 {
        if i == chosen {
            continue;
        }
        assert_eq!(
            sel.base().children[i].base().bounds.size,
            Size::new(0.0, 0.0),
            "row {i} is collapsed while the list is closed",
        );
    }
    let trigger = sel.base().bounds;
    assert!(
        trigger.contains(Point::new(
            trigger.loc.x + 5.0,
            sel.base().children[chosen].base().bounds.loc.y + 2.0,
        )),
        "the chosen option stands in the trigger",
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

    let first_row = ui.base().children[0].base().children[0].base().bounds;
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

    // Trigger label (1) + at most MAX_VISIBLE (6) rows are painted.
    let texts = row_texts(&sel);
    assert_eq!(texts.len(), 1 + 6, "long list caps the visible rows");
    assert_eq!(texts[1], "OPT0", "starts at the top");

    // Wheel-scroll moves the visible window down.
    let over_list = Point::new(b.loc.x + 5.0, b.loc.y + b.size.h + 5.0);
    heca_grid_ui::dispatch(&mut sel, &Event::wheel(over_list, 0.0, 5.0));
    assert_eq!(row_texts(&sel)[1], "OPT5", "scroll reveals later options");

    // Scrolling past the end clamps to the last full window.
    heca_grid_ui::dispatch(&mut sel, &Event::wheel(over_list, 0.0, 999.0));
    assert_eq!(row_texts(&sel)[1], "OPT14", "scroll clamps at max (20 - 6)");
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
/// its pill stops at the chevron gutter — so the text lit up and the caret beside it did not
/// (F003/P096/T483).
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
    for (i, option) in sel.base().children.iter().enumerate() {
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
/// over whatever was underneath (F003/P096/T483).
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
