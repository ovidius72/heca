use super::*;
use crate::layout::FocusDomain;
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

/// Helper: create a workspace with one tiled pane and one floating pane this window has placed and
/// focused — the content add plus the per-window view the window sets when it reacts to a float.
fn workspace_with_floating_pane(pane_id: u64) -> Shown {
    let mut ws = workspace_with_pane(99); // one tiled pane
    let rect = Rectangle::new(Point::new(50.0, 50.0), Size::new(400.0, 300.0));
    ws.add_floating_pane(Pane::new(PaneId(pane_id), format!("float{}", pane_id)), None);
    ws.m().float_in_view(PaneId(pane_id), rect);
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
    ws.add_floating_pane(Pane::new(PaneId(1), "float1"), None);
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

    // The content loses the float...
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);
    assert!(ws.floating_panes.is_empty());

    // ...and when the window reconciles, with no float left it is back on the tiling.
    ws.m().reconcile_floats();
    assert_eq!(
        ws.m().focus_domain(),
        FocusDomain::Tiled,
        "domain should be Tiled after the last float goes"
    );
    assert!(ws.has_panes(), "workspace should still have tiled panes");
}

#[test]
fn remove_one_of_multiple_floating_panes_stays_in_floating_domain() {
    let mut ws = workspace_with_floating_pane(42);
    ws.add_floating_pane(Pane::new(PaneId(43), "float43"), None);

    // Remove the one this window had active.
    let idx = ws
        .floating_panes
        .iter()
        .position(|f| f.pane.id.0 == 42)
        .expect("floating pane should exist");
    ws.floating_panes.remove(idx);
    assert!(!ws.floating_panes.is_empty());

    // A float still remains, so the window stays in the floating domain; its active float is gone.
    ws.m().reconcile_floats();
    assert_eq!(
        ws.m().focus_domain(),
        FocusDomain::Floating,
        "domain should stay Floating with remaining floats"
    );
    assert_eq!(
        ws.m().reader().active_float(),
        None,
        "the float it had active is gone"
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
fn deactivate_floating_panes_clears_the_active_float() {
    let mut ws = workspace_with_floating_pane(42);
    ws.add_floating_pane(Pane::new(PaneId(43), "float43"), None);

    ws.m().deactivate_floating_panes();

    // This window now has no active float.
    assert_eq!(ws.m().reader().active_float(), None);
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

/// The rect this window draws floating pane `id` at.
fn float_rect(ws: &mut Shown, id: u64) -> Rectangle {
    ws.m()
        .reader()
        .floating()
        .into_iter()
        .find(|p| p.pane.id == PaneId(id))
        .expect("floating pane should exist")
        .rect
}

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
    ws.m()
        .float_in_view(PaneId(42), Rectangle::new(Point::new(fx, fy), Size::new(fw, fh)));

    // Grow the working area to 1600x1200 (2x each axis).
    let new_wa = Rectangle::new(Point::new(0.0, 0.0), Size::new(1600.0, 1200.0));
    ws.m().update_working_area(new_wa);

    let r = float_rect(&mut ws, 42);
    // Size doubles (coverage preserved at 95%).
    assert!((r.size.w - fw * 2.0).abs() < 0.01, "width should scale 2x: got {}", r.size.w);
    assert!((r.size.h - fh * 2.0).abs() < 0.01, "height should scale 2x: got {}", r.size.h);
    // Position stays centered (relative position preserved).
    let expected_x = new_wa.loc.x + (new_wa.size.w - r.size.w) / 2.0;
    let expected_y = new_wa.loc.y + (new_wa.size.h - r.size.h) / 2.0;
    assert!((r.loc.x - expected_x).abs() < 0.01, "x should stay centered: got {}", r.loc.x);
    assert!((r.loc.y - expected_y).abs() < 0.01, "y should stay centered: got {}", r.loc.y);
}

#[test]
fn update_working_area_noop_when_size_unchanged() {
    let mut ws = workspace_with_floating_pane(42);
    let before = float_rect(&mut ws, 42);
    // Same size → no rescale (guards against drift from repeated no-op updates).
    let area = ws.view.area;
    ws.m().update_working_area(area);
    assert_eq!(float_rect(&mut ws, 42), before, "rect must not drift on no-op");
}

/// **A workspace's pane list holds every pane** — the columns' in order, then the floating ones —
/// so a list, a sync or a search built on it cannot leave the floats out.
#[test]
fn a_workspaces_panes_include_the_floating_ones_after_the_columns() {
    let mut window = Windowed::new(Size::new(1280.0, 800.0), 1.0);
    for i in 1..=3u64 {
        window
            .m()
            .add_pane(Pane::new(PaneId(i), format!("p{i}")), None, true);
    }
    assert!(window.ws().float_tiled_pane(PaneId(2)).is_some());

    let ws = window.l().active_workspace().expect("a workspace");
    let ids: Vec<u64> = ws.panes().map(|pane| pane.id.0).collect();
    assert_eq!(ids, [1, 3, 2]);
    assert_eq!(ws.find_pane(PaneId(2)).map(|pane| pane.id), Some(PaneId(2)));
}
