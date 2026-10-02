//! Where the keyboard is in the window tree, and the chrome store agreeing with it.
//!
//! There is one truth: the tree. A press on a control, a dock taking the keyboard and a dialog
//! closing all move it there, and the chrome store's `focused_container` — which the key rules read
//! to tell a dock from the panes — **follows**. The reverse also holds, through the same door: a
//! host action that focuses a dock sets the store, and the dock's region (which follows that signal)
//! takes the keyboard on the next settle. Neither side keeps a record the other can contradict.

use crate::app_state::AppState;

/// What the store should say, given where the keyboard is: the dock the owner sits in, `None` when
/// it sits in the page outside any dock — and no opinion (`Keep`) when nothing holds it or a surface
/// above the page does, because that surface will hand it back to the dock when it closes.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StoreSays {
    Keep,
    Dock(Option<String>),
}

pub(crate) fn store_says(owner: Option<&heca_grid_ui::KeyboardOwner>) -> StoreSays {
    match owner {
        Some(owner) if !owner.in_surface => StoreSays::Dock(owner.scope.clone()),
        _ => StoreSays::Keep,
    }
}

/// Make the window tree's keyboard single and bring the store in line with it. Called once a frame
/// after the tree has ticked, and after a press, so what the key rules read is current.
pub(crate) fn settle_window_focus(state: &mut AppState) {
    heca_grid_ui::settle_focus(&mut state.window_root);
    let owner = heca_grid_ui::keyboard_owner(&state.window_root);
    if let StoreSays::Dock(dock) = store_says(owner.as_ref()) {
        state.chrome_state.set_focused_container(dock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::KeyboardOwner;

    fn owner(scope: Option<&str>, in_surface: bool) -> KeyboardOwner {
        KeyboardOwner {
            path: vec![0],
            scope: scope.map(str::to_string),
            in_surface,
        }
    }

    #[test]
    fn a_control_in_a_dock_makes_that_dock_the_focused_container() {
        assert_eq!(
            store_says(Some(&owner(Some("workspaces"), false))),
            StoreSays::Dock(Some("workspaces".into()))
        );
    }

    /// Clicking a control outside every dock takes the keyboard from the dock — one truth, so the
    /// store lets go too and the keys go back to the panes.
    #[test]
    fn a_control_outside_every_dock_leaves_no_dock_focused() {
        assert_eq!(store_says(Some(&owner(None, false))), StoreSays::Dock(None));
    }

    /// A dialog over a dock does not take the dock's claim with it: closing the dialog hands the
    /// keyboard back to the dock, and the store never stopped saying so.
    #[test]
    fn a_surface_above_the_page_leaves_the_store_alone() {
        assert_eq!(
            store_says(Some(&owner(Some("workspaces"), true))),
            StoreSays::Keep
        );
        assert_eq!(store_says(None), StoreSays::Keep);
    }
}
