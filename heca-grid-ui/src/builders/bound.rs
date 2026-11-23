//! **Children that follow a value** — `.child(items.map(Row::new))`.
//!
//! A tree is built once and kept, so children made from data go stale when the data changes:
//! nothing builds them again. Handing `.child(..)` a list *mapped* to widgets, instead of the
//! widgets themselves, keeps them in step with the value: the container makes a child per item, and
//! when the value changes it keeps the children of the items still there — the same widgets, with
//! their focus, hover and state — makes the new ones, drops the gone ones and puts them in order.
//! It is the same door every child goes through, so the children sit directly in the container:
//! a `Flex` gap, a `Grid` track and a hint letter treat them like any other child.
//!
//! ```ignore
//! let rooms = signal(vec![Room::new("lobby")]);
//! Flex::column().gap(Spacing::Sm).child(rooms.map(RoomRow::new))
//! ```
//!
//! **An item is its own identity**: an item equal to one already shown keeps that child wherever it
//! moved; an item whose data changed is a new item, and its child is made again.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{IntoChildren, IntoComponent};
use crate::component::Base;
use crate::reactive::{Signal, SignalGet, create_effect};

/// Puts a range of a container's children in step with the latest items; `None` when nothing
/// changed since the last call.
type Place = dyn FnMut(&mut Vec<crate::WidgetModel>) -> Option<bool>;

/// **A list of items and how one is shown** — what `.child(..)` keeps in step with the value. Made
/// by [`MapChildren::map`] on a signal holding a list, or by a value's own `map` (a service's
/// window-side value has one); [`new`](Self::new) is the general form those are built on.
pub struct BoundChildren {
    dirty: Rc<Cell<bool>>,
    place: Box<Place>,
}

impl BoundChildren {
    /// **The children `make` builds from what `read` returns.** `read` is run again whenever a
    /// value it read changes, and the children follow in the next frame.
    pub fn new<T, C>(read: impl Fn() -> Vec<T> + 'static, make: impl Fn(&T) -> C + 'static) -> Self
    where
        T: Clone + PartialEq + 'static,
        C: IntoComponent,
    {
        let latest: Rc<RefCell<Option<Vec<T>>>> = Rc::new(RefCell::new(None));
        let dirty = Rc::new(Cell::new(false));
        {
            let (latest, dirty) = (latest.clone(), dirty.clone());
            create_effect(move |_| {
                *latest.borrow_mut() = Some(read());
                dirty.set(true);
                // A changed value is a frame to draw; without asking, the new children would wait
                // for whatever input comes next.
                crate::component::request_frame();
            });
        }
        let mut shown: Vec<T> = Vec::new();
        let place = move |children: &mut Vec<crate::WidgetModel>| {
            let items = latest.borrow_mut().take()?;
            let (changed, _) = crate::reconcile::reconcile_matching(
                children,
                &items,
                |i, item| shown[i] == *item,
                |item| make(item).into_component(),
            );
            shown = items;
            Some(changed)
        };
        Self {
            dirty,
            place: Box::new(place),
        }
    }
}

impl IntoChildren for BoundChildren {
    /// What a builder that takes a fixed list gets: nothing yet. The children arrive when the list
    /// is handed to a container's `.child(..)`, which keeps them in step.
    fn into_children(self) -> Vec<crate::WidgetModel> {
        Vec::new()
    }

    fn attach(self, base: &mut Base) {
        base.bound.push(Bound {
            start: base.children.len(),
            len: 0,
            dirty: self.dirty,
            place: self.place,
        });
    }
}

impl IntoChildren for Option<BoundChildren> {
    /// **An optional live list** — `.child(report.map(|signal| signal.map(build_line)))`. `None`
    /// attaches nothing, the same rule as an optional widget, so a component with a list it may
    /// not have never keeps an always-present empty signal just to have something to hand over.
    fn into_children(self) -> Vec<crate::WidgetModel> {
        Vec::new()
    }

    fn attach(self, base: &mut Base) {
        if let Some(bound) = self {
            bound.attach(base);
        }
    }
}

/// **`.map(make)` on a signal holding a list** — the children `make` builds, kept in step with it.
pub trait MapChildren<T> {
    /// A child per item, made by `make`, following the list as it changes.
    fn map<C: IntoComponent>(self, make: impl Fn(&T) -> C + 'static) -> BoundChildren;
}

impl<T: Clone + PartialEq + 'static> MapChildren<T> for Signal<Vec<T>> {
    fn map<C: IntoComponent>(self, make: impl Fn(&T) -> C + 'static) -> BoundChildren {
        BoundChildren::new(move || self.get(), make)
    }
}

/// One range of a container's children that follows a value. Held by the container's [`Base`].
pub(crate) struct Bound {
    /// Where the range begins among the container's children.
    start: usize,
    len: usize,
    dirty: Rc<Cell<bool>>,
    place: Box<Place>,
}

impl Bound {
    /// Whether the value changed since the children were last put in step.
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty.get()
    }
}

/// **Put every range of `base`'s children in step with its value** — run by the layout pass on
/// every node before it reads its children, so the frame in which a value changed also lays out
/// and draws it. A range that grows or shrinks moves the ranges after it.
pub(crate) fn apply(base: &mut Base) {
    let mut shift: isize = 0;
    for i in 0..base.bound.len() {
        let start = base.bound[i].start.saturating_add_signed(shift);
        base.bound[i].start = start;
        if !base.bound[i].dirty.replace(false) {
            continue;
        }
        let len = base.bound[i].len;
        let end = (start + len).min(base.children.len());
        let mut range: Vec<crate::WidgetModel> = base.children.drain(start..end).collect();
        let changed = (base.bound[i].place)(&mut range).unwrap_or(false);
        let new_len = range.len();
        base.children.splice(start..start, range);
        base.bound[i].len = new_len;
        shift += new_len as isize - len as isize;
        if changed {
            base.mark_needs_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{IdentityExt, Parent};
    use crate::component::Component;
    use crate::reactive::{SignalUpdate, signal};
    use crate::widgets::{Flex, Label};
    use crate::{LayoutEngine, Size};

    fn frame(root: &mut Flex) {
        LayoutEngine::new().compute(root, Size::new(300.0, 300.0));
    }

    fn keys(root: &Flex) -> Vec<String> {
        root.base()
            .children
            .iter()
            .map(|c| c.base().key.clone().unwrap_or_default())
            .collect()
    }

    fn mark_all(root: &mut Flex) {
        for child in root.base_mut().children.iter_mut() {
            child.base_mut().set_hidden(true);
        }
    }

    fn kept(root: &Flex, at: usize) -> bool {
        root.base().children[at].base().style.layout.hidden
    }

    fn items(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn row(name: &str) -> Label {
        Label::new(name).key(name)
    }

    /// **The children follow the value**: added, removed and moved, with every unchanged one kept.
    #[test]
    fn the_children_follow_the_value_and_unchanged_ones_are_kept() {
        let source = signal(items(&["a", "b", "c"]));
        let mut root = Flex::column().child(source.map(|name: &String| row(name)));
        frame(&mut root);
        assert_eq!(keys(&root), ["a", "b", "c"]);
        mark_all(&mut root);

        source.set(items(&["c", "a", "d"]));
        frame(&mut root);
        assert_eq!(
            keys(&root),
            ["c", "a", "d"],
            "b removed, d added, c moved first"
        );
        assert!(
            kept(&root, 0) && kept(&root, 1),
            "c and a are the same widgets, moved"
        );
        assert!(!kept(&root, 2), "d is new");
    }

    /// **An optional live list**: `Some` follows its value like any bound list, `None` attaches
    /// nothing — and the children around it are untouched either way.
    #[test]
    fn an_optional_live_list_is_a_child_or_nothing() {
        let source = signal(items(&["a"]));
        let present: Option<Signal<Vec<String>>> = Some(source);
        let absent: Option<Signal<Vec<String>>> = None;
        let mut root = Flex::column()
            .child(Label::new("head").key("head"))
            .child(present.map(|list| list.map(|name: &String| row(name))))
            .child(absent.map(|list| list.map(|name: &String| row(name))))
            .child(Label::new("foot").key("foot"));
        frame(&mut root);
        assert_eq!(keys(&root), ["head", "a", "foot"]);
        source.set(items(&["a", "b"]));
        frame(&mut root);
        assert_eq!(
            keys(&root),
            ["head", "a", "b", "foot"],
            "the present one still follows its value"
        );
    }

    /// **An item whose data changed is a new item.**
    #[test]
    fn a_changed_item_is_made_again() {
        let source = signal(items(&["a", "b"]));
        let mut root = Flex::column().child(source.map(|name: &String| row(name)));
        frame(&mut root);
        mark_all(&mut root);
        source.set(items(&["a", "b2"]));
        frame(&mut root);
        assert!(kept(&root, 0));
        assert!(!kept(&root, 1), "b2 is not b");
    }

    /// **The value's children sit among the container's own** — in the place they were added, with
    /// the children written before and after them left alone.
    #[test]
    fn the_children_sit_between_the_ones_written_around_them() {
        let source = signal(items(&["a"]));
        let mut root = Flex::column()
            .child(Label::new("head").key("head"))
            .child(source.map(|name: &String| row(name)))
            .child(Label::new("foot").key("foot"));
        frame(&mut root);
        assert_eq!(keys(&root), ["head", "a", "foot"]);
        source.set(items(&["a", "b", "c"]));
        frame(&mut root);
        assert_eq!(keys(&root), ["head", "a", "b", "c", "foot"]);
        source.set(Vec::new());
        frame(&mut root);
        assert_eq!(keys(&root), ["head", "foot"]);
    }

    /// **The very next frame shows the change** — the container asks for a layout pass itself, so
    /// nothing else has to happen first (a second click once did).
    #[test]
    fn the_next_frame_asks_for_layout_and_shows_the_change() {
        let source = signal(items(&["a"]));
        let mut root = Flex::column().child(source.map(|name: &String| row(name)));
        frame(&mut root);
        assert!(!crate::component::needs_layout(&root));
        source.set(items(&["a", "b"]));
        assert!(
            crate::component::needs_layout(&root),
            "a changed value asks for a layout pass"
        );
        frame(&mut root);
        assert_eq!(keys(&root), ["a", "b"]);
    }
}
