//! **One table between the event names a description binds and the events a native widget
//! listens to** — so a plugin's node reaches every event the native API has, by the same name, and
//! the two cannot drift apart: both functions match exhaustively, so a new event on either side
//! does not compile until it is placed here.

use heca_grid_ui::EventKind;
use heca_view::ViewEvent;

/// The native event a described name listens to. `None` for the names that are not one native
/// event — `press` is a click plus a pick, `change` is a value — which each widget realizes itself.
pub fn native(event: ViewEvent) -> Option<EventKind> {
    use EventKind as K;
    use ViewEvent as V;
    match event {
        V::DoubleClick => Some(K::DoubleClick),
        V::TripleClick => Some(K::TripleClick),
        V::RightClick => Some(K::RightClick),
        V::MiddleClick => Some(K::MiddleClick),
        V::PointerEnter => Some(K::PointerEnter),
        V::PointerLeave => Some(K::PointerLeave),
        V::FocusGained => Some(K::Focus),
        V::FocusLost => Some(K::Blur),
        V::Key => Some(K::Key),
        V::Scroll => Some(K::Scroll),
        V::DragStart => Some(K::DragStart),
        V::Drag => Some(K::Drag),
        V::DragEnd => Some(K::DragEnd),
        V::DragEnter => Some(K::DragEnter),
        V::DragOver => Some(K::DragOver),
        V::DragLeave => Some(K::DragLeave),
        V::Drop => Some(K::Drop),
        // `press` is a left click, Enter/Space on the focused control and the prefix+/ letter
        // together, so it is not one native event: each widget realizes it.
        V::Press
        | V::Hint
        | V::Change
        | V::Submit
        | V::Toggle
        | V::Dismiss
        | V::Action
        | V::Activate
        | V::Move
        | V::Resize => None,
    }
}

/// The described name of a native event a widget author listens to. `None` for the framework's
/// own plumbing, which no author binds: typed text, the raw pointer stream, mounting.
pub fn described(kind: EventKind) -> Option<ViewEvent> {
    use EventKind as K;
    use ViewEvent as V;
    match kind {
        K::DoubleClick => Some(V::DoubleClick),
        K::TripleClick => Some(V::TripleClick),
        K::RightClick => Some(V::RightClick),
        K::MiddleClick => Some(V::MiddleClick),
        K::PointerEnter => Some(V::PointerEnter),
        K::PointerLeave => Some(V::PointerLeave),
        K::Focus => Some(V::FocusGained),
        K::Blur => Some(V::FocusLost),
        K::Key => Some(V::Key),
        K::Scroll => Some(V::Scroll),
        K::DragStart => Some(V::DragStart),
        K::Drag => Some(V::Drag),
        K::DragEnd => Some(V::DragEnd),
        K::DragEnter => Some(V::DragEnter),
        K::DragOver => Some(V::DragOver),
        K::DragLeave => Some(V::DragLeave),
        K::Drop => Some(V::Drop),
        // A left click is bound as `press`, which is more than this one event. A drag held past
        // an edge (`EdgeScroll`) has no described name yet.
        K::Click
        | K::EdgeScroll
        | K::TextInput
        | K::FilesDropped
        | K::ModifiersChanged
        | K::Widget
        | K::Hint
        | K::PointerDown
        | K::PointerUp
        | K::PointerMove
        | K::PointerRecheck
        | K::PointerDownOutside
        | K::DragInFlight
        | K::DragSettled
        | K::Mount
        | K::Unmount => None,
    }
}

/// The native event a node's bound name listens to, if it is one.
pub(crate) fn native_by_name(name: &str) -> Option<EventKind> {
    ViewEvent::ALL
        .iter()
        .find(|event| event.name() == name)
        .and_then(|event| native(*event))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name that is a native event comes back to the same name: one table, read both ways.
    #[test]
    fn every_native_name_is_read_back_the_same_way() {
        for event in ViewEvent::ALL {
            if let Some(kind) = native(*event) {
                assert_eq!(described(kind), Some(*event), "{event:?} ↔ {kind:?}");
            }
        }
    }
}
