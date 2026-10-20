//! **What chrome things are called** — panes, columns, a pane's controls, drop places and pick
//! scopes. Each name is composed here once, so no call site spells a format.

/// **The scope a column's letter belongs to** — "somewhere a pane can be moved to".
///
/// A column wears a letter while `prefix+Ctrl+c` is choosing a destination, and at no other time.
/// Naming the scope is what keeps it out of `prefix+/`, where its letter landed you in the dock
/// having selected nothing. Addressing it by key is unaffected: that names `col:<id>` outright
/// rather than collecting a set.
pub(crate) const COLUMN_PICK_SCOPE: &str = "column.destination";

/// **The scope a mounted dock's own letter belongs to** — "a place the keyboard can go".
///
/// The host wraps every mounted container in a focus scope that clicks to `FocusDock`, which makes
/// the whole dock actionable and therefore lettered. That letter is a *destination for the
/// keyboard*, which `prefix+Shift+e` already offers on every dock — so in `prefix+/` it spent one
/// of the 52 on something with its own binding, once per placement, while the rows inside it were
/// the things you actually wanted to reach.
///
/// Naming the scope keeps it out of `prefix+/`. The dock pick is unaffected: it addresses the
/// container by key rather than collecting a set, exactly as the column pick does.
pub(crate) const DOCK_PICK_SCOPE: &str = "dock.destination";

/// `pane:<id>` — **a pane's identity**, declared by the pane itself.
///
/// Whoever owns the thing declares its identity; anything else showing it is a view. A pane owns
/// `pane:7`; the sidebar row and the exposé card that show that pane are second views of it. This
/// lives here rather than in a provider so the pane and every view of it read the SAME string
/// instead of keeping two copies in step.
///
/// It is never a position and never a counter: a `PaneId` survives every tree rebuild, which is
/// what lets a hint letter stay with the same pane between openings of the picker.
pub(crate) fn pane_key(pane: heca_core::layout::PaneId) -> String {
    format!("pane:{}", pane.0)
}

/// The pane a `pane:<id>` key names — the inverse of [`pane_key`].
pub(crate) fn pane_id_of_key(key: &str) -> Option<heca_core::layout::PaneId> {
    key.strip_prefix("pane:")?
        .parse()
        .ok()
        .map(heca_core::layout::PaneId)
}

/// `col:<id>` — a column. No workspace prefix: a [`ColumnId`](heca_core::layout::ColumnId) is
/// allocated from the session's counter, so it is unique across the whole session on its own.
///
/// It lives here beside [`pane_key`] rather than in the workspaces provider because the column in
/// the scrolling area is what OWNS this identity; the provider's group is a second view of it, and
/// two homes for one string is two spellings waiting to drift.
pub(crate) fn column_key(col_id: heca_core::layout::ColumnId) -> String {
    format!("col:{}", col_id.0)
}

/// What a pane says it is when it is carried, and what a place for a pane takes.
pub(crate) const PANE_DRAG_KIND: &str = "pane";

/// `slot:<n>` — **the place a new column would be made**, `n` columns from the left.
///
/// The workspace writes it and the drop reads it back, so what the name means lives in one place.
pub(crate) fn slot_key(at: usize) -> String {
    format!("slot:{at}")
}

/// `row:<col>:<row>` — **the place a pane would be put at row `row` of column `col`**, counted with
/// the pane where it is.
pub(crate) fn row_key(col: usize, row: usize) -> String {
    format!("row:{col}:{row}")
}

/// The column and row a `row:<col>:<row>` key names — the inverse of [`row_key`].
pub(crate) fn row_of_key(key: &str) -> Option<(usize, usize)> {
    let (col, row) = key.strip_prefix("row:")?.split_once(':')?;
    Some((col.parse().ok()?, row.parse().ok()?))
}

/// The index a `slot:<n>` key names — the inverse of [`slot_key`].
pub(crate) fn slot_of_key(key: &str) -> Option<usize> {
    key.strip_prefix("slot:")?.parse().ok()
}

/// **What one of a pane's own controls is called** — `pane:7-zoom`.
///
/// A control named for its ROLE is not named at all: every pane has a zoom button, so `zoom` says
/// which button it is only if you already know which pane you are looking at. The name says which
/// pane, so the picker can hold all of them at once and give each its own letter.
///
/// Composed here, once, so no call site spells the format — the same reason [`pane_key`] and
/// [`column_key`] live together. An action contributed from `config.toml` or by a plugin gets a
/// correct name without its author knowing any of this.
pub(crate) fn pane_control_key(pane: heca_core::layout::PaneId, control: &str) -> String {
    format!("{}-{control}", pane_key(pane))
}

#[cfg(test)]
mod tests {
    use heca_core::layout::{ColumnId, PaneId};

    /// **A name says which thing, so the same thing has one name wherever it is drawn.**
    ///
    /// The pane in the scrolling area, the sidebar row for it and the exposé card are three views
    /// of one pane. They call it the same, so the picker gives it one letter instead of three.
    #[test]
    fn every_view_of_a_pane_calls_it_the_same() {
        assert_eq!(super::pane_key(PaneId(7)), "pane:7");
        assert_eq!(super::column_key(ColumnId(3)), "col:3");
    }

    /// **…and a control is named for the pane it belongs to, never for its role.**
    ///
    /// `zoom` is every pane's zoom button. Naming them that gave all of them one letter the moment
    /// a name was taken at face value, so you could not say which pane you meant.
    #[test]
    fn two_panes_controls_are_not_the_same_control() {
        let a = super::pane_control_key(PaneId(7), "zoom");
        let b = super::pane_control_key(PaneId(9), "zoom");
        assert_eq!(a, "pane:7-zoom");
        assert_ne!(a, b, "one pane's zoom is not another's");
    }

    /// A control is not its pane either — they are two targets and wear two letters.
    #[test]
    fn a_pane_and_its_control_are_two_things() {
        assert_ne!(
            super::pane_key(PaneId(7)),
            super::pane_control_key(PaneId(7), "zoom"),
        );
    }
}
