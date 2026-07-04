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

#[test]
fn test_chrome_content_rect_left_sidebar() {
    let cfg = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: 200.0,
        right_sidebar_width: 0.0,
        sidebar_gap: 0.0,
    };
    let r = cfg.content_rect(1280.0, 800.0);
    assert_eq!(r.loc.x, 200.0);
    assert_eq!(r.loc.y, 32.0);
    assert_eq!(r.size.w, 1080.0);
    assert_eq!(r.size.h, 744.0);
}

#[test]
fn test_chrome_content_rect_no_sidebars() {
    let cfg = ChromeConfig {
        tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
        status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
        left_sidebar_width: 40.0,
        right_sidebar_width: 0.0,
        sidebar_gap: 0.0,
    };
    let r = cfg.content_rect(1280.0, 800.0);
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
