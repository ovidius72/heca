//! The column shell — the box a column occupies in the scrolling area, and the identity it owns.
//!
//! **Why it exists at all.** A column was addressable only through the sidebar's view of it: the
//! only widget carrying `col:<id>` was a `MarkerGroup` inside the workspaces dock. So a pick asked
//! you to aim at a target visible only in a dock you may have collapsed, and the columns you were
//! actually looking at wore nothing (F003/P082/T474).
//!
//! It owns no content **yet**. Antonio, 2026-09-10: *"a pane should not only be positionable inside
//! columns but also live elsewhere, like a div … in the future I'd like to have sticky columns and
//! sticky panes."* That is where this goes — the panes become its children, so moving one between
//! containers is a tree operation. Until then it is the box and the name, and the panes are still
//! placed by the host.

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::Flex;
use heca_grid_ui::{ComponentExt, LayoutExt};

use super::model::ColumnShellModel;

/// The app seams the column binds, travelling as **one group** rather than one argument at a time
/// (AGENTS.md § 0b-bis rule 3) — so a test can hand it the app's edges and nothing else.
#[derive(Clone)]
pub(crate) struct ColumnCallbacks {
    /// What picking this column does: focus the pane you would land on. Emitted as an intent,
    /// never performed here — the column asks, the core does.
    pub(crate) pick: std::rc::Rc<dyn Fn(PaneId)>,
    /// The keycap tint a column target wears — the theme token, so both views of a column agree
    /// and a theme reload moves them together.
    pub(crate) tint: heca_grid_ui::Color,
}

/// The column shell component. Properties are struct fields and the constructor is a struct
/// literal (AGENTS.md § 0b-bis rule 2), so a call site reads without counting arguments.
pub(crate) struct ColumnShell<'a> {
    pub(crate) model: &'a ColumnShellModel,
    pub(crate) cb: &'a ColumnCallbacks,
}

impl ColumnShell<'_> {
    /// Build the retained tree for one column.
    ///
    /// **The column fills whatever rect it is given — it never carries one.** The scrolling engine
    /// owns a column's geometry and [`super::sync_columns`] writes that rect onto this tree's root
    /// every frame, exactly as the pane shell already works.
    pub(crate) fn build(self) -> Flex {
        let ColumnShellModel {
            col_id, focus_pane, ..
        } = *self.model;

        let mut column = Flex::column()
            .width(heca_grid_ui::Length::Percent(1.0))
            .height(heca_grid_ui::Length::Percent(1.0));

        // The column's own identity, from the data — never a position. A `ColumnId` is stable
        // across a split, which is what stops every column after an insertion being renamed.
        column = column.key(crate::chrome::column_key(col_id));

        // **What picking a column means: take me there.** A column with no pane declares nothing
        // and wears no letter, because there is nothing to go to.
        if let Some(pane_id) = focus_pane {
            let pick = self.cb.pick.clone();
            column = column.on_hint(move || pick(pane_id));
        }

        // **A column reads the same wherever you see it.** The sidebar's view tints its keycap
        // `success` so a column target is distinct from a pane's; this is the same target, so it
        // takes the same tint rather than the picker's default.
        column.hint_color(self.cb.tint)
    }
}
