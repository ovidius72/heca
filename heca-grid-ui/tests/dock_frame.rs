mod common;

#[test]
fn ordinary_parent_children_share_the_dock_body() {
    use heca_grid_ui::{DockFrame, Parent};

    let mut dock = Parent::child(
        DockFrame::new("FILES"),
        [Label::new("first"), Label::new("second")],
    );
    LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
    let body = heca_grid_ui::component::area(&dock, "body").unwrap();
    assert_eq!(body.base().children.len(), 2);
    let scene = common::paint(&dock, &Theme::default());
    assert!(scene.iter().any(|command| matches!(command,
        heca_grid_ui::DrawCommand::Text(text) if text.text == "FILES")));
    for child in &body.base().children {
        assert!(child.base().bounds.loc.y >= body.base().bounds.loc.y);
        assert!(
            child.base().bounds.loc.y + child.base().bounds.size.h
                <= body.base().bounds.loc.y + body.base().bounds.size.h
        );
    }
}

use common::{click_at, fixed_box};
use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, Point, Size, Theme};

#[test]
fn workspace_state_backgrounds_are_off_by_default() {
    use heca_grid_ui::{DockFrame, DrawCommand};
    let theme = Theme::default();
    for active in [true, false] {
        let mut dock = DockFrame::new("Workspace")
            .frameless(true)
            .active(active)
            .previous(!active)
            .child(Label::new("Session"));
        LayoutEngine::new()
            .theme(&theme)
            .compute(&mut dock, Size::new(320.0, 240.0));
        let scene = common::paint(&dock, &theme);
        assert!(
            !scene.iter().any(|cmd| matches!(cmd, DrawCommand::Rect(rect)
            if rect.rect == dock.base().bounds && rect.fill.a > 0)),
            "a workspace-wide tint still paints with the default theme"
        );
    }
}

#[test]
fn workspace_wash_opacity_affects_only_the_background() {
    use heca_grid_ui::{Color, DockFrame, DrawCommand};
    let mut theme = Theme::default();
    theme.colors.active_wash_alpha = 0.25;
    theme.colors.workspace_active_background = Some(Color::new(11, 22, 33, 128));
    let mut dock = DockFrame::new("Workspace")
        .frameless(true)
        .active(true)
        .child(Label::new("Session"));
    LayoutEngine::new()
        .theme(&theme)
        .compute(&mut dock, Size::new(320.0, 240.0));
    let scene = common::paint(&dock, &theme);
    assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Rect(rect)
        if rect.rect == dock.base().bounds && rect.fill == Color::new(11, 22, 33, 32))));
    assert!(
        scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text)
        if text.text == "Session" && text.color == theme.colors.foreground)),
        "background opacity must not dim text"
    );
    dock.nav_state().set(true);
    let scene = common::paint(&dock, &theme);
    assert!(
        scene.iter().any(|cmd| matches!(cmd, DrawCommand::Rect(rect)
        if rect.border.is_some() && rect.glow.is_some())),
        "navigation keeps its outline"
    );
}

#[test]
fn dock_frame_body_has_height_when_expanded() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("FILES").child(fixed_box(120.0, 80.0));

    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));

    let body_h = dock.base().children[1].base().bounds.size.h;
    assert!(body_h > 0.0, "expanded body has height");
}

#[test]
fn dock_frame_collapse_folds_body_out_of_layout() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("FILES").child(fixed_box(120.0, 80.0));
    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));
    let expanded_h = dock.base().bounds.size.h;

    // Collapse via the expanded signal, relayout: body folds away (display:none).
    dock.state().set(false);
    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));

    let body_h = dock.base().children[1].base().bounds.size.h;
    assert_eq!(body_h, 0.0, "collapsed body takes no layout space");
    let collapsed_h = dock.base().bounds.size.h;
    assert!(
        collapsed_h < expanded_h,
        "collapsed dock is shorter ({collapsed_h} < {expanded_h})"
    );
}

#[test]
fn dock_frame_header_click_toggles_and_emits_dock_toggle() {
    use heca_grid_ui::{Action, DockFrame, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut dock = DockFrame::new("FILES").on_toggle(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));
    assert!(dock.state().get_untracked(), "starts expanded");

    // Click the toggle area of the title bar (header child 0): collapses + reports.
    let toggle = dock.base().children[0].base().children[0].base().bounds;
    let center = Point::new(
        toggle.loc.x + toggle.size.w / 2.0,
        toggle.loc.y + toggle.size.h / 2.0,
    );
    click_at(&mut dock, center, PointerButton::Left);

    assert!(
        !dock.state().get_untracked(),
        "header click collapses the frame"
    );
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("dock-toggle", SignalData::Bool(false))),
    );
}

#[test]
fn dock_frame_header_control_receives_events_before_toggle() {
    use heca_grid_ui::DockFrame;
    use std::cell::Cell;
    use std::rc::Rc;

    // A search-like interactive control living in the header-actions slot.
    let control_clicks = Rc::new(Cell::new(0u32));
    let sink = control_clicks.clone();
    let control = Item::new("search").on_activate(move || sink.set(sink.get() + 1));
    let mut dock = DockFrame::new("FILES").header_actions(control);
    LayoutEngine::new().compute(&mut dock, Size::new(260.0, 400.0));

    // Click the control (header child 1): it consumes the event; frame must NOT toggle.
    let ctrl = dock.base().children[0].base().children[1].base().bounds;
    let center = Point::new(
        ctrl.loc.x + ctrl.size.w / 2.0,
        ctrl.loc.y + ctrl.size.h / 2.0,
    );
    click_at(&mut dock, center, PointerButton::Left);

    assert_eq!(control_clicks.get(), 1, "header control received the click");
    assert!(
        dock.state().get_untracked(),
        "clicking the control did not toggle the frame"
    );
}

#[test]
fn dock_frame_collapsed_body_is_skipped_by_focus_traversal() {
    use heca_grid_ui::{DockFrame, FocusManager};

    // Header toggle + one interactive body row are both focusable when expanded.
    let mut dock = DockFrame::new("FILES").child(Item::new("file.rs").on_activate(|| {}));
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut dock, true);
    assert_eq!(
        focus.focused(&mut dock),
        Some(0),
        "header toggle is first in tab order"
    );
    focus.advance(&mut dock, true);
    assert_eq!(
        focus.focused(&mut dock),
        Some(1),
        "body row is tabbable while expanded"
    );

    // Collapse + relayout: the body subtree becomes display:none and drops out of
    // the tab order, so only the header toggle remains (forward Tab wraps to it).
    dock.state().set(false);
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut dock, true);
    assert_eq!(
        focus.focused(&mut dock),
        Some(0),
        "only the header toggle is focusable when collapsed"
    );
    focus.advance(&mut dock, true);
    assert_eq!(
        focus.focused(&mut dock),
        Some(0),
        "collapsed body row is not reachable by Tab"
    );
}

#[test]
fn chrome_region_expanded_uses_full_width() {
    use heca_grid_ui::ChromeRegion;
    let mut region = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0)
        .dock(fixed_box(100.0, 60.0));

    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(
        region.base().bounds.size.w,
        240.0,
        "expanded sidebar uses its full width"
    );
}

#[test]
fn chrome_region_collapses_to_rail_width() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut region = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0)
        .dock(fixed_box(100.0, 60.0));

    region.mode_signal().set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(
        region.base().bounds.size.w,
        48.0,
        "collapsed sidebar shrinks to the rail width"
    );
}

#[test]
fn chrome_region_hidden_folds_out_of_layout() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut region = ChromeRegion::vertical().dock(fixed_box(100.0, 60.0));

    region.mode_signal().set(RegionMode::Hidden);
    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(
        region.base().bounds.size.w,
        0.0,
        "hidden region takes no layout space"
    );
}

#[test]
fn chrome_region_horizontal_bar_collapses_height() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut bar = ChromeRegion::horizontal()
        .expanded_size(200.0)
        .rail_size(40.0)
        .dock(fixed_box(60.0, 100.0));

    bar.mode_signal().set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut bar, Size::new(800.0, 300.0));

    assert_eq!(
        bar.base().bounds.size.h,
        40.0,
        "collapsed top/bottom bar shrinks to the rail height"
    );
}

#[test]
fn chrome_region_toggle_flips_expanded_and_rail() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let region = ChromeRegion::vertical();
    assert_eq!(region.mode_signal().get_untracked(), RegionMode::Expanded);

    region.toggle();
    assert_eq!(
        region.mode_signal().get_untracked(),
        RegionMode::CollapsedRail,
        "toggle collapses to rail"
    );
    region.toggle();
    assert_eq!(
        region.mode_signal().get_untracked(),
        RegionMode::Expanded,
        "toggle expands again"
    );
}

#[test]
fn collapsed_dock_body_is_not_painted() {
    use heca_grid_ui::{DockFrame, DrawCommand};

    // A row whose label must NOT be painted while the dock is collapsed — a
    // display:none subtree is collapsed to the top-left by layout, so painting it
    // would stamp overlapping text there (the showcase artifact this guards).
    let collect_labels = |dock: &mut DockFrame| -> Vec<String> {
        LayoutEngine::new().compute(dock, Size::new(220.0, 400.0));
        let theme = Theme::default();
        let scene = common::paint(dock, &theme);
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let mut open = DockFrame::new("FILES").child(Item::new("SECRET.rs"));
    assert!(
        collect_labels(&mut open).iter().any(|t| t == "SECRET.rs"),
        "expanded dock paints its body row"
    );

    let mut collapsed = DockFrame::new("FILES")
        .expanded(false)
        .child(Item::new("SECRET.rs"));
    assert!(
        collect_labels(&mut collapsed)
            .iter()
            .all(|t| t != "SECRET.rs"),
        "collapsed dock must not paint its hidden body row"
    );
}

#[test]
fn dock_frame_rail_mode_folds_header_and_body_to_icon() {
    use heca_grid_ui::{ChromeRegion, DockFrame, Glyph, Item, RegionMode};

    // A rail-aware dock bound to its region's mode signal (obtained before the
    // region is moved into `.dock(...)`).
    let sidebar = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0);
    let mode = sidebar.mode_signal();
    let dock = DockFrame::new("FILES")
        .rail(mode, Glyph::FolderOpen)
        .child(Item::new("main.rs"));
    let mut sidebar = sidebar.dock(dock);

    // Expanded: header + body are shown; the rail icon is hidden.
    LayoutEngine::new().compute(&mut sidebar, Size::new(400.0, 600.0));
    {
        let dock = sidebar.base().children[0].base();
        assert!(
            !dock.children[0].base().style.layout.hidden,
            "header shown while expanded"
        );
        assert!(
            !dock.children[1].base().style.layout.hidden,
            "body shown while expanded"
        );
        assert!(
            dock.children[2].base().style.layout.hidden,
            "rail icon hidden while expanded"
        );
        assert!(
            dock.children[1].base().bounds.size.h > 0.0,
            "expanded body has height"
        );
    }

    // Collapse the region to its rail: header + body fold away; the rail icon shows.
    mode.set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut sidebar, Size::new(400.0, 600.0));
    {
        let dock = sidebar.base().children[0].base();
        assert!(
            dock.children[0].base().style.layout.hidden,
            "header folds away in rail mode"
        );
        assert!(
            dock.children[1].base().style.layout.hidden,
            "body folds away in rail mode"
        );
        assert!(
            !dock.children[2].base().style.layout.hidden,
            "rail icon shows in rail mode"
        );
        assert_eq!(
            dock.children[1].base().bounds.size.h,
            0.0,
            "folded body takes no layout space"
        );
        assert!(
            dock.children[2].base().bounds.size.h > 0.0,
            "rail icon is laid out"
        );
    }
}

#[test]
fn dock_frame_rail_paints_icon_not_title() {
    use heca_grid_ui::{ChromeRegion, DockFrame, DrawCommand, FontRole, Glyph, Item, RegionMode};

    let sidebar = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0);
    let mode = sidebar.mode_signal();
    let dock = DockFrame::new("FILES")
        .rail(mode, Glyph::FolderOpen)
        .child(Item::new("main.rs"));
    let mut sidebar = sidebar.dock(dock);

    let paint = |sidebar: &mut ChromeRegion| -> (Vec<String>, usize) {
        LayoutEngine::new().compute(sidebar, Size::new(400.0, 600.0));
        let theme = Theme::default();
        let scene = common::paint(sidebar, &theme);
        let mut texts = Vec::new();
        let mut icons = 0usize;
        for c in scene.iter() {
            if let DrawCommand::Text(t) = c {
                if t.font == FontRole::Icon {
                    icons += 1;
                } else {
                    texts.push(t.text.clone());
                }
            }
        }
        (texts, icons)
    };

    // Expanded: the title + body row paint as text; no icon-rail glyph yet.
    let (texts, _) = paint(&mut sidebar);
    assert!(
        texts.iter().any(|t| t == "FILES"),
        "title paints while expanded"
    );
    assert!(
        texts.iter().any(|t| t == "main.rs"),
        "body row paints while expanded"
    );

    // Rail mode: the title + body text are gone; a duotone icon (2 glyph runs) paints.
    mode.set(RegionMode::CollapsedRail);
    let (texts, icons) = paint(&mut sidebar);
    assert!(
        texts.iter().all(|t| t != "FILES"),
        "title is not painted in rail mode"
    );
    assert!(
        texts.iter().all(|t| t != "main.rs"),
        "body row is not painted in rail mode"
    );
    assert!(
        icons >= 2,
        "rail paints the duotone dock icon (secondary + primary), got {icons}"
    );
}

#[test]
fn nested_header_frames_do_not_claim_the_outer_frames_parts() {
    use heca_grid_ui::reactive::signal;
    use heca_grid_ui::{DockFrame, Glyph, RegionMode};

    let mode = signal(RegionMode::Expanded);
    let inner = DockFrame::new("INNER")
        .child(Label::new("inner body"))
        .rail(mode, Glyph::FolderOpen);
    let mut outer = DockFrame::new("OUTER")
        .header_actions(inner)
        .child([Label::new("outer first"), Label::new("outer second")])
        .rail(mode, Glyph::FolderOpen);
    LayoutEngine::new().compute(&mut outer, Size::new(700.0, 600.0));
    let own = |name: &str| {
        outer
            .base()
            .children
            .iter()
            .find(|c| c.base().grid_area.as_deref() == Some(name))
            .unwrap()
    };
    assert_eq!(
        own("body").base().children.len(),
        2,
        "the outer body must receive its children, even with another body in the header"
    );
    assert!(
        own("rail").base().style.layout.hidden,
        "the outer frame must own a rail distinct from the nested header frame"
    );

    outer.state().set(false);
    LayoutEngine::new().compute(&mut outer, Size::new(700.0, 600.0));
    let outer_body = outer
        .base()
        .children
        .iter()
        .find(|c| c.base().grid_area.as_deref() == Some("body"))
        .unwrap();
    assert!(
        outer_body.base().style.layout.hidden,
        "folding hides the outer body"
    );
    // The header-actions slot holds what it was given — here the nested frame.
    let slot = heca_grid_ui::area(&outer, "header_actions").unwrap();
    let inner = slot
        .base()
        .children
        .first()
        .expect("the slot holds the nested frame");
    let inner_body = inner
        .base()
        .children
        .iter()
        .find(|c| c.base().grid_area.as_deref() == Some("body"))
        .unwrap();
    assert!(
        !inner_body.base().style.layout.hidden,
        "the nested header frame keeps its own state"
    );
    assert_eq!(
        inner_body.base().children.len(),
        1,
        "no outer content leaked into the inner body"
    );
}

fn texts(dock: &heca_grid_ui::DockFrame) -> Vec<String> {
    common::paint(dock, &Theme::default())
        .iter()
        .filter_map(|cmd| match cmd {
            heca_grid_ui::DrawCommand::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

fn centre(rect: heca_grid_ui::Rectangle) -> Point {
    Point::new(
        rect.loc.x + rect.size.w / 2.0,
        rect.loc.y + rect.size.h / 2.0,
    )
}

/// A `Section` changes only the title row: the leading part sits right before the title, centred
/// on the row, and the body is laid out exactly as a `Dock`'s — indenting it is the caller's call.
#[test]
fn a_section_puts_its_leading_part_before_the_title_and_leaves_the_body_alone() {
    use heca_grid_ui::{DockFrame, DockFrameVariant, DrawCommand};
    let texts_at = |dock: DockFrame| {
        let mut dock = dock;
        LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
        common::paint(&dock, &Theme::default())
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCommand::Text(text) => Some((text.text.clone(), text.rect)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let find = |all: &[(String, heca_grid_ui::Rectangle)], what: &str| {
        all.iter()
            .find(|(text, _)| text == what)
            .map(|(_, rect)| *rect)
            .unwrap_or_else(|| panic!("{what} painted"))
    };
    let section = texts_at(
        DockFrame::new("Development")
            .variant(DockFrameVariant::Section)
            .leading(Label::new("◆"))
            .child(Item::new("alice")),
    );
    let dock = texts_at(DockFrame::new("FILES").child(Item::new("alice")));
    let (icon, title) = (find(&section, "◆"), find(&section, "Development"));
    let middle = |r: heca_grid_ui::Rectangle| r.loc.y + r.size.h / 2.0;
    assert!(
        icon.loc.x + icon.size.w < title.loc.x,
        "the leading part is before the title"
    );
    assert!(
        title.loc.x - (icon.loc.x + icon.size.w) < 12.0,
        "only a gap between them, no room kept for a chevron: {icon:?} {title:?}"
    );
    assert!(
        (middle(icon) - middle(title)).abs() < 1.0,
        "the leading part is centred on the title row"
    );
    assert_eq!(
        find(&section, "alice").loc.x,
        find(&dock, "alice").loc.x,
        "the body starts where a Dock's does — the frame does not indent it"
    );
    assert!(
        !section.iter().any(|(text, _)| text == "⠿"),
        "a section has no grip"
    );
}

#[test]
fn a_header_of_the_callers_own_still_folds_the_frame() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("unused")
        .header(Flex::row().child([Label::new("Team"), Label::new("3")]))
        .child(fixed_box(100.0, 40.0));
    LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
    let painted = texts(&dock);
    assert!(painted.iter().any(|t| t == "Team") && painted.iter().any(|t| t == "3"));
    assert!(
        !painted.iter().any(|t| t == "unused"),
        "the caller's header replaces the title"
    );
    assert!(
        painted.iter().any(|t| t == "▾"),
        "the frame keeps its chevron"
    );
    let toggle = heca_grid_ui::component::area(&dock, "toggle")
        .unwrap()
        .base()
        .bounds;
    click_at(&mut dock, centre(toggle), PointerButton::Left);
    assert!(
        !dock.state().get_untracked(),
        "a click on the caller's header folds the body"
    );
}

#[test]
fn a_header_survives_a_later_variant_and_is_wrapped_once() {
    use heca_grid_ui::{DockFrame, DockFrameVariant};
    let build = |dock: DockFrame| {
        let mut dock = dock;
        LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
        let custom = heca_grid_ui::component::area(&dock, "dock.header").unwrap();
        (
            texts(&dock).iter().any(|t| t == "Team"),
            custom.base().children.len(),
        )
    };
    let once = build(DockFrame::new("x").header(Label::new("Team")));
    let rebuilt = build(
        DockFrame::new("x")
            .header(Label::new("Team"))
            .variant(DockFrameVariant::Section)
            .grip(false),
    );
    assert_eq!(once, (true, 1));
    assert_eq!(
        rebuilt, once,
        "rebuilding the title row keeps the caller's header as it was"
    );
}

#[test]
fn a_frame_that_does_not_collapse_ignores_the_click_and_shows_no_chevron() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("FILES")
        .expanded(false)
        .collapsible(false)
        .child(fixed_box(100.0, 40.0));
    LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
    assert!(
        dock.state().get_untracked(),
        "a frame that cannot fold is open"
    );
    assert!(
        !texts(&dock).iter().any(|t| t == "▾" || t == "▸"),
        "no chevron"
    );
    let toggle = heca_grid_ui::component::area(&dock, "toggle")
        .unwrap()
        .base()
        .bounds;
    click_at(&mut dock, centre(toggle), PointerButton::Left);
    assert!(dock.state().get_untracked(), "the click folds nothing");
}

#[test]
fn a_dock_without_grip_paints_none() {
    use heca_grid_ui::DockFrame;
    let mut with = DockFrame::new("FILES");
    let mut without = DockFrame::new("FILES").grip(false);
    LayoutEngine::new().compute(&mut with, Size::new(300.0, 400.0));
    LayoutEngine::new().compute(&mut without, Size::new(300.0, 400.0));
    assert!(texts(&with).iter().any(|t| t == "⠿"));
    assert!(!texts(&without).iter().any(|t| t == "⠿"));
}

/// **Every frame's first part starts at one left edge**: a Dock's grip, a Section's leading part
/// and a title row of the caller's own all begin where a row's content begins.
#[test]
fn every_kind_of_title_row_starts_at_the_same_left_edge() {
    use heca_grid_ui::{DockFrame, DockFrameVariant, DrawCommand};
    let first_x = |dock: DockFrame, what: &str| {
        let mut dock = dock;
        LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
        common::paint(&dock, &Theme::default())
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Text(text) if text.text == what => Some(text.rect.loc.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{what} painted"))
    };
    let grip = first_x(DockFrame::new("FILES"), "⠿");
    let leading = first_x(
        DockFrame::new("Team")
            .variant(DockFrameVariant::Section)
            .leading(Label::new("◆")),
        "◆",
    );
    let custom = first_x(DockFrame::new("x").header(Label::new("Team")), "Team");
    let row = first_x(
        DockFrame::new("x")
            .header(Label::new("Team"))
            .child(Item::new("row")),
        "row",
    );
    assert!(
        (leading - grip).abs() < 1.0,
        "a Section's leading part starts at the grip: {leading} vs {grip}"
    );
    assert!(
        (custom - grip).abs() < 1.0,
        "a caller's title row starts at the grip: {custom} vs {grip}"
    );
    assert!(
        (row - grip).abs() < 1.0,
        "a body row's content starts there too: {row} vs {grip}"
    );
}

/// **A title row of the caller's own sits where a Dock's title row sits**: as tall, so the first
/// body row starts at the same height in both.
#[test]
fn a_callers_title_row_is_as_tall_as_a_docks() {
    use heca_grid_ui::{DockFrame, DrawCommand};
    let first_row_y = |dock: DockFrame| {
        let mut dock = dock;
        LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
        common::paint(&dock, &Theme::default())
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Text(text) if text.text == "row" => Some(text.rect.loc.y),
                _ => None,
            })
            .expect("the row is painted")
    };
    let dock = first_row_y(DockFrame::new("FILES").child(Item::new("row")));
    let custom = first_row_y(
        DockFrame::new("x")
            .header(Label::new("Team"))
            .child(Item::new("row")),
    );
    assert!(
        (dock - custom).abs() < 1.0,
        "the first row starts at the same height: {dock} vs {custom}"
    );
}

/// **A count in the title row ends where the rows' badges end** — the caller adds no spacer.
#[test]
fn header_actions_end_where_a_rows_trailing_part_ends() {
    use heca_grid_ui::{DockFrame, DrawCommand};
    let mut dock = DockFrame::new("FILES")
        .header_actions(Label::new("3"))
        .child(Item::new("row").trailing(Label::new("M")));
    LayoutEngine::new().compute(&mut dock, Size::new(300.0, 400.0));
    let right = |what: &str| {
        common::paint(&dock, &Theme::default())
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Text(text) if text.text == what => {
                    Some(text.rect.loc.x + text.rect.size.w)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{what} painted"))
    };
    assert!(
        (right("3") - right("M")).abs() < 1.0,
        "the title count and the row badge end together: {} vs {}",
        right("3"),
        right("M")
    );
}

/// A dock frame that is a row of a reorderable list shows its own reorder handle, not the
/// dock-move grip.
#[test]
fn the_grip_shows_the_glyph_it_is_given() {
    use heca_grid_ui::prelude::*;
    fn labels(node: &dyn heca_grid_ui::Component, out: &mut Vec<String>) {
        if let Some(text) = node.text_summary() {
            out.push(text);
        }
        for child in &node.base().children {
            labels(child.as_ref(), out);
        }
    }
    let (mut plain, mut handle) = (Vec::new(), Vec::new());
    labels(&DockFrame::new("Lobby"), &mut plain);
    labels(&DockFrame::new("Lobby").grip_glyph("≡"), &mut handle);
    assert!(
        plain.iter().any(|t| t == "⠿"),
        "the default grip: {plain:?}"
    );
    assert!(
        handle.iter().any(|t| t == "≡") && !handle.iter().any(|t| t == "⠿"),
        "its own glyph: {handle:?}"
    );
}
