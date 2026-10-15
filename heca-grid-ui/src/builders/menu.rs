//! **The menu a right-click opens on a widget.**

use crate::component::Component;
use super::IntoContextMenu;

/// **The menu a right-click opens on a widget** — a value, or a way to make one at the point.
pub trait MenuExt: Component + Sized {
    /// **Give this widget a context menu.** Right-click it — or anything inside it — and the menu
    /// opens at the cursor; trigger the host's `open_context_menu` while focus is here and it
    /// opens under the widget.
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
    ///
    /// let id = 7u64;
    /// let ctx = ContextMenu::new("pane-menu").child(
    ///     Menu::new("Pane", "What you can do with this pane")
    ///         .child(MenuItem::new().label("Rename").on_click(move || { let _ = id; }))
    ///         .child(MenuItem::new().label("Close").danger(true).on_click(move || { let _ = id; })),
    /// );
    ///
    /// // A value — and the same value again on the next row, because a menu is `Clone`.
    /// let row = Row::new().child(Label::new("nvim")).context_menu(ctx.clone());
    /// // …or a closure, when the rows must read state at the moment it opens.
    /// let other = Row::new().context_menu(move |_at| ctx.clone());
    /// ```
    ///
    /// Nothing else is needed: no row identity, no path string, no registered builder, no
    /// host-side hit test, no `Shift+F10` handling, and no anchor — the framework picks that from
    /// what triggered the menu. Universal, like `key`, so an `Icon` and a plugin's own widget
    /// carry one on the same terms as a `Row`. See [`crate::menu`] for bubbling and the host sink.
    ///
    /// **A value or a closure** — see [`IntoContextMenu`]. Either way the menu is realized when it
    /// is triggered, so a composed row's subtree is built fresh for each opening.
    fn context_menu(mut self, menu: impl IntoContextMenu + 'static) -> Self {
        self.base_mut().context_menu = Some(Box::new(move |at| menu.build(at)));
        self
    }
}

impl<T: Component + Sized> MenuExt for T {}
