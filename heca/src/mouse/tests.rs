use super::*;
use crate::chrome::{DEFAULT_STATUS_BAR_HEIGHT, DEFAULT_TAB_BAR_HEIGHT};
use heca_core::layout::types::{LayoutOptions, Point, Rectangle, Size};
use heca_core::layout::workspace::Workspace;
use heca_core::layout::{Column, ColumnId, ColumnWidth, Pane, PaneId};

#[test]
fn test_rubberband_zero() {
    let result = rubberband(1.0);
    let expected = (1.0 - (1.0 / (1.0 * 1.0 / 0.5 + 1.0))) * 0.5;
    assert!((result - expected).abs() < 1e-6);
}

#[test]
fn test_rubberband_small() {
    let r = rubberband(0.25);
    assert!(r > 0.0 && r < 0.5);
}

#[test]
fn test_rubberband_large() {
    let r = rubberband(100.0);
    assert!((r - 0.5).abs() < 0.01);
}

#[test]
fn test_rubberband_negative() {
    let r = rubberband(-1.0);
    assert!(r.is_finite(), "rubberband(-1.0) should be finite");
}

#[test]
fn test_find_pane_found() {
    let mut ws = Workspace::new(
        heca_core::layout::WorkspaceId(1),
        Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
        2.0,
        LayoutOptions::default(),
    );
    ws.scrolling.add_column(
        None,
        Column::new(
            ColumnId(42),
            Pane::new(PaneId(100), "test-pane"),
            ColumnWidth::Proportion(1.0),
        ),
        false,
    );
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(100)), Some((0, 0)));
}

#[test]
fn test_find_pane_not_found() {
    let mut ws = Workspace::new(
        heca_core::layout::WorkspaceId(1),
        Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
        2.0,
        LayoutOptions::default(),
    );
    ws.scrolling.add_column(
        None,
        Column::new(
            ColumnId(1),
            Pane::new(PaneId(10), "a"),
            ColumnWidth::Proportion(1.0),
        ),
        false,
    );
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(999)), None);
}

#[test]
fn test_find_pane_empty() {
    let mut ws = Workspace::new(
        heca_core::layout::WorkspaceId(1),
        Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
        2.0,
        LayoutOptions::default(),
    );
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(1)), None);
}

#[test]
fn test_find_pane_multi_column() {
    let mut ws = Workspace::new(
        heca_core::layout::WorkspaceId(1),
        Rectangle::new(Point::default(), Size::new(1280.0, 800.0)),
        2.0,
        LayoutOptions::default(),
    );
    ws.scrolling.add_column(
        None,
        Column::new(
            ColumnId(10),
            Pane::new(PaneId(1), "a"),
            ColumnWidth::Proportion(0.5),
        ),
        false,
    );
    ws.scrolling.add_column(
        None,
        Column::new(
            ColumnId(20),
            Pane::new(PaneId(2), "b"),
            ColumnWidth::Proportion(0.5),
        ),
        false,
    );
    ws.scrolling
        .add_pane_to_column(0, Some(1), Pane::new(PaneId(3), "c"), false);

    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(1)), Some((0, 0)));
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(3)), Some((0, 1)));
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(2)), Some((1, 0)));
    assert_eq!(find_pane_in_workspace(&mut ws, PaneId(999)), None);
}

/// **With a sidebar hidden, the panes own that edge right up to the window** — there is no strip
/// along it that belongs to nobody.
///
/// The hit test used to answer this itself, and got it wrong twice: it fell back to a hardcoded
/// `40.0` on the left, the width of a rail that no longer exists, so a hidden left sidebar left a
/// 40px band where clicking a pane did nothing; and it guarded the right edge with an **x**
/// compared against half the window's **height**. It asks `content_rect` now, so both edges are
/// whatever the division says they are — which is what this pins.
#[test]
fn a_hidden_sidebar_leaves_no_strip_the_panes_do_not_own() {
    let r = ChromeConfig::for_window(
        Size::new(1280.0, 800.0),
        DEFAULT_TAB_BAR_HEIGHT,
        DEFAULT_STATUS_BAR_HEIGHT,
        0.0,
        0.0,
        0.0,
    )
    .content_rect();

    assert!(r.contains(Point::new(1.0, 100.0)), "the left edge is the window's");
    assert!(r.contains(Point::new(1279.0, 100.0)), "and so is the right edge");
    assert!(
        r.contains(Point::new(700.0, 100.0)),
        "a point past half the window's height is still in the panes"
    );
}

#[test]
fn test_chrome_content_rect_left_sidebar() {
    let r = ChromeConfig::for_window(
        heca_core::layout::types::Size::new(1280.0, 800.0),
        DEFAULT_TAB_BAR_HEIGHT,
        DEFAULT_STATUS_BAR_HEIGHT,
        200.0,
        0.0,
        0.0,
    )
    .content_rect();
    assert_eq!(r.loc.x, 200.0);
    assert_eq!(r.loc.y, 32.0);
    assert_eq!(r.size.w, 1080.0);
    assert_eq!(r.size.h, 744.0);
}

#[test]
fn test_chrome_content_rect_no_sidebars() {
    let r = ChromeConfig::for_window(
        heca_core::layout::types::Size::new(1280.0, 800.0),
        DEFAULT_TAB_BAR_HEIGHT,
        DEFAULT_STATUS_BAR_HEIGHT,
        40.0,
        0.0,
        0.0,
    )
    .content_rect();
    assert_eq!(r.loc.x, 40.0);
    assert_eq!(r.loc.y, 32.0);
    assert_eq!(r.size.w, 1240.0);
    assert_eq!(r.size.h, 744.0);
}

#[test]
fn test_rubberband_niri_ref() {
    assert!((rubberband(0.0) - 0.0).abs() < 1e-6);
    assert!((rubberband(0.5) - 0.25).abs() < 1e-6);
    assert!((rubberband(1.0) - 1.0 / 3.0).abs() < 1e-6);
    assert!((rubberband(2.0) - 0.4).abs() < 1e-6);
}
