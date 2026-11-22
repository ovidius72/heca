//! **Who a widget is**: its name, the scope it names, and the props it takes.

use crate::component::Component;

/// **Who a widget is**: the name it is found by, the scope it gives the names inside it, and the
/// props its owner hands it.
pub trait IdentityExt: Component + Sized {
    /// **The name this widget is found by** — the cursor, the pick letter, a drag and a right-click
    /// all address it by this. Optional: a widget without one is named by what it reads as.
    ///
    /// Takes a name, or an `Option` of one: `None` leaves the widget as it was, so a row whose name
    /// may be missing writes `.key(name)` with no branch around it.
    fn key(mut self, key: impl IntoKey) -> Self {
        if let Some(key) = key.into_key() {
            self.base_mut().key = Some(key);
        }
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

/// **What `.key(..)` takes**: a name, or an `Option` of one (`None` names nothing).
///
/// Written out per type rather than for every `Into<String>`: the standard library could one day
/// let an `Option` turn into a `String`, and Rust refuses a rule that might then mean two things.
pub trait IntoKey {
    /// The name, or `None` for none.
    fn into_key(self) -> Option<String>;
}

macro_rules! into_key {
    ($($t:ty),*) => {$(
        impl IntoKey for $t {
            fn into_key(self) -> Option<String> { Some(self.into()) }
        }
        impl IntoKey for Option<$t> {
            fn into_key(self) -> Option<String> { self.map(Into::into) }
        }
    )*};
}

into_key!(
    &str,
    String,
    &String,
    std::borrow::Cow<'_, str>,
    Box<str>,
    char
);
