//! **What a container holds**: one child or many through one builder, and a context menu as a
//! value or a way to make one.

use crate::component::Component;

/// Components that contain children.
/// **Anything that can be a child** — a widget, or a subtree someone else already built.
///
/// A `Box<dyn Component>` is not itself a [`Component`], which is the whole reason every
/// child-taking builder used to come in twos: `child` / `child_boxed`, `body` / `body_boxed`,
/// `leading` / `leading_boxed`, sixteen of them. A caller with a dynamically built subtree — what
/// the host's `realize()` returns from a `ViewNode`, or a chrome provider's render seam — had to
/// know which spelling to reach for, and a widget author had to remember to write both.
///
/// One bound covers both, so there is one builder per slot and nothing to remember. Implemented
/// for every `Component` and for `Box<dyn Component>`; nothing else needs an impl, and a caller
/// never names this trait.
pub trait IntoComponent {
    /// The subtree, boxed exactly once.
    fn into_component(self) -> Box<dyn Component>;
}

impl<C: Component + 'static> IntoComponent for C {
    fn into_component(self) -> Box<dyn Component> {
        Box::new(self)
    }
}

impl IntoComponent for Box<dyn Component> {
    /// Already boxed — handed straight through, so a realized subtree costs no second allocation.
    fn into_component(self) -> Box<dyn Component> {
        self
    }
}

/// **One child or many, through one builder.**
///
/// A widget, an already-boxed subtree, or a `Vec`/array of either. It exists so that every place
/// that takes children takes them in whichever shape the caller happens to hold — `.child(row)` and
/// `.child(rows)` are the same door, and nobody has to find out that a plural spelling exists.
///
/// This replaced a `child` / `children` pair. Two names for one idea is the shape this codebase
/// treats as a defect: the tests exercise one, the caller reaches for whichever name they saw
/// first, and the two drift.
pub trait IntoChildren {
    /// The subtree(s), each boxed exactly once.
    fn into_children(self) -> Vec<Box<dyn Component>>;
}

impl<C: Component + 'static> IntoChildren for C {
    fn into_children(self) -> Vec<Box<dyn Component>> {
        vec![Box::new(self)]
    }
}

impl IntoChildren for Box<dyn Component> {
    /// Already boxed — handed straight through, so a realized subtree costs no second allocation.
    fn into_children(self) -> Vec<Box<dyn Component>> {
        vec![self]
    }
}

impl<T: IntoComponent> IntoChildren for Vec<T> {
    fn into_children(self) -> Vec<Box<dyn Component>> {
        self.into_iter()
            .map(IntoComponent::into_component)
            .collect()
    }
}

impl<T: IntoComponent, const N: usize> IntoChildren for [T; N] {
    fn into_children(self) -> Vec<Box<dyn Component>> {
        self.into_iter()
            .map(IntoComponent::into_component)
            .collect()
    }
}

pub trait Parent: Component + Sized {
    /// **Append a child, or several** — a widget, an already-boxed subtree, or a `Vec`/array of
    /// them; see [`IntoChildren`].
    ///
    /// ```ignore
    /// Flex::column().child(header).child(body)
    /// Flex::column().child([header, body])
    /// DockFrame::new(name).child(ws.columns.iter().map(build).collect::<Vec<_>>())
    /// ```
    fn child(mut self, c: impl IntoChildren) -> Self {
        self.base_mut().children.extend(c.into_children());
        self
    }
}

/// **What [`MenuExt::context_menu`](super::MenuExt::context_menu) accepts: a menu, or a
/// way to make one.**
///
/// One method, two spellings, because both are honest ways to say the same thing:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
///
/// # fn build_menu(_: u64) -> ContextMenu { ContextMenu::new("m") }
/// let ctx = ContextMenu::new("pane").child(Menu::new("Pane", "…"));
/// Row::new().context_menu(ctx.clone());               // a value
/// Row::new().context_menu(move |_at| build_menu(7));  // a closure
/// ```
///
/// A [`ContextMenu`](crate::widgets::ContextMenu) is `Clone` — its content is plain data and `Rc`
/// closures — so the value form is a clone per opening, not a shared panel. Reach for the closure
/// when the menu's rows depend on state this widget's tree is not rebuilt on, or when building it
/// eagerly would be wasted work.
pub trait IntoContextMenu {
    /// Produce the menu to show, for a right-click at `at`. Called **each time** the menu is
    /// triggered.
    ///
    /// **The point is passed because a menu is always opened at one**, and some entries are answers
    /// about what is under it rather than about the widget as a whole — a terminal's *Open link*
    /// is the case that forced it: whether that entry exists depends on the exact cell clicked, so
    /// a builder that cannot see the point cannot decide. Without it the host had to work the menu
    /// out on the widget's behalf, which is how a pane's menu ended up hand-written in the mouse
    /// handler. Ignore it with `|_at|` when the menu does not vary.
    fn build(&self, at: heca_core::layout::Point) -> crate::widgets::ContextMenu;
}

impl IntoContextMenu for crate::widgets::ContextMenu {
    fn build(&self, _at: heca_core::layout::Point) -> crate::widgets::ContextMenu {
        self.clone()
    }
}

impl<F: Fn(heca_core::layout::Point) -> crate::widgets::ContextMenu> IntoContextMenu for F {
    fn build(&self, at: heca_core::layout::Point) -> crate::widgets::ContextMenu {
        self(at)
    }
}
