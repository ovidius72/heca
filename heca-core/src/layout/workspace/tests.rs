use super::*;
use crate::layout::column::Pane;
use crate::layout::testing::{Shown, Windowed};

/// **A column id is allocated, never derived — so it is never reused.**
///
/// It used to be computed from the column *count* (`ws.id * 1000 + columns.len()`), so closing
/// a column and opening another handed the new one an id that was still in use: three columns
/// gave `[0, 1, 2]`, and after that round trip `[0, 2, 2]`. Nothing looked a column up by id at
/// the time, so nothing failed — but identity is what the keyboard cursor, a right-click, a drag
/// and a remembered hint letter are all kept on, and two columns answering to one id are two
/// rows none of them can tell apart.
#[test]
fn a_closed_column_never_hands_its_id_to_the_next_one() {
    let mut window = Windowed::new(Size::new(1280.0, 800.0), 1.0);
    for i in 1..=3u64 {
        window
            .m()
            .add_pane(Pane::new(PaneId(i), format!("p{i}")), None, true);
    }
    window.ws().scroll_mut().remove_column(1);
    window
        .m()
        .add_pane(Pane::new(PaneId(9), "p9".to_string()), None, true);

    let ids: Vec<u64> = window
        .l()
        .active_workspace()
        .expect("a workspace")
        .scrolling
        .columns
        .iter()
        .map(|c| c.id.0)
        .collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "a column id was reused: {ids:?}");
}

/// Helper: create a workspace with a single column and pane.
fn workspace_with_pane(pane_id: u64) -> Shown {
    let mut ws = Shown::new(Size::new(800.0, 600.0));
    let pane = Pane::new(PaneId(pane_id), format!("pane{}", pane_id));
    ws.m().add_pane(pane, None, true, ColumnId(pane_id));
    ws
}

/// Helper: create a workspace with one tiled pane and one floating pane.
fn workspace_with_floating_pane(pane_id: u64) -> Shown {
    let mut ws = workspace_with_pane(99); // one tiled pane
    ws.floating_panes.push(FloatingPane {
        pane: Pane::new(PaneId(pane_id), format!("float{}", pane_id)),
        position: Point::new(50.0, 50.0),
        size: Size::new(400.0, 300.0),
        is_active: true,
        original_column_idx: None,
        original_pane_idx: None,
    });
    ws.focus_domain = FocusDomain::Floating;
    ws
}

// ── has_panes ──

#[test]
fn has_panes_true_when_tiled_panes_exist() {
    let ws = workspace_with_pane(1);
    assert!(
        ws.has_panes(),
        "workspace with tiled pane should have_panes()"
    );
}

#[test]
fn has_panes_true_when_only_floating_panes_exist() {
    let mut ws = Shown::new(Size::new(800.0, 600.0));
    ws.floating_panes.push(FloatingPane {
        pane: Pane::new(PaneId(1), "float1"),
        position: Point::new(50.0, 50.0),
        size: Size::new(400.0, 300.0),
        is_active: true,
        original_column_idx: None,
        original_pane_idx: None,
    });
    assert!(
        ws.has_panes(),
        "workspace with only floating pane should have_panes()"
    );
}

#[test]
fn has_panes_false_when_empty() {
    let ws = Shown::new(Size::new(800.0, 600.0));
    assert!(!ws.has_panes(), "empty workspace should not have_panes()");
}

// ── Floating pane removal + domain switching ──

#[test]
fn remove_last_floating_pane_switches_domain_to_tiled() {
    let mut ws = workspace_with_floating_pane(42);

    // Remove the floating pane
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);

    // Domain should switch to Tiled when no floating panes remain
    assert!(
        ws.floating_panes.is_empty(),
        "floating panes should be empty after removal"
    );
    ws.deactivate_floating_panes();
    ws.focus_domain = FocusDomain::Tiled;
    assert_eq!(
        ws.focus_domain,
        FocusDomain::Tiled,
        "domain should be Tiled after deactivating floats"
    );
    // Tiled panes still exist
    assert!(ws.has_panes(), "workspace should still have tiled panes");
}

#[test]
fn remove_one_of_multiple_floating_panes_stays_in_floating_domain() {
    let mut ws = workspace_with_floating_pane(42);
    // Add a second floating pane
    ws.floating_panes.push(FloatingPane {
        pane: Pane::new(PaneId(43), "float43"),
        position: Point::new(100.0, 100.0),
        size: Size::new(300.0, 200.0),
        is_active: false,
        original_column_idx: None,
        original_pane_idx: None,
    });

    // Remove the first floating pane
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);

    // Domain should stay Floating since other floats exist
    assert!(
        !ws.floating_panes.is_empty(),
        "should still have floating panes after removing one"
    );
    // Only switch domain when floating_panes is empty
    if ws.floating_panes.is_empty() {
        ws.deactivate_floating_panes();
        ws.focus_domain = FocusDomain::Tiled;
    }
    assert_eq!(
        ws.focus_domain,
        FocusDomain::Floating,
        "domain should stay Floating with remaining floats"
    );
}

#[test]
fn remove_all_tiled_panes_leaves_workspace_empty() {
    let mut ws = workspace_with_pane(1);

    // Remove the only tiled pane (also removes the column)
    let removed = ws.m().scroll_mut().remove_pane(0, 0);
    assert!(removed.is_some(), "should remove the pane");

    // Workspace should now be empty
    assert!(!ws.has_panes());
    assert!(ws.scrolling.is_empty());
    assert!(ws.floating_panes.is_empty());
}

#[test]
fn remove_floating_pane_preserves_tiled_panes() {
    let mut ws = workspace_with_floating_pane(42);

    // Remove the floating pane
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);

    // Tiled pane (ID 99) should still exist
    assert!(ws.has_panes());
    assert!(!ws.scrolling.is_empty());
    assert!(ws.floating_panes.is_empty());
}

#[test]
fn deactivate_floating_panes_clears_all_active_flags() {
    let mut ws = workspace_with_floating_pane(42);
    ws.floating_panes.push(FloatingPane {
        pane: Pane::new(PaneId(43), "float43"),
        position: Point::new(100.0, 100.0),
        size: Size::new(300.0, 200.0),
        is_active: false,
        original_column_idx: None,
        original_pane_idx: None,
    });

    ws.deactivate_floating_panes();

    // All floating panes should have is_active = false
    assert!(
        ws.floating_panes.iter().all(|f| !f.is_active),
        "all floating panes should be deactivated"
    );
}

#[test]
fn remove_floating_pane_then_remove_tiled_leaves_workspace_empty() {
    let mut ws = workspace_with_floating_pane(42);

    // Remove the floating pane
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);

    // Workspace still has tiled pane
    assert!(
        ws.has_panes(),
        "workspace should still have panes after removing float"
    );

    // Now remove the only tiled pane (also removes the column)
    let removed = ws.m().scroll_mut().remove_pane(0, 0);
    assert!(removed.is_some(), "should remove the tiled pane");

    // Workspace should be completely empty
    assert!(
        !ws.has_panes(),
        "workspace should be empty after removing all panes"
    );
    assert!(ws.scrolling.is_empty(), "scrolling should be empty");
    assert!(
        ws.floating_panes.is_empty(),
        "floating panes should be empty"
    );
}

// ── Floating pane resize-follows-window ──

#[test]
fn update_working_area_scales_floating_pane_proportionally() {
    // 800x600 working area, float at 95% centered (matches `handle_float`).
    let mut ws = workspace_with_floating_pane(42);
    // Reposition the float to a 95%-coverage centered rect, like handle_float.
    let wa = ws.view.area;
    let fw = wa.size.w * 0.95;
    let fh = wa.size.h * 0.95;
    let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
    let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
    ws.floating_panes[0].position = Point::new(fx, fy);
    ws.floating_panes[0].size = Size::new(fw, fh);

    // Grow the working area to 1600x1200 (2x each axis).
    let new_wa = Rectangle::new(Point::new(0.0, 0.0), Size::new(1600.0, 1200.0));
    ws.m().update_working_area(new_wa);

    let f = &ws.floating_panes[0];
    // Size doubles (coverage preserved at 95%).
    assert!(
        (f.size.w - fw * 2.0).abs() < 0.01,
        "width should scale 2x: got {}",
        f.size.w
    );
    assert!(
        (f.size.h - fh * 2.0).abs() < 0.01,
        "height should scale 2x: got {}",
        f.size.h
    );
    // Position stays centered (relative position preserved).
    let new_w = new_wa.size.w;
    let new_h = new_wa.size.h;
    let expected_x = new_wa.loc.x + (new_w - f.size.w) / 2.0;
    let expected_y = new_wa.loc.y + (new_h - f.size.h) / 2.0;
    assert!(
        (f.position.x - expected_x).abs() < 0.01,
        "x should stay centered: got {}",
        f.position.x
    );
    assert!(
        (f.position.y - expected_y).abs() < 0.01,
        "y should stay centered: got {}",
        f.position.y
    );
}

#[test]
fn update_working_area_noop_when_size_unchanged() {
    let mut ws = workspace_with_floating_pane(42);
    let before = ws.floating_panes[0].position;
    let before_size = ws.floating_panes[0].size;
    // Same size → no rescale (guards against drift from repeated no-op updates).
    let area = ws.view.area;
    ws.m().update_working_area(area);
    assert_eq!(
        ws.floating_panes[0].position, before,
        "position must not drift on no-op"
    );
    assert_eq!(
        ws.floating_panes[0].size, before_size,
        "size must not drift on no-op"
    );
}
