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

    /// **While this widget holds the keyboard, the keyboard acts on another item** — the one whose
    /// `key` `item` returns now, as the DOM's `aria-activedescendant`. A search field that keeps the
    /// focus while the arrows move a selection through a list beside it says so here, and the
    /// keyboard's menu action (heca's `prefix+>`) opens the selected row's menu.
    ///
    /// ```ignore
    /// Input::new().value(query).active_descendant(move || selected.get().map(row_key))
    /// ```
    fn active_descendant(mut self, item: impl Fn() -> Option<String> + 'static) -> Self {
        self.base_mut().active_descendant = Some(std::rc::Rc::new(item));
        self
    }
}

impl<T: Component + Sized> FocusExt for T {}
