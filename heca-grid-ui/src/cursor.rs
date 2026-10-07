//! **What the pointer looks like over a widget** — declared by the widget, read from the tree.
//!
//! A widget says what the cursor should be while the pointer is over it, the way CSS `cursor` does,
//! with [`cursor`](crate::builders::ComponentExt::cursor). [`cursor_at`] asks the tree: the pointer
//! is over whatever the topmost-first hit test finds, and the **nearest** widget on the way up from
//! there that declared one wins. A host turns the answer into whatever its window calls a cursor,
//! once, after each move — it keeps no list of what is draggable, resizable or a link.
//!
//! A widget whose cursor depends on **where** the pointer is (a terminal's link, a text run) answers
//! [`Component::cursor_over`](crate::component::Component::cursor_over) instead of declaring one.

use crate::component::Component;
use crate::pointer::hit_test;
use heca_core::layout::Point;

/// A cursor, by what it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    /// The ordinary arrow.
    #[default]
    Default,
    /// Something that can be activated, such as a link.
    Pointer,
    /// Something that can be picked up.
    Grab,
    /// Something being carried.
    Grabbing,
    /// Text that can be selected or edited.
    Text,
    /// An edge that resizes left and right.
    ResizeHorizontal,
    /// An edge that resizes up and down.
    ResizeVertical,
}

/// **The cursor for a pointer at `point`.**
///
/// While a drag is in flight it is [`Cursor::Grabbing`], whatever is under it. Otherwise the nearest
/// declared cursor on the path to the widget under the pointer; a widget that can be dragged and
/// declared nothing is [`Cursor::Grab`]; anything else is the default.
pub fn cursor_at(root: &dyn Component, point: Point) -> Cursor {
    if crate::pointer::dragging(root) {
        return Cursor::Grabbing;
    }
    let Some(path) = hit_test(root, point) else {
        return Cursor::Default;
    };
    let mut declared = None;
    let mut node = root;
    if let Some(cursor) = node.cursor_over(point).or(node.base().cursor) {
        declared = Some(cursor);
    }
    for step in path {
        let Some(child) = node.base().children.get(step) else {
            break;
        };
        node = child.as_ref();
        if let Some(cursor) = node.cursor_over(point).or(node.base().cursor) {
            declared = Some(cursor);
        }
    }
    match declared {
        Some(cursor) => cursor,
        None if crate::drag::source_at(root, point).is_some() => Cursor::Grab,
        None => Cursor::Default,
    }
}

#[cfg(test)]
mod tests;
