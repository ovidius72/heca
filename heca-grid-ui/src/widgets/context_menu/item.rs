use super::{ICON_GAP, ICON_SCALE, KEYCAP_INSET, ROW_PAD_X, ROW_PAD_Y, SHORTCUT_GAP};
use crate::Tone;
use crate::builders::{LayoutExt, Parent, ToneExt};
use crate::style::{Fill, WidgetSize};
use crate::widgets::{CursorLook, Glyph, Icon, Keycap, KeycapVariant, Label, Row};
use std::rc::Rc;

/// One row in a [`Menu`](super::Menu): what it shows, and what it does.
///
/// Two ways to say what it shows, and they are the same two every composed widget in this library
/// offers — **sugar or children, children win**:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{Glyph, Grid, Icon, Label, MenuItem};
///
/// // Sugar: the row nearly every menu wants.
/// let save = MenuItem::new().label("Save as…").icon(Glyph::File).on_click(|| {});
///
/// // Composed: any subtree, laid out by the engine like any other tree.
/// let close = MenuItem::new()
///     .child(|| Grid::new().child(Icon::new(Glyph::Trash)).child(Label::new("Close")))
///     .danger(true)
///     .on_click(|| {});
/// ```
///
/// [`child`](MenuItem::child) takes a **builder**, not a widget — see the module docs
/// for why. Everything else on the row ([`key`](MenuItem::key), [`shortcut`](MenuItem::shortcut),
/// [`danger`](MenuItem::danger), [`enabled`](MenuItem::enabled)) applies to both forms.
#[derive(Clone)]
pub struct MenuItem {
    pub(super) label: Option<String>,
    icon: Option<Glyph>,
    /// A builder for composed content. `Rc` so the item stays [`Clone`]; a `Fn` so it can run once
    /// per open.
    content: Option<Rc<dyn Fn() -> crate::WidgetModel>>,
    pub(super) key: Option<char>,
    pub(super) shortcut: Option<String>,
    pub(super) danger: bool,
    pub(super) enabled: bool,
    pub(super) on_select: Rc<dyn Fn()>,
}

/// The name this row had when only a host built menus, kept so existing callers still compile.
pub type MenuEntry = MenuItem;

#[heca_grid_ui_macros::props]
impl MenuItem {
    /// An empty row. Give it content with [`label`](MenuItem::label) /
    /// [`icon`](MenuItem::icon) or [`child`](MenuItem::child), and behaviour with
    /// [`on_click`](MenuItem::on_click).
    pub fn new() -> Self {
        Self {
            label: None,
            icon: None,
            content: None,
            key: None,
            shortcut: None,
            danger: false,
            enabled: true,
            on_select: Rc::new(|| {}),
        }
    }

    /// The row's text — the sugar form. Ignored if the row also has
    /// [`child`](MenuItem::child) content.
    #[heca_grid_ui_macros::prop]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A leading icon for the sugar form. Ignored if the row also has
    /// [`child`](MenuItem::child) content.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// **Compose the row out of widgets** instead of a label and an icon. Replaces the sugar.
    ///
    /// Takes a builder rather than a widget because a menu can be shown more than once and a
    /// subtree is owned — see the module docs.
    /// The subtree becomes a real child of the panel, so the layout engine sizes it exactly as it
    /// would anywhere else.
    #[heca_grid_ui_macros::host_only(
        "a subtree builder — composed content crosses as ViewNode children"
    )]
    pub fn child<C: crate::builders::IntoComponent + 'static>(
        mut self,
        f: impl Fn() -> C + 'static,
    ) -> Self {
        self.content = Some(Rc::new(move || f().into_component()));
        self
    }

    /// What this row does when chosen. Replaces whatever was there.
    ///
    /// A **plain closure**: this library knows nothing about actions, and an item that took an
    /// action id would tie every menu in it to one host's dispatch. Firing a catalogued action
    /// inside the closure is the author's choice — and the way to stay reachable from the command
    /// palette and RPC, which a closure alone is not.
    #[heca_grid_ui_macros::host_only("a closure — behaviour crosses a description as an Intent")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_select = Rc::new(f);
        self
    }

    /// A **quick-pick key** rendered as a [`KeyHint`](crate::widgets::KeyHint)-style keycap on
    /// the right; pressing it (case-insensitive) activates the entry immediately.
    #[heca_grid_ui_macros::host_only("unsupported argument type (char)")]
    pub fn key(mut self, key: char) -> Self {
        self.key = Some(key);
        self
    }

    /// An optional textual shortcut hint (e.g. `"prefix+x"`), drawn left of the
    /// quick-pick keycap. Informational only — not pressable inside the menu.
    #[heca_grid_ui_macros::prop]
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        self.shortcut = Some(hint.into());
        self
    }

    /// Mark this entry as **destructive** — its content renders in the `danger` hue.
    #[heca_grid_ui_macros::prop]
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Enable/disable the entry. A disabled entry is dimmed and cannot be selected.
    #[heca_grid_ui_macros::prop]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Default for MenuItem {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuItem {
    /// **Run what this row runs.** The row's own act, reachable without a pointer.
    ///
    /// The counterpart of [`label_text`](MenuItem::label_text): that reads a menu back, this drives
    /// it. A test asserting that a row *does* something needs it, and so does anything choosing an
    /// entry by keyboard, by quick-pick key or over RPC — otherwise "this row is wired up" is only
    /// checkable by clicking it, and a row wired to nothing looks exactly like a row wired to
    /// something — contributed rows once opened fine and did nothing at all.
    pub fn activate(&self) {
        (self.on_select)();
    }

    /// The row's text, for a caller (or a test) reading a menu back. `None` for a composed row,
    /// whose text lives in its subtree.
    pub fn label_text(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Width this row reserves on its right for the shortcut hint and the quick-pick keycap. The
    /// row's content is padded away from it so composed content never has to know they exist.
    /// **The row this item becomes** — a real [`Row`] the panel stacks: the item's content (its
    /// composed child, else its icon and label), then its shortcut and its quick-pick letter at the
    /// trailing edge, laid out by the engine. `chosen` says whether the menu's cursor is on it; the
    /// row wears a menu entry's look ([`CursorLook::Pill`]), reads in the danger colour when the
    /// item is destructive, and quiet when it is disabled.
    pub(super) fn build_row(&self, font: f32, chosen: impl Fn() -> bool + 'static) -> Row {
        let inner: crate::WidgetModel = match &self.content {
            Some(build) => build(),
            None => {
                let mut row = crate::widgets::container()
                    .direction("row")
                    .align("center")
                    .gap(ICON_GAP);
                if let Some(g) = self.icon {
                    row = row.child(Icon::new(g).size(font * ICON_SCALE));
                }
                if let Some(l) = &self.label {
                    row = row.child(Label::new(l));
                }
                crate::WidgetModel::new(row)
            }
        };
        // The content takes the room; what trails it keeps its own size at the far edge.
        let mut line = crate::widgets::container()
            .direction("row")
            .align("center")
            .gap(SHORTCUT_GAP as f32)
            .grow(1.0)
            .child(crate::widgets::container().grow(1.0).child(inner));
        if let Some(shortcut) = &self.shortcut {
            line =
                line.child(Label::new(shortcut).color(Fill::theme("muted").alpha("menu_shortcut")));
        }
        if let Some(key) = self.key {
            let quiet = (!self.enabled).then_some(Tone::Muted);
            line = line.child(
                Keycap::new(key.to_string())
                    .variant(KeycapVariant::Bordered)
                    .size(WidgetSize::Caption)
                    .tone(quiet),
            );
        }
        // What trails the content sits close to the edge; a plain row keeps its own air.
        let trailing = self.shortcut.is_some() || self.key.is_some();
        Row::new()
            .cursor_look(CursorLook::Pill)
            .nav_selected(chosen)
            .disabled(!self.enabled)
            .content_tone(self.danger.then_some(Tone::Danger))
            .padding_xy(ROW_PAD_X, ROW_PAD_Y)
            .padding_right(match trailing {
                true => KEYCAP_INSET as f32,
                false => ROW_PAD_X,
            })
            .child(line)
    }
}
