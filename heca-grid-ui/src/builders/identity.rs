//! **Who a widget is**: its name, the scope it names, and the props it takes.

use crate::component::Component;

/// **Who a widget is**: the name it is found by, the scope it gives the names inside it, and the
/// props its owner hands it.
pub trait IdentityExt: Component + Sized {
    fn key(mut self, key: impl Into<String>) -> Self {
        self.base_mut().key = Some(key.into());
        self
    }


    /// Name this subtree as an enclosing **scope** — a panel, a dock, a tab group.
    ///
    /// Stamp it on the wrapper around a region and a press anywhere inside it resolves back to that
    /// region, including a press a widget consumes (a scrollbar thumb is still *inside* the panel
    /// holding it). That is what lets "click a panel to focus it" work for every panel with nothing
    /// declared per panel. See [`crate::focus::page_scope`].
    fn scope_key(mut self, id: impl Into<String>) -> Self {
        self.base_mut().scope_key = Some(id.into());
        self
    }


    /// **Take new props of type `T`** from whoever owns this widget: `f` runs with each value
    /// [`set_props`](crate::component::Component::set_props) hands over, and with the widget's own
    /// [`Base`](crate::component::Base) — so it can place and reconcile its own children. The facts
    /// a widget shows arrive here, so its owner never reaches inside it.
    fn on_props<T: std::any::Any>(
        mut self,
        f: impl FnMut(&T, &mut crate::component::Base) + 'static,
    ) -> Self {
        self.base_mut().props = Some(crate::props::PropsSlot::of(f));
        self
    }
}

impl<T: Component + Sized> IdentityExt for T {}
