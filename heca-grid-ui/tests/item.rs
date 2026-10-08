mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

// ── Item (generic list row) ──

#[test]
fn item_activates_on_click_when_interactive() {
    use std::cell::Cell;
    use std::rc::Rc;

    let hits = Rc::new(Cell::new(0u32));
    let h = hits.clone();
    let mut item = Item::new("VIEW PROFILE").on_activate(move || h.set(h.get() + 1));
    LayoutEngine::new().compute(&mut item, Size::new(260.0, 40.0));

    assert!(item.focusable(), "interactive item is focusable");
    let b = item.base().bounds;
    click_at(
        &mut item,
        Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(hits.get(), 1, "click activates the row");

    // Space activates too (keyboard).
    // A raw key reaches only the widget that owns the keyboard — focus it, as a real surface
    // would before sending one.
    item.base().focus(true);
    heca_grid_ui::dispatch(
        &mut item,
        &Event::Key {
            key: GridKey::Space,
            pressed: true,
        },
    );
    assert_eq!(hits.get(), 2);
}

#[test]
fn display_only_item_is_inert_and_unfocusable() {
    let mut item = Item::new("STATIC");
    LayoutEngine::new().compute(&mut item, Size::new(260.0, 40.0));
    assert!(
        !item.focusable(),
        "an item without on_activate is not focusable"
    );
    let b = item.base().bounds;
    // No panic / no effect; just confirms it ignores the press.
    assert_eq!(
        heca_grid_ui::dispatch(
            &mut item,
            &Event::pointer_pressed(
                Point::new(b.loc.x + 1.0, b.loc.y + 1.0),
                PointerButton::Left
            )
        ),
        Handled::No,
    );
}

#[test]
fn item_label_color_tracks_selected_state() {
    let theme = Theme::default();
    let label_color = |item: &Item| {
        let scene = common::paint(item, &theme);
        scene.iter().find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.color),
            _ => None,
        })
    };

    let mut plain = Item::new("PROGRAMS");
    LayoutEngine::new().compute(&mut plain, Size::new(260.0, 40.0));
    let mut sel = Item::new("DASHBOARD").active(true);
    LayoutEngine::new().compute(&mut sel, Size::new(260.0, 40.0));

    assert_eq!(
        label_color(&plain),
        Some(theme.colors.foreground),
        "plain label uses foreground"
    );
    assert_eq!(
        label_color(&sel),
        Some(theme.colors.accent),
        "active label uses accent"
    );
}

#[test]
fn item_slots_lay_out_left_and_right() {
    // Leading badge on the left, trailing hint on the right; label sits between.
    let mut item = Item::new("SETTINGS")
        .leading(StatusDot::online())
        .trailing(Badge::neutral("CMD ,"));
    LayoutEngine::new().compute(&mut item, Size::new(300.0, 40.0));

    let b = item.base().bounds;
    let lead = item.base().children[0].base().bounds; // leading
    let trail = item.base().children[1].base().bounds; // trailing
    assert!(lead.loc.x < trail.loc.x, "leading sits left of trailing");
    assert!(
        trail.loc.x + trail.size.w <= b.loc.x + b.size.w + 0.5,
        "trailing stays within the row's right edge"
    );
    assert!(
        lead.size.w > 0.0 && trail.size.w > 0.0,
        "both slots are laid out"
    );
}

#[test]
fn item_trailing_border_draws_a_flat_frame_no_glow() {
    let theme = Theme::default();
    let frames = |item: &Item| -> usize {
        let scene = common::paint(item, &theme);
        // A bordered, glow-free rect = the chip frame.
        scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some() && r.glow.is_none()))
            .count()
    };

    let mut plain = Item::new("SETTINGS").trailing(Label::new("CMD ,"));
    LayoutEngine::new().compute(&mut plain, Size::new(300.0, 40.0));
    let mut bordered = Item::new("SETTINGS")
        .trailing(Label::new("CMD ,"))
        .trailing_bordered(true);
    LayoutEngine::new().compute(&mut bordered, Size::new(300.0, 40.0));

    assert_eq!(frames(&plain), 0, "no frame without trailing_bordered");
    assert_eq!(
        frames(&bordered),
        1,
        "trailing_bordered draws one flat frame"
    );
}

#[test]
fn pane_draws_rounded_accent_border_no_brackets() {
    let theme = Theme::default();
    let mut pane = Pane::new()
        .background(theme.colors.surface)
        .border(theme.colors.accent, theme.colors.border_width)
        .child(Label::new("X"));
    LayoutEngine::new().compute(&mut pane, Size::new(200.0, 300.0));

    let scene = common::paint(&pane, &theme);
    // New design: the corner brackets are segments of a *rounded border* (so they
    // share the theme radius), drawn with rects + dimmed straights — not the flat,
    // always-square `BracketCmd` primitive, and never glowing.
    let brackets = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Brackets(_)))
        .count();
    assert_eq!(
        brackets, 0,
        "pane no longer uses the square bracket primitive"
    );

    let rounded_border = scene.iter().any(|c| {
        matches!(
            c,
            DrawCommand::Rect(r)
                if r.border.is_some() && r.radius == theme.colors.border_radius
        )
    });
    assert!(
        rounded_border,
        "pane draws a rounded accent border at the theme radius"
    );

    // The surface carries the faint theme REST glow (`interaction.control_rest_glow`)
    // so the `glow_size` setting visibly scales panes at rest too (T011).
    let expected_i = theme.colors.interaction.control_rest_glow as f32 / 255.0;
    let rest_glow = scene.iter().any(|c| {
        matches!(
            c,
            DrawCommand::Rect(r)
                if r.border.is_some()
                    && r.glow.is_some_and(|g| (g.intensity - expected_i).abs() < 1e-6)
        )
    });
    assert!(rest_glow, "pane surface carries the theme rest glow");
}

#[test]
fn item_group_collapses_rows_out_of_layout() {
    use heca_grid_ui::ItemGroup;
    let row = || Item::new("row").on_activate(|| {});
    let mut group = ItemGroup::new("GROUP").child(row()).child(row());

    // Expanded: header + 2 rows all take height.
    LayoutEngine::new().compute(&mut group, Size::new(200.0, 400.0));
    let expanded_h = group.base().bounds.size.h;
    let r1 = group.base().children[1].base().bounds.size.h;
    assert!(r1 > 0.0, "expanded rows have height");

    // Collapse via the expanded signal, relayout: rows fold away (display:none).
    group.state().set(false);
    LayoutEngine::new().compute(&mut group, Size::new(200.0, 400.0));
    let collapsed_h = group.base().bounds.size.h;
    let r1c = group.base().children[1].base().bounds.size.h;
    assert!(
        collapsed_h < expanded_h,
        "collapsed group is shorter ({collapsed_h} < {expanded_h})"
    );
    assert_eq!(r1c, 0.0, "collapsed rows take no layout space");
}

#[test]
fn row_activates_on_click_and_key_when_interactive() {
    use heca_grid_ui::Row;
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut row = Row::new()
        .child(Label::new("PANE 1"))
        .on_activate(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    assert!(row.focusable(), "an interactive row is focusable");

    let b = row.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 50.0, b.loc.y);

    click_at(&mut row, outside, PointerButton::Left);
    assert_eq!(clicks.get(), 0, "a click outside the row does nothing");
    // **A raw key reaches the widget that owns the keyboard.** Unfocused, the row does not take
    // Enter — which is what stopped it eating keys meant for the list it sits in.
    let enter = Event::Key {
        key: GridKey::Enter,
        pressed: true,
    };
    heca_grid_ui::dispatch(&mut row, &enter);
    assert_eq!(clicks.get(), 0, "an unfocused row ignores Enter");
    click_at(&mut row, center, PointerButton::Left);
    assert_eq!(clicks.get(), 1, "a click inside the row activates it");
    // …and the click gave it the keyboard, as clicking a control does anywhere else.
    heca_grid_ui::dispatch(&mut row, &enter);
    assert_eq!(clicks.get(), 2, "Enter activates the row that was clicked");
}

/// **A focused row does not take `j` or `k`**, as text or as keys, so they bubble to the dock above
/// it — which is how the sidebar's cursor keys keep working now that the tree is offered a key
/// before the dock's own bindings are.
#[test]
fn a_focused_row_leaves_j_and_k_to_the_container_above() {
    use heca_grid_ui::Row;
    let mut row = Row::new().child(Label::new("pane")).on_activate(|| {});
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));
    row.base().focus(false);

    for c in ['j', 'k'] {
        assert_eq!(
            heca_grid_ui::dispatch(&mut row, &Event::TextInput(c.to_string())),
            Handled::No
        );
        let key = Event::Key {
            key: GridKey::Char(c),
            pressed: true,
        };
        assert_eq!(heca_grid_ui::dispatch(&mut row, &key), Handled::No);
    }
}

#[test]
fn row_without_on_activate_is_not_focusable() {
    use heca_grid_ui::Row;
    let row = Row::new().child(Label::new("static"));
    assert!(!row.focusable(), "a display-only row is not focusable");
}

#[test]
fn rail_cell_lays_out_a_square_with_centered_icon() {
    use heca_grid_ui::{Glyph, Icon, RailCell};

    let mut cell = RailCell::new(Icon::new(Glyph::Terminal).size(20.0)).cell_size(44.0);
    LayoutEngine::new().compute(&mut cell, Size::new(200.0, 200.0));

    let b = cell.base().bounds;
    assert_eq!(b.size.w, 44.0, "cell is its configured width");
    assert_eq!(b.size.h, 44.0, "cell is square");

    // The single icon child sits centered in the square.
    let icon = cell.base().children[0].base().bounds;
    let icon_cx = icon.loc.x + icon.size.w / 2.0;
    let icon_cy = icon.loc.y + icon.size.h / 2.0;
    assert!(
        (icon_cx - (b.loc.x + b.size.w / 2.0)).abs() < 1.0,
        "icon centered horizontally"
    );
    assert!(
        (icon_cy - (b.loc.y + b.size.h / 2.0)).abs() < 1.0,
        "icon centered vertically"
    );
}

#[test]
fn rail_cell_activates_on_click_and_enter() {
    use heca_grid_ui::{Glyph, Icon, RailCell};
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut cell = RailCell::new(Icon::new(Glyph::GitBranch).size(20.0))
        .on_activate(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut cell, Size::new(200.0, 200.0));

    let b = cell.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    // A raw key reaches only the widget that owns the keyboard — focus it, as a real surface
    // would before sending one.
    cell.base().focus(true);
    click_at(&mut cell, center, PointerButton::Left);
    heca_grid_ui::dispatch(
        &mut cell,
        &Event::Key {
            key: heca_grid_ui::GridKey::Enter,
            pressed: true,
        },
    );
    assert_eq!(clicks.get(), 2, "click + Enter both activate the cell");
}
