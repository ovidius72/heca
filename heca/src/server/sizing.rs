//! Pane-size arbitration. A running terminal has one grid, but several windows may show the same
//! pane at once at different sizes. This decides, per the `pane_size` setting, which window's grid
//! the one process runs at. Only rows×cols are shared; the picture each window draws uses its own
//! cell size, so nothing here touches how a window looks — only the one size the process is given.
//!
//! One place owns this. A window is just a client id: it reports the grid it shows a terminal at
//! ([`request`](PaneSizing::request)) and becomes the active one when it types or resizes
//! ([`touch`](PaneSizing::touch)). The server asks [`resolve`](PaneSizing::resolve) for the grid to
//! run at and applies it only when it moved. No caller has to know the rule.

use std::collections::HashMap;

use heca_config::settings::PaneSize;

use crate::app::backend_store::TerminalOwner;

/// The grid a window shows a terminal at: the shared part (`cols`, `rows`) and that window's own
/// pixel size (`cell`, `scale`), kept so the active window's picture stays sharp when it wins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cell: (f32, f32),
    pub(crate) scale: f32,
}

/// Which windows show each terminal, at what grid, and which one last typed in it.
#[derive(Default)]
pub(crate) struct PaneSizing {
    /// Per terminal: each showing window's (client id → grid it last reported).
    requested: HashMap<TerminalOwner, HashMap<u64, Grid>>,
    /// Per terminal: the window that last typed in or resized it — the "latest" one.
    active: HashMap<TerminalOwner, u64>,
    /// Per terminal: the grid last handed to the process, so a resize happens only on a real change.
    applied: HashMap<TerminalOwner, Grid>,
}

impl PaneSizing {
    /// A window reported the grid it shows this terminal at.
    pub(crate) fn request(&mut self, owner: &TerminalOwner, client: u64, grid: Grid) {
        self.requested.entry(owner.clone()).or_default().insert(client, grid);
    }

    /// A window typed into (or resized) this terminal: it becomes the active one.
    pub(crate) fn touch(&mut self, owner: &TerminalOwner, client: u64) {
        self.active.insert(owner.clone(), client);
    }

    /// A window is gone: forget its grids and any claim to being active. Returns the terminals whose
    /// winning grid may now differ, so the caller can re-apply them (the pane grows back when the
    /// window that was holding it small, or active, leaves).
    pub(crate) fn forget_client(&mut self, client: u64) -> Vec<TerminalOwner> {
        let mut affected = Vec::new();
        for (owner, grids) in &mut self.requested {
            if grids.remove(&client).is_some() {
                affected.push(owner.clone());
            }
        }
        self.active.retain(|owner, active| {
            let keep = *active != client;
            if !keep && !affected.contains(owner) {
                affected.push(owner.clone());
            }
            keep
        });
        affected
    }

    /// A terminal ended: drop everything about it.
    pub(crate) fn forget_terminal(&mut self, owner: &TerminalOwner) {
        self.requested.remove(owner);
        self.active.remove(owner);
        self.applied.remove(owner);
    }

    /// The grid this terminal's process should run at under `policy`, or `None` when no window shows
    /// it yet. A terminal shown by one window always gets that window's grid, whatever the policy —
    /// `min`, `max` and "the active one" all come out the same for a single entry.
    pub(crate) fn resolve(&self, owner: &TerminalOwner, policy: PaneSize) -> Option<Grid> {
        let grids = self.requested.get(owner)?;
        let active = self.active.get(owner).and_then(|client| grids.get(client)).copied();
        match policy {
            // The window last typed in. Fall back to any showing window until one has typed, so a
            // freshly shown pane still gets a size.
            PaneSize::Latest => active.or_else(|| grids.values().copied().next()),
            PaneSize::Smallest => self.extent(grids, active, usize::min),
            PaneSize::Largest => self.extent(grids, active, usize::max),
        }
    }

    /// `cols`/`rows` folded across every showing window with `pick` (min or max). The pixel cell
    /// comes from the active window when one has typed, else from any — only the grid is shared, so
    /// the exact cell only decides how the process reports its pixel size to the program in it.
    fn extent(
        &self,
        grids: &HashMap<u64, Grid>,
        active: Option<Grid>,
        pick: fn(usize, usize) -> usize,
    ) -> Option<Grid> {
        let mut values = grids.values().copied();
        let first = values.next()?;
        let (cols, rows) = values.fold((first.cols, first.rows), |(cols, rows), grid| {
            (pick(cols, grid.cols), pick(rows, grid.rows))
        });
        let base = active.unwrap_or(first);
        Some(Grid { cols, rows, cell: base.cell, scale: base.scale })
    }

    /// Record the grid just applied to the process and say whether it changed — so the caller
    /// resizes the process only when the winning grid actually moved. A needless PTY resize can
    /// disturb a full-screen program (it redraws), so this guard is not just a saving.
    pub(crate) fn mark_applied(&mut self, owner: &TerminalOwner, grid: Grid) -> bool {
        self.applied.insert(owner.clone(), grid) != Some(grid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> TerminalOwner {
        TerminalOwner::Pane(heca_core::layout::PaneId(1))
    }

    fn grid(cols: usize, rows: usize) -> Grid {
        Grid { cols, rows, cell: (8.0, 16.0), scale: 1.0 }
    }

    #[test]
    fn one_window_gets_its_own_size_under_every_policy() {
        let mut sizing = PaneSizing::default();
        sizing.request(&owner(), 1, grid(80, 24));
        for policy in [PaneSize::Smallest, PaneSize::Latest, PaneSize::Largest] {
            assert_eq!(sizing.resolve(&owner(), policy), Some(grid(80, 24)), "{policy:?}");
        }
    }

    #[test]
    fn no_window_showing_means_no_size() {
        let sizing = PaneSizing::default();
        assert_eq!(sizing.resolve(&owner(), PaneSize::Latest), None);
    }

    #[test]
    fn latest_follows_the_window_that_last_typed() {
        let mut sizing = PaneSizing::default();
        sizing.request(&owner(), 1, grid(80, 24));
        sizing.request(&owner(), 2, grid(120, 40));
        sizing.touch(&owner(), 1);
        assert_eq!(sizing.resolve(&owner(), PaneSize::Latest), Some(grid(80, 24)));
        sizing.touch(&owner(), 2);
        assert_eq!(sizing.resolve(&owner(), PaneSize::Latest), Some(grid(120, 40)));
    }

    #[test]
    fn smallest_and_largest_fold_each_dimension() {
        let mut sizing = PaneSizing::default();
        sizing.request(&owner(), 1, grid(80, 40));
        sizing.request(&owner(), 2, grid(120, 24));
        assert_eq!(sizing.resolve(&owner(), PaneSize::Smallest).map(|g| (g.cols, g.rows)), Some((80, 24)));
        assert_eq!(sizing.resolve(&owner(), PaneSize::Largest).map(|g| (g.cols, g.rows)), Some((120, 40)));
    }

    #[test]
    fn a_window_leaving_frees_the_size_and_names_the_terminal() {
        let mut sizing = PaneSizing::default();
        sizing.request(&owner(), 1, grid(80, 24));
        sizing.request(&owner(), 2, grid(120, 40));
        sizing.touch(&owner(), 2);
        let affected = sizing.forget_client(2);
        assert_eq!(affected, vec![owner()]);
        // Only window 1 is left: its size wins under latest, even though it never typed.
        assert_eq!(sizing.resolve(&owner(), PaneSize::Latest), Some(grid(80, 24)));
        assert_eq!(sizing.resolve(&owner(), PaneSize::Largest), Some(grid(80, 24)));
    }

    #[test]
    fn apply_is_marked_only_on_a_real_change() {
        let mut sizing = PaneSizing::default();
        assert!(sizing.mark_applied(&owner(), grid(80, 24)), "first apply is a change");
        assert!(!sizing.mark_applied(&owner(), grid(80, 24)), "same grid is not");
        assert!(sizing.mark_applied(&owner(), grid(100, 24)), "a new grid is");
    }
}
