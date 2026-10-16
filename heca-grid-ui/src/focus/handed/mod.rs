//! **Where a widget hands its keyboard** — the one answer to
//! [`keyboard_to`](crate::builders::PresenceExt::keyboard_to), read by a press and by the pick
//! letters alike, so the widget a press gives the keyboard to and the widget that goes unlettered
//! for it are always the same one.

use crate::component::Component;

#[cfg(test)]
mod tests;

/// The path, from `node`, to what a press on `node` gives the keyboard to: the first shown widget
/// that takes it, in `node`'s part named by its [`keyboard_to`](crate::Base::keyboard_to). `None`
/// when `node` names no part, the part is not there, or nothing in it takes the keyboard.
pub(crate) fn handed_to(node: &dyn Component) -> Option<Vec<usize>> {
    let area = node.base().keyboard_to.as_deref()?;
    let mut path = part_path(node, area)?;
    let part = super::at(node, &path);
    path.extend(first_taker(part)?);
    Some(path)
}

/// The path to the part of `node` placed in the named area, depth-first.
fn part_path(node: &dyn Component, area: &str) -> Option<Vec<usize>> {
    if node.base().grid_area.as_deref() == Some(area) {
        return Some(Vec::new());
    }
    node.base().children.iter().enumerate().find_map(|(i, child)| {
        let mut path = part_path(child.as_ref(), area)?;
        path.insert(0, i);
        Some(path)
    })
}

/// The path to the first shown widget in `node` (itself included) that takes the keyboard.
fn first_taker(node: &dyn Component) -> Option<Vec<usize>> {
    if node.focusable() {
        return Some(Vec::new());
    }
    node.base()
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.is_shown())
        .find_map(|(i, child)| {
            let mut path = first_taker(child.as_ref())?;
            path.insert(0, i);
            Some(path)
        })
}
