use super::scrolling::ScrollingSpace;
use super::types::*;

/// Which layout domain has keyboard focus.
///
/// When `Floating`, only pane-local actions (close, rename) operate on the
/// active floating pane. Tiled-layout mutations (resize, zoom, swap, move,
/// column navigation) are no-op. Navigation between floating panes is
/// deferred to a future phase.
///
/// The floating domain is **modal** — mouse clicks, keyboard shortcuts, and
/// sidebar selection cannot switch to a tiled pane while a floating pane is
/// focused. Use `prefix+f` (unfloat), closing the floating pane, or
/// `prefix+i` (toggle) to return to `Tiled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusDomain {
    #[default]
    Tiled,
    Floating,
}

/// A workspace contains a scrolling layout and optionally floating panes.
///
/// This is heca's equivalent of NIRI's `Workspace<W>`, which contains
/// both a `ScrollingSpace` (tiling) and a `FloatingSpace` (floating windows).
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: Option<String>,
    /// The scrollable-tiling layout.
    pub scrolling: ScrollingSpace,
    /// Floating panes (future feature).
    pub floating_panes: Vec<FloatingPane>,
    /// Which layout domain has keyboard focus.
    pub focus_domain: FocusDomain,
}

/// A floating pane with position and size.
#[derive(Debug, Clone)]
pub struct FloatingPane {
    pub pane: super::column::Pane,
    pub position: Point,
    pub size: Size,
    pub is_active: bool,
    /// Original column index when floated (for restore).
    pub original_column_idx: Option<usize>,
    /// Original pane index within the column when floated.
    pub original_pane_idx: Option<usize>,
}

impl Workspace {
    pub fn new(
        id: WorkspaceId,
        working_area: Rectangle,
        scale: f64,
        options: LayoutOptions,
    ) -> Self {
        let scrolling = ScrollingSpace::new(working_area, scale, options);
        Self {
            id,
            name: None,
            scrolling,
            floating_panes: Vec::new(),
            focus_domain: FocusDomain::default(),
        }
    }

    pub fn has_panes(&self) -> bool {
        !self.scrolling.is_empty() || !self.floating_panes.is_empty()
    }

    pub fn active_pane(&self) -> Option<&super::column::Pane> {
        if self.focus_domain == FocusDomain::Floating {
            self.floating_panes
                .iter()
                .find(|p| p.is_active)
                .map(|p| &p.pane)
        } else {
            self.scrolling.active_pane()
        }
    }

    /// Clear active state from all floating panes.
    pub fn deactivate_floating_panes(&mut self) {
        for float in &mut self.floating_panes {
            float.is_active = false;
        }
    }

    /// Activate exactly one floating pane by id.
    pub fn activate_floating_pane(&mut self, pane_id: PaneId) -> bool {
        let mut found = false;
        for float in &mut self.floating_panes {
            let is_target = float.pane.id == pane_id;
            float.is_active = is_target;
            found |= is_target;
        }
        self.focus_domain = if found {
            FocusDomain::Floating
        } else {
            FocusDomain::Tiled
        };
        found
    }

    /// Find any pane by ID across both scrolling and floating.
    pub fn find_pane(&self, pane_id: PaneId) -> Option<&super::column::Pane> {
        // Check floating panes first
        if let Some(f) = self.floating_panes.iter().find(|f| f.pane.id == pane_id) {
            return Some(&f.pane);
        }
        for col in &self.scrolling.columns {
            let found = col.panes.iter().find(|p| p.id == pane_id);
            if found.is_some() {
                return found;
            }
        }
        None
    }

    pub fn find_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut super::column::Pane> {
        // Check floating panes first
        if let Some(f) = self
            .floating_panes
            .iter_mut()
            .find(|f| f.pane.id == pane_id)
        {
            return Some(&mut f.pane);
        }
        // Check scrolling columns
        for col in &mut self.scrolling.columns {
            if let Some(pane) = col.panes.iter_mut().find(|p| p.id == pane_id) {
                return Some(pane);
            }
        }
        None
    }

    /// Update working area (called on resize).
    pub fn update_working_area(&mut self, working_area: Rectangle) {
        // Capture the old working area (as plain f64s, to avoid borrowing
        // `self.scrolling.working_area` across the mutable call below).
        let (old_x, old_y, old_w, old_h) = (
            self.scrolling.working_area.loc.x,
            self.scrolling.working_area.loc.y,
            self.scrolling.working_area.size.w,
            self.scrolling.working_area.size.h,
        );
        self.scrolling.update_working_area(working_area);
        // Scale floating panes proportionally so they keep their relative position
        // + coverage when the working area changes (window resize, chrome toggle).
        // Without this, a float spawned at 95% keeps its absolute pixel size while
        // the window grows/shrinks around it — drifting off-screen or looking
        // stranded. Position is stored relative to the working-area origin (see
        // `handle_float` + the render path), so a pure scale by the size ratio is
        // correct (plus an origin shift in case `loc` ever moves).
        let sx = working_area.size.w / old_w.max(1.0);
        let sy = working_area.size.h / old_h.max(1.0);
        if (sx - 1.0).abs() > 1e-6 || (sy - 1.0).abs() > 1e-6 {
            for float in &mut self.floating_panes {
                float.position.x = working_area.loc.x + (float.position.x - old_x) * sx;
                float.position.y = working_area.loc.y + (float.position.y - old_y) * sy;
                float.size.w *= sx;
                float.size.h *= sy;
            }
        }
    }

    /// Advance all animations in this workspace.
    pub fn advance_animations(&mut self) {
        self.scrolling.advance_animations();
    }

    pub fn are_animations_ongoing(&self) -> bool {
        self.scrolling.are_animations_ongoing()
    }

    /// Add a pane to the scrolling layout.
    ///
    /// `new_column_id` is spent only when a column is actually created (`column_idx` is `None`).
    /// It is handed in rather than derived here because a [`ColumnId`] must be **allocated**: see
    /// [`Session::next_id`](super::session::Session::next_id), the one counter panes and workspaces
    /// already draw from.
    pub fn add_pane(
        &mut self,
        pane: super::column::Pane,
        column_idx: Option<usize>,
        activate: bool,
        width: ColumnWidth,
        new_column_id: ColumnId,
    ) {
        use super::column::Column;

        if let Some(idx) = column_idx {
            // Add to existing column.
            self.scrolling.add_pane_to_column(idx, None, pane, activate);
        } else {
            // Create new column.
            let col = Column::new(new_column_id, pane, width);
            self.scrolling.add_column(None, col, activate);
        }
    }

    /// Focus left in the scrolling layout.
    pub fn focus_left(&mut self) -> bool {
        if self.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scrolling.focus_left()
        }
    }

    /// Focus right in the scrolling layout.
    pub fn focus_right(&mut self) -> bool {
        if self.focus_domain == FocusDomain::Floating {
            false // TODO: floating focus
        } else {
            self.scrolling.focus_right()
        }
    }

    /// Focus up (previous pane in column, or previous workspace).
    pub fn focus_up(&mut self) -> bool {
        if self.focus_domain == FocusDomain::Floating {
            false // TODO
        } else if let Some(col) = self.scrolling.active_column_mut() {
            if col.focus_up() {
                true
            } else {
                // Wrap to previous column's last pane.
                let col_idx = self.scrolling.active_column_idx;
                if col_idx > 0 {
                    self.scrolling.activate_column(col_idx - 1);
                    if let Some(new_col) = self.scrolling.active_column_mut() {
                        let last_idx = new_col.panes.len().saturating_sub(1);
                        new_col.activate_pane(last_idx);
                    }
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }

    /// Focus down (next pane in column, or next workspace).
    pub fn focus_down(&mut self) -> bool {
        if self.focus_domain == FocusDomain::Floating {
            false // TODO
        } else if let Some(col) = self.scrolling.active_column_mut() {
            if col.focus_down() {
                true
            } else {
                // Wrap to next column's first pane.
                let col_idx = self.scrolling.active_column_idx;
                if col_idx + 1 < self.scrolling.columns.len() {
                    self.scrolling.activate_column(col_idx + 1);
                    if let Some(new_col) = self.scrolling.active_column_mut() {
                        new_col.activate_pane(0);
                    }
                    true
                } else {
                    false
                }
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::column::Pane;
    use crate::layout::session::Session;
    use crate::layout::types::LayoutOptions;

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
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        for i in 1..=3u64 {
            session.add_pane(Pane::new(PaneId(i), format!("p{i}")), None, true);
        }
        if let Some(ws) = session.active_workspace_mut() {
            ws.scrolling.remove_column(1);
        }
        session.add_pane(Pane::new(PaneId(9), "p9".to_string()), None, true);

        let ids: Vec<u64> = session
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
    fn workspace_with_pane(pane_id: u64) -> Workspace {
        let mut ws = Workspace::new(
            WorkspaceId(0),
            Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
            1.0,
            LayoutOptions::default(),
        );
        let pane = Pane::new(PaneId(pane_id), format!("pane{}", pane_id));
        ws.add_pane(pane, None, true, ColumnWidth::Proportion(0.5), ColumnId(pane_id));
        ws
    }

    /// Helper: create a workspace with one tiled pane and one floating pane.
    fn workspace_with_floating_pane(pane_id: u64) -> Workspace {
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
        let mut ws = Workspace::new(
            WorkspaceId(0),
            Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
            1.0,
            LayoutOptions::default(),
        );
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
        let ws = Workspace::new(
            WorkspaceId(0),
            Rectangle::new(Point::new(0.0, 0.0), Size::new(800.0, 600.0)),
            1.0,
            LayoutOptions::default(),
        );
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
        let removed = ws.scrolling.remove_pane(0, 0);
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
        let removed = ws.scrolling.remove_pane(0, 0);
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
        let wa = ws.scrolling.working_area;
        let fw = wa.size.w * 0.95;
        let fh = wa.size.h * 0.95;
        let fx = wa.loc.x + (wa.size.w - fw) / 2.0;
        let fy = wa.loc.y + (wa.size.h - fh) / 2.0;
        ws.floating_panes[0].position = Point::new(fx, fy);
        ws.floating_panes[0].size = Size::new(fw, fh);

        // Grow the working area to 1600x1200 (2x each axis).
        let new_wa = Rectangle::new(Point::new(0.0, 0.0), Size::new(1600.0, 1200.0));
        ws.update_working_area(new_wa);

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
        ws.update_working_area(ws.scrolling.working_area);
        assert_eq!(
            ws.floating_panes[0].position, before,
            "position must not drift on no-op"
        );
        assert_eq!(
            ws.floating_panes[0].size, before_size,
            "size must not drift on no-op"
        );
    }
}
