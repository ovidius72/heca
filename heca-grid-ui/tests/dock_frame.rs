mod common;

use common::{click_at, fixed_box};
use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, Point, Size, Theme};

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

    // A search-like interactive control living in the header-controls slot.
    let control_clicks = Rc::new(Cell::new(0u32));
    let sink = control_clicks.clone();
    let control = Item::new("search").on_activate(move || sink.set(sink.get() + 1));
    let mut dock = DockFrame::new("FILES").header(control);
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
