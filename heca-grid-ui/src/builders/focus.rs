//! **Where the keyboard goes, declared on the widget** — the builders over
//! [`Base::follow_focus`](crate::component::Base::follow_focus) and its modal twin, so nobody reaches
//! into `base_mut()` to say it.

use crate::component::Component;
use crate::reactive::Signal;

/// Focus builders, on every widget.
pub trait FocusExt: Component + Sized {
    /// **Take the keyboard when `source` turns true, let it go when it turns false** — a search
    /// field that is focused as its bar opens, a dock the host says holds the keyboard. Only a
    /// change moves focus: a click elsewhere while `source` stays true is left alone.
    ///
    /// ```ignore
    /// Input::new().value(query).follow_focus(open)
    /// ```
    fn follow_focus(mut self, source: Signal<bool>) -> Self {
        self.base_mut().follow_focus(source);
        self
    }

    /// **The same, for a surface over the page** — a dialog, a menu, a palette: when `source` turns
    /// false the keyboard goes back to whoever held it when the surface opened.
    fn follow_focus_modal(mut self, source: Signal<bool>) -> Self {
        self.base_mut().follow_focus_modal(source);
        self
    }

    /// **While this widget holds the keyboard, the keyboard acts on another item** — the widget
    /// `item` names now, as the DOM's `aria-activedescendant`. A search field that keeps the focus
    /// while the arrows move a selection through a list beside it hands it the list's selected row,
    /// and the keyboard's menu action (heca's `prefix+>`) opens that row's menu. No `key` is needed:
    /// the list names its row by the row's own identity.
    ///
    /// ```ignore
    /// let list = List::new(items);
    /// Input::new().value(query).active_descendant(list.selected_item())
    /// ```
    fn active_descendant(mut self, item: impl Fn() -> Option<u64> + 'static) -> Self {
        self.base_mut().active_descendant = Some(std::rc::Rc::new(item));
        self
    }
}

impl<T: Component + Sized> FocusExt for T {}
