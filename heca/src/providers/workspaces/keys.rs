//! **What the workspaces component calls its rows** — the identities a cursor, a right-click and a
//! drag are kept on.

use super::*;

// ── The workspaces container's row identities ──
//
// A row declares one of these and the host derives everything that has to *name* a row from it: the
// keyboard cursor, the right-click target, later the drag identity. They are the same identities
// `SidebarSelection` carries today, written as strings so nothing outside this component has to
// know a workspace from a pane — which is what lets a plugin row be pointed at at all.
//
// They must stay **stable across a tree rebuild**: a cursor that resets whenever a pane's git status
// changes is not a cursor. Positions therefore never appear in a key that has an id available.

/// `pane:<id>` — a pane card, tiled or floating.
///
/// **The pane declares this, not the row.** A sidebar row showing a pane is a *view* of it, so it
/// reads the pane's own identity rather than spelling a second copy.
pub(crate) use crate::chrome::pane_key;

/// `ws:<id>` — a workspace header.
///
/// **Its identity, never its position.** A key is what the keyboard cursor, a right-click, a drag
/// and a remembered hint letter are all kept on, so it has to survive the thing moving. These were
/// built from indices, so inserting a workspace or a column renamed every row after it and each of
/// those four silently reset — for rows that had not moved and had not changed.
pub(crate) fn workspace_key(ws_id: heca_core::layout::WorkspaceId) -> String {
    format!("ws:{}", ws_id.0)
}

// `column_key` lives beside `pane_key` in `crate::chrome`: the column in the scrolling area OWNS
// that identity and this provider shows a VIEW of it, so both sides read the same string rather
// than keeping two copies in step.
pub(crate) use crate::chrome::column_key;

// ── What each row kind does when you press it ──
//
// **Each item kind declares its own click, by name.** A workspace row and a pane row genuinely do
// different things, so neither inherits from the other and nothing is forced on a kind that has no
// gesture — a column row declares none, and that is a real answer.
//
// A name, not a closure: the same declaration answers the click, the `prefix+/` pick, a menu entry,
// a keybinding and RPC, and is routed by that action's own policy. Both of these **bind actions
// that already exist** — focusing a pane means the same thing with or without this component, so by
// the ownership test they are the app's and this component only points at them.

/// Pressing a **pane row** focuses that pane.
pub(super) fn pane_row_press(pane_id: PaneId) -> Intent {
    Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane_id.0 as i64))
}

// A workspace row declares **no click**: a `DockFrame`'s own header owns its press (the disclosure
// caret folds the section), and nothing forces a kind to declare a gesture it does not have. It
// used to carry `focus_workspace` here purely so the `prefix+/` picker had an intent to register —
// which is what pointing one declaration at two gestures looks like from the other side. Its hint
// says the same thing and more (`peek_selected` also lands the cursor on the row), so the
// stand-in is gone.

/// **What a `prefix+/` pick does to a row of this component — and it is not what a click does.**
///
/// A click on a row means *go there and leave*: `focus_pane` + the dock releasing the keyboard. A
/// pick means *look at that one*, so it moves the cursor onto the picked row and brings its pane to
/// the front **without leaving the dock** — this component's own `peek_selected`, aimed at a row by
/// its nav key instead of the cursor.
///
/// Pointing one intent at both gestures is precisely the bug this replaces: commit `e712d70`
/// (2026-07-30) made the row's hint target fire the row's *click*, and `prefix+/` on a sidebar row
/// started activating the pane and leaving the sidebar.
pub(super) fn row_hint(key: String) -> Intent {
    Intent::new(PEEK_SELECTED).arg("key", PropValue::Text(key))
}

/// The nav key naming the row a [`SidebarSelection`] points at.
///
/// The bridge between the domain-typed selection this container still keeps and the generic cursor
/// every row's outline now reads. It goes away with the selection itself — until
/// then it is the single conversion point, so the two cannot drift.
pub(crate) fn selection_key(selection: crate::chrome::SidebarSelection) -> String {
    use crate::chrome::SidebarSelection as S;
    match selection {
        S::Pane { pane_id } | S::FloatingPane { pane_id, .. } => pane_key(pane_id),
        S::Column { col_id, .. } => column_key(col_id),
        S::Workspace { ws_id, .. } => workspace_key(ws_id),
    }
}
