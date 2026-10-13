//! **Each terminal's own font size** — an offset in points on top of the app's, kept by the
//! terminal's id, so a terminal in a dock or an overlay zooms the same way a pane's does.
//!
//! The effective size is `(base + offset).clamp(MIN, MAX)`, where `base` is the configured terminal
//! size plus the app-wide zoom. Both the grid a process is fitted to and the glyphs drawn come from
//! that one value, so they never disagree.

use std::collections::HashMap;

use crate::app::terminal_metrics::{TERMINAL_FONT_SIZE_MAX, TERMINAL_FONT_SIZE_MIN};
use crate::chrome::terminal::TerminalId;

/// Every terminal's font offset and the cell size it was measured to. A terminal absent here
/// follows the app's size.
#[derive(Default)]
pub(crate) struct TerminalZoom {
    offsets: HashMap<TerminalId, f32>,
    cells: HashMap<TerminalId, (f32, f32)>,
}

impl TerminalZoom {
    /// The font size `terminal` is drawn at, given the app's `base`.
    pub(crate) fn size(&self, terminal: TerminalId, base: f32) -> f32 {
        (base + self.offset(terminal)).clamp(TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX)
    }

    /// `terminal`'s offset in points; `0.0` when it follows the app.
    pub(crate) fn offset(&self, terminal: TerminalId) -> f32 {
        self.offsets.get(&terminal).copied().unwrap_or(0.0)
    }

    /// **Step `terminal`'s font by `delta` points** over the app's `base`, or back to following the
    /// app when `delta` is `0.0`. The size stops at the limits rather than storing an offset past
    /// them. Returns whether anything changed.
    pub(crate) fn step(&mut self, terminal: TerminalId, base: f32, delta: f32) -> bool {
        let current = self.offset(terminal);
        let next = if delta == 0.0 {
            0.0
        } else {
            (base + current + delta).clamp(TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX) - base
        };
        if next == current {
            return false;
        }
        if next == 0.0 {
            self.offsets.remove(&terminal);
            self.cells.remove(&terminal);
        } else {
            self.offsets.insert(terminal, next);
        }
        true
    }

    /// The terminals that do not follow the app's size.
    pub(crate) fn zoomed(&self) -> Vec<TerminalId> {
        self.offsets.keys().copied().collect()
    }

    /// The cell size `terminal` was measured to, if it has its own.
    pub(crate) fn cell(&self, terminal: TerminalId) -> Option<(f32, f32)> {
        self.cells.get(&terminal).copied()
    }

    /// Record the cell size `terminal`'s own font measures to.
    pub(crate) fn set_cell(&mut self, terminal: TerminalId, cell: (f32, f32)) {
        self.cells.insert(terminal, cell);
    }

    /// Forget every terminal `keep` does not keep, so ended terminals leave nothing behind.
    pub(crate) fn retain(&mut self, keep: impl Fn(TerminalId) -> bool) {
        self.offsets.retain(|id, _| keep(*id));
        self.cells.retain(|id, _| keep(*id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: f32 = 13.0;

    #[test]
    fn a_terminal_follows_the_app_until_it_is_zoomed() {
        let zoom = TerminalZoom::default();
        assert_eq!(zoom.size(TerminalId(1), BASE), BASE);
        assert_eq!(
            zoom.size(TerminalId(1), 20.0),
            20.0,
            "it follows the app's zoom too"
        );
    }

    /// **The zoom is the terminal's own**: stepping one leaves every other where it was, whoever
    /// owns either of them.
    #[test]
    fn zooming_one_terminal_leaves_the_others_alone() {
        let mut zoom = TerminalZoom::default();
        assert!(zoom.step(TerminalId(7), BASE, 2.0));
        assert_eq!(zoom.size(TerminalId(7), BASE), BASE + 2.0);
        assert_eq!(zoom.size(TerminalId(8), BASE), BASE);
        assert_eq!(zoom.zoomed(), vec![TerminalId(7)]);
    }

    #[test]
    fn a_step_stops_at_the_limits_and_says_when_nothing_changed() {
        let mut zoom = TerminalZoom::default();
        assert!(zoom.step(TerminalId(1), BASE, 1_000.0));
        assert_eq!(zoom.size(TerminalId(1), BASE), TERMINAL_FONT_SIZE_MAX);
        assert!(!zoom.step(TerminalId(1), BASE, 1.0), "already at the top");
        assert!(zoom.step(TerminalId(1), BASE, -1_000.0));
        assert_eq!(zoom.size(TerminalId(1), BASE), TERMINAL_FONT_SIZE_MIN);
    }

    #[test]
    fn a_reset_follows_the_app_again_and_forgets_the_cell() {
        let mut zoom = TerminalZoom::default();
        zoom.step(TerminalId(1), BASE, 3.0);
        zoom.set_cell(TerminalId(1), (9.0, 18.0));
        assert!(zoom.step(TerminalId(1), BASE, 0.0));
        assert_eq!(zoom.size(TerminalId(1), BASE), BASE);
        assert_eq!(zoom.cell(TerminalId(1)), None);
        assert!(zoom.zoomed().is_empty());
        assert!(!zoom.step(TerminalId(1), BASE, 0.0), "nothing to reset");
    }

    #[test]
    fn an_ended_terminal_leaves_nothing_behind() {
        let mut zoom = TerminalZoom::default();
        zoom.step(TerminalId(1), BASE, 1.0);
        zoom.set_cell(TerminalId(1), (9.0, 18.0));
        zoom.step(TerminalId(2), BASE, 1.0);
        zoom.retain(|id| id == TerminalId(2));
        assert_eq!(zoom.zoomed(), vec![TerminalId(2)]);
        assert_eq!(zoom.cell(TerminalId(1)), None);
    }
}
