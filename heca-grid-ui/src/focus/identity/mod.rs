//! **The keyboard by widget identity** — the browser's `document.activeElement` and
//! `element.focus()`, for a host that remembers a widget by its focus identity
//! ([`Base::focus_id`](crate::component::Base::focus_id)) rather than by a path a rebuild moves.

use super::at;
use super::settle::{holder_path, settle};
use crate::component::Component;

/// **Which widget holds the keyboard**: its focus identity, or `None` when nobody holds it.
pub fn keyboard_holder(root: &dyn Component) -> Option<u64> {
    let path = holder_path(root)?;
    Some(at(root, &path).base().focus_id())
}

/// **Give the keyboard to the widget whose focus identity is `id`**, whoever holds it now — the way
/// a click on it would. Settled at once, so a read straight after it is already true. Returns
/// whether a shown widget has that identity; one hidden, or no longer in the tree, takes nothing.
pub fn focus_widget_by_id(root: &mut dyn Component, id: u64) -> bool {
    settle(root);
    let Some(path) = path_of(root, id) else {
        return false;
    };
    at(root, &path).base().focus(false);
    settle(root);
    true
}

/// The path to the shown widget whose focus identity is `id`.
fn path_of(node: &dyn Component, id: u64) -> Option<Vec<usize>> {
    if node.base().focus_door.id.get() == id {
        return Some(Vec::new());
    }
    node.base()
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.is_shown())
        .find_map(|(i, child)| {
            let mut sub = path_of(child.as_ref(), id)?;
            sub.insert(0, i);
            Some(sub)
        })
}

#[cfg(test)]
mod tests;
