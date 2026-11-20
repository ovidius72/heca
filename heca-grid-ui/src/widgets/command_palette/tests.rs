//! The palette as a composition: rows laid out by the engine inside the panel the overlay places.

use super::*;
use crate::widgets::overlay::VIEWPORT_MARGIN;
use crate::{DrawCommand, LayoutEngine, PaintCx, Scene, Theme};
use heca_core::layout::{Rectangle, Size};

/// A laid-out, painted palette.
pub(super) fn shown(mut palette: CommandPalette, viewport: Size) -> (CommandPalette, Scene) {
    LayoutEngine::new().compute(&mut palette, viewport);
    let scene = painted(&palette, viewport);
    (palette, scene)
}

/// Lay out again, as a host does after something changed, and paint.
fn relayout(palette: &mut CommandPalette, viewport: Size) -> Scene {
    LayoutEngine::new().compute(palette, viewport);
    painted(palette, viewport)
}

fn painted(palette: &CommandPalette, viewport: Size) -> Scene {
    let mut scene = Scene::new();
    palette.paint(&mut PaintCx::new(&mut scene, &Theme::default()).with_viewport(viewport));
    scene
}

/// The laid-out box of command `index`'s row.
pub(super) fn row_bounds(palette: &mut CommandPalette, index: usize) -> Rectangle {
    palette.list_mut().children[index].base().bounds
}

/// Every text run the scene shows, with the part of its box a clip leaves visible — a row scrolled
/// past the fold is drawn by the scene and cut by the clip, so it is not on screen.
fn visible_text(scene: &Scene) -> Vec<(String, Rectangle)> {
    let mut out = Vec::new();
    for segment in scene.overlay_segments() {
        let mut clips: Vec<Option<Rectangle>> = Vec::new();
        for command in segment.iter() {
            match command {
                DrawCommand::PushClip(r) => clips.push(match clips.last() {
                    Some(Some(open)) => open.intersection(*r),
                    Some(None) => None,
                    None => Some(*r),
                }),
                DrawCommand::PopClip => {
                    clips.pop();
                }
                DrawCommand::Text(t) if !t.text.is_empty() => {
                    let shown = match clips.last() {
                        Some(Some(clip)) => clip.intersection(t.rect),
                        Some(None) => None,
                        None => Some(t.rect),
                    };
                    if let Some(rect) = shown.filter(|r| r.size.w > 0.0 && r.size.h > 0.0) {
                        out.push((t.text.clone(), rect));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

fn described(n: usize) -> CommandPalette {
    let mut palette = CommandPalette::new();
    for i in 0..n {
        palette = palette.command(
            Command::new(format!("Command {i}"), || {})
                .description("What this command does, at some length.")
                .keys([KeyCap::Text("λ".into()), KeyCap::Text("a".into())])
                .keys([KeyCap::Text("λ".into()), KeyCap::Text("b".into())]),
        );
    }
    palette.default_open(true)
}

fn inside(inner: Rectangle, outer: Rectangle) -> bool {
    inner.loc.x >= outer.loc.x - 0.5
        && inner.loc.y >= outer.loc.y - 0.5
        && inner.loc.x + inner.size.w <= outer.loc.x + outer.size.w + 0.5
        && inner.loc.y + inner.size.h <= outer.loc.y + outer.size.h + 0.5
}

#[test]
fn rows_are_laid_out_by_the_engine_in_order_inside_the_panel() {
    let (mut palette, scene) = shown(described(3), Size::new(900.0, 700.0));
    let panel = palette.panel_bounds();
    assert!(panel.size.w > 0.0 && panel.size.h > 0.0, "{panel:?}");
    let rows: Vec<_> = (0..3).map(|i| row_bounds(&mut palette, i)).collect();
    for row in &rows {
        assert!(
            row.size.w > 0.0 && row.size.h > 0.0,
            "a row has a box: {row:?}"
        );
        assert!(inside(*row, panel), "{row:?} in {panel:?}");
    }
    assert!(
        rows[0].loc.y < rows[1].loc.y && rows[1].loc.y < rows[2].loc.y,
        "{rows:?}"
    );
    // Every title is drawn, and drawn on its own row — not over the input, not outside the panel.
    let text = visible_text(&scene);
    for (i, row) in rows.iter().enumerate() {
        let (_, at) = text
            .iter()
            .find(|(t, _)| *t == format!("Command {i}"))
            .expect("title drawn");
        assert!(inside(*at, *row), "{at:?} in {row:?}");
    }
}

/// **Nothing paints outside the panel, and no content renders as nothing** — at several window
/// sizes, with a list longer than the window can show. The palette-sized form of what the
/// per-widget sweeps assert of every other surface.
#[test]
fn the_palette_paints_inside_its_panel_and_never_as_nothing_at_every_window_size() {
    for (w, h) in [
        (1400.0, 900.0),
        (900.0, 600.0),
        (600.0, 300.0),
        (320.0, 260.0),
    ] {
        let viewport = Size::new(w, h);
        let (mut palette, scene) = shown(described(40), viewport);
        let panel = palette.panel_bounds();
        let window = Rectangle::new(heca_core::layout::Point::default(), viewport);
        assert!(
            inside(panel, window),
            "{w}x{h}: the panel {panel:?} leaves the window"
        );
        let text = visible_text(&scene);
        for (words, at) in &text {
            assert!(
                inside(*at, panel),
                "{w}x{h}: {words:?} at {at:?} is outside the panel {panel:?}"
            );
        }
        // In a window too short for even one reflowed row the region shows the selected row's
        // *bottom* (its minimal reveal of a target taller than itself), so only a window with room
        // for a whole row must show the first title.
        if h >= 300.0 {
            assert!(
                text.iter().any(|(t, _)| t == "Command 0"),
                "{w}x{h}: the first row shows"
            );
        }
        assert!(
            text.len() > 2,
            "{w}x{h}: the rows draw their text, got {text:?}"
        );
        let _ = row_bounds(&mut palette, 0);
    }
}

/// **The panel keeps inside the window with room to spare**, at every window height — swept, because
/// a panel resting exactly on an edge only appears where the room divides evenly.
#[test]
fn the_panel_never_runs_off_a_short_window() {
    for size in [WidgetSize::Small, WidgetSize::Normal, WidgetSize::Large] {
        for w in [1280.0, 480.0] {
            for h in (120..=1000).step_by(7) {
                let viewport = Size::new(w, f64::from(h));
                let (palette, _) = shown(described(40).panel_size(size), viewport);
                let panel = palette.panel_bounds();
                let margin = f64::from(VIEWPORT_MARGIN);
                assert!(
                    panel.loc.x >= margin - 0.5
                        && panel.loc.y >= margin - 0.5
                        && panel.loc.x + panel.size.w <= w - margin + 0.5
                        && panel.loc.y + panel.size.h <= viewport.h - margin + 0.5,
                    "{size:?} at {w}x{h}: panel {panel:?} is closer than {margin}px to an edge",
                );
            }
        }
    }
}

/// The size setting is the panel's own size: its width in pixels (560 / 700 / 1000) and how many
/// rows the list shows before it scrolls (6 / 8 / 12 rows of a described list). Never wider than
/// the window leaves room for.
#[test]
fn the_size_setting_is_the_panels_own_width_and_row_count() {
    let panel_of = |size: WidgetSize, viewport: Size| {
        let mut palette = described(40).panel_size(size);
        LayoutEngine::new().compute(&mut palette, viewport);
        let list = palette.part_mut("palette-list").base().bounds;
        (palette.panel_bounds(), list)
    };
    let roomy = Size::new(2400.0, 1400.0);
    let [
        (small, small_list),
        (normal, normal_list),
        (large, large_list),
    ] = [WidgetSize::Small, WidgetSize::Normal, WidgetSize::Large]
        .map(|size| panel_of(size, roomy));
    for (panel, px) in [(small, 560.0), (normal, 700.0), (large, 1000.0)] {
        assert!(
            (panel.size.w - px).abs() < 0.5,
            "{px}px wide, got {}",
            panel.size.w
        );
    }
    // A row of this list is two lines and its padding; the list stops at the variant's row count.
    assert!(small_list.size.h < normal_list.size.h && normal_list.size.h < large_list.size.h);
    let per_row = |list: Rectangle, rows: f64| list.size.h / rows;
    let (a, b, c) = (
        per_row(small_list, 6.0),
        per_row(normal_list, 8.0),
        per_row(large_list, 12.0),
    );
    assert!(
        (a - b).abs() < 0.5 && (b - c).abs() < 0.5,
        "the same row height: {a} {b} {c}"
    );
    // A window narrower than the panel wins: 92% of what the overlay leaves.
    let narrow = Size::new(480.0, 700.0);
    let (capped, _) = panel_of(WidgetSize::Large, narrow);
    assert!(
        capped.size.w
            <= f64::from(extent::MAX_WIDTH_SHARE) * (narrow.w - 2.0 * f64::from(VIEWPORT_MARGIN))
                + 1.0,
        "{capped:?}"
    );
    // The caption and header variants read as the nearest of the three.
    assert_eq!(
        extent::extent(WidgetSize::Caption),
        extent::extent(WidgetSize::Small)
    );
    assert_eq!(
        extent::extent(WidgetSize::Header),
        extent::extent(WidgetSize::Large)
    );
}

/// The panel hangs 12% down the window and keeps 8% of it clear underneath, at any window height.
#[test]
fn the_panel_hangs_a_share_down_the_window_and_keeps_a_share_clear_below() {
    for h in [500.0, 800.0, 1200.0] {
        let viewport = Size::new(1280.0, h);
        let (palette, _) = shown(described(60).panel_size(WidgetSize::Large), viewport);
        let panel = palette.panel_bounds();
        assert!(
            (panel.loc.y - 0.12 * h).abs() < 1.0,
            "{h}: starts at {}",
            panel.loc.y
        );
        assert!(
            h - (panel.loc.y + panel.size.h) >= 0.08 * h - 1.0,
            "{h}: ends at {}",
            panel.loc.y + panel.size.h
        );
    }
}

/// **The selected row grows and pushes the next one down.** A long description reflows while its
/// row is selected and is cut while it is not, and the engine moves everything after it — nothing
/// here measures a row.
#[test]
fn the_selected_row_grows_and_the_next_one_slides_down() {
    let long = "Give chrome keyboard focus to a dock — press a letter to pick one, or name it, \
                and keep reading until the line has to fold more than once.";
    let viewport = Size::new(700.0, 800.0);
    let mut palette = CommandPalette::new()
        .command(Command::new("First", || {}).description(long))
        .command(Command::new("Second", || {}).description(long))
        .command(Command::new("Third", || {}).description(long))
        .default_open(true);
    LayoutEngine::new().compute(&mut palette, viewport);
    let (first_before, second_before) = (row_bounds(&mut palette, 0), row_bounds(&mut palette, 1));
    assert!(
        first_before.size.h > second_before.size.h + 10.0,
        "the selected row is taller: {first_before:?} {second_before:?}"
    );
    assert!(
        (second_before.loc.y - (first_before.loc.y + first_before.size.h)).abs() < 1.0,
        "rows are contiguous"
    );

    crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::MenuDown));
    relayout(&mut palette, viewport);
    let (first, second, third) = (
        row_bounds(&mut palette, 0),
        row_bounds(&mut palette, 1),
        row_bounds(&mut palette, 2),
    );
    assert!(
        first.size.h < first_before.size.h,
        "the row it left is cut back to one line"
    );
    assert!(second.size.h > first.size.h, "the new selection reflows");
    assert!(
        third.loc.y > second.loc.y + second.size.h - 1.0,
        "the row below slides under the reflowed one"
    );
}

/// **Moving past the last visible row scrolls it into view** — the region does it, because the
/// selected row says it is the one to follow.
#[test]
fn the_selection_stays_in_view_when_it_moves_past_the_last_visible_row() {
    let viewport = Size::new(900.0, 400.0);
    let mut palette = described(40);
    LayoutEngine::new().compute(&mut palette, viewport);
    let list_box = |p: &mut CommandPalette| p.part_mut("palette-list").base().bounds;
    let visible_rows = {
        let list = list_box(&mut palette);
        (0..40)
            .filter(|&i| inside(row_bounds(&mut palette, i), list))
            .count()
    };
    assert!(
        visible_rows < 20,
        "the window shows a few rows, not all forty ({visible_rows})"
    );
    for step in 1..=30 {
        crate::dispatch(&mut palette, &Event::Widget(WidgetIntent::MenuDown));
        // A reveal follows the layout it reacts to, so a frame or two settle it.
        for _ in 0..3 {
            relayout(&mut palette, viewport);
        }
        let list = list_box(&mut palette);
        let row = row_bounds(&mut palette, step);
        assert!(
            inside(row, list),
            "after {step} steps down the selected row {row:?} is outside the list {list:?}"
        );
    }
}

/// Pointing at a row selects it and **never scrolls it from under the pointer**.
#[test]
fn hovering_a_partly_visible_row_selects_it_without_moving_the_list() {
    let viewport = Size::new(900.0, 400.0);
    let (mut palette, _) = shown(described(40), viewport);
    let list = palette.part_mut("palette-list").base().bounds;
    let edge = (0..40)
        .map(|i| (i, row_bounds(&mut palette, i)))
        .find(|(_, row)| {
            row.loc.y < list.loc.y + list.size.h
                && row.loc.y + row.size.h > list.loc.y + list.size.h
        })
        .expect("a row straddles the bottom of the list");
    let at = heca_core::layout::Point::new(edge.1.loc.x + 20.0, edge.1.loc.y + 4.0);
    crate::dispatch(&mut palette, &Event::pointer_moved(at));
    palette.tick(0.0);
    assert_eq!(
        palette.selected, edge.0,
        "the row under the pointer is selected"
    );
    for _ in 0..3 {
        relayout(&mut palette, viewport);
    }
    let after = row_bounds(&mut palette, edge.0);
    assert!(
        (after.loc.y - edge.1.loc.y).abs() < 1.0,
        "the row stayed under the pointer: {after:?} vs {:?}",
        edge.1
    );
}

/// The query decides which rows are in the layout and in what order: a filtered-out row has no box,
/// and the rows that are left follow the ranking, not the order they were added in.
#[test]
fn the_query_removes_rows_from_the_layout_and_orders_the_rest() {
    let viewport = Size::new(900.0, 700.0);
    let mut palette = CommandPalette::new()
        .command(Command::new("alpha beta", || {}))
        .command(Command::new("zulu", || {}))
        .command(Command::new("beta", || {}))
        .query("beta")
        .default_open(true);
    LayoutEngine::new().compute(&mut palette, viewport);
    assert_eq!(
        row_bounds(&mut palette, 1).size,
        Size::new(0.0, 0.0),
        "`zulu` is out of the layout"
    );
    let (alpha_beta, beta) = (row_bounds(&mut palette, 0), row_bounds(&mut palette, 2));
    assert!(beta.size.h > 0.0 && alpha_beta.size.h > 0.0);
    assert!(
        beta.loc.y < alpha_beta.loc.y,
        "the better match leads even though it was added last"
    );
}

/// The ink of a row is the theme's, by meaning: the selected title in full foreground, the others
/// soft, the row you are on in the accent, the icon and chips accented on the selected row.
#[test]
fn a_rows_ink_follows_its_state_through_theme_tones() {
    use crate::{Tone, widgets::Glyph};
    let theme = Theme::default();
    let (_p, scene) = shown(
        CommandPalette::new()
            .command(Command::new("Selected", || {}).keys([KeyCap::Text("k".into())]))
            .command(Command::new("Plain", || {}).keys([KeyCap::Text("p".into())]))
            .command(Command::new("Here", || {}).current(true).icon(Glyph::Gear))
            .default_open(true),
        Size::new(900.0, 600.0),
    );
    let ink = |text: &str| {
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) if t.text == text => Some(t.color),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text} is painted"))
    };
    assert_eq!(ink("Selected"), theme.colors.foreground);
    assert_eq!(ink("Plain"), Tone::Soft.resolve(&theme));
    assert_eq!(ink("Here"), Tone::Accent.resolve(&theme));
    assert_eq!(
        ink("k"),
        Tone::Accent.resolve(&theme),
        "the selected row's chips take the accent"
    );
    assert_eq!(
        ink("p"),
        Tone::Muted.resolve(&theme),
        "the others' are muted"
    );
}

/// **The query line fills the panel**, at every panel width — by layout, not by a width of its own.
#[test]
fn the_query_field_fills_the_panels_width() {
    for size in [WidgetSize::Small, WidgetSize::Normal, WidgetSize::Large] {
        let (mut palette, _) = shown(described(2).panel_size(size).placeholder("Type a command…"), Size::new(1600.0, 900.0));
        let panel = palette.panel_bounds();
        let query = palette.part_mut("palette-query").base().bounds;
        let list = palette.part_mut("palette-list").base().bounds;
        assert!(
            (query.size.w - list.size.w).abs() < 1.0,
            "{size:?}: query {query:?} vs list {list:?}"
        );
        assert!(
            query.size.w > 0.9 * (panel.size.w - 40.0),
            "{size:?}: {query:?} in {panel:?}"
        );
    }
}

/// **A command's icon is drawn at 1.3 times the row's font**, so it reads as the row's subject.
#[test]
fn a_commands_icon_is_a_touch_larger_than_the_text() {
    use crate::widgets::Glyph;
    let (mut palette, _) = shown(
        CommandPalette::new()
            .command(Command::new("Gear", || {}).icon(Glyph::Gear))
            .default_open(true),
        Size::new(900.0, 600.0),
    );
    let font = palette.panel_mut().font;
    let row = &palette.list_mut().children[0];
    let cell = &row.base().children[0].base().children[0].base().children[0];
    assert!(
        (cell.base().bounds.size.w - f64::from(font * 1.3)).abs() < 0.6,
        "{:?} at font {font}",
        cell.base().bounds
    );
}

/// **Page, first and last move the selection** — through the real keymap, with the query field
/// holding the keyboard (Home and End are the list's here, not the caret's).
#[test]
fn page_home_and_end_move_the_selection_through_the_keymap() {
    use crate::{GridKey, KeyPress, Keymap, Modifiers};
    let viewport = Size::new(900.0, 500.0);
    let (mut palette, _) = shown(described(40), viewport);
    let keys = Keymap::with_defaults();
    let press = |palette: &mut CommandPalette, key| {
        let handled = keys.deliver_press(
            &KeyPress {
                key,
                text: None,
                mods: Modifiers::default(),
            },
            |ev| crate::dispatch(palette, ev),
        );
        for _ in 0..3 {
            relayout(palette, viewport);
        }
        handled
    };
    assert_eq!(press(&mut palette, GridKey::End), Handled::Yes);
    assert_eq!(palette.selected, 39, "End: the last row");
    assert_eq!(press(&mut palette, GridKey::Home), Handled::Yes);
    assert_eq!(palette.selected, 0, "Home: the first row");
    press(&mut palette, GridKey::PageDown);
    let page = palette.selected;
    assert!(
        page > 1 && page < 20,
        "a page is the rows the list shows whole, not one and not all: {page}"
    );
    press(&mut palette, GridKey::PageDown);
    assert!(palette.selected > page, "and again");
    press(&mut palette, GridKey::PageUp);
    assert_eq!(palette.selected, page, "PageUp is PageDown backwards");
    press(&mut palette, GridKey::PageUp);
    press(&mut palette, GridKey::PageUp);
    assert_eq!(palette.selected, 0, "and stops at the top");
}

/// The query line fills the panel **after the placeholder replaced the field** — the app always sets
/// one, and the replacement must be seated exactly like the first.
#[test]
fn the_query_field_still_fills_the_panel_once_a_placeholder_replaced_it() {
    let (mut palette, _) = shown(
        described(2).placeholder("Type a command…"),
        Size::new(1400.0, 900.0),
    );
    let query = palette.part_mut("palette-query").base().bounds;
    let list = palette.part_mut("palette-list").base().bounds;
    assert!(
        (query.size.w - list.size.w).abs() < 1.0,
        "query {query:?} vs list {list:?}"
    );
}
