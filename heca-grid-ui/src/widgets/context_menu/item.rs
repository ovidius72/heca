use super::{ICON_GAP, KEYCAP_FONT_SCALE, KEYCAP_INSET, ROW_PAD_X, ROW_PAD_Y, SHORTCUT_GAP};
use crate::builders::{LayoutExt, Parent};
use crate::component::Component;
use crate::widgets::key_hint::keycap_size;
use crate::widgets::{Glyph, Icon, Label};
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
    content: Option<Rc<dyn Fn() -> Box<dyn Component>>>,
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
    pub fn child<C: Component + 'static>(mut self, f: impl Fn() -> C + 'static) -> Self {
        self.content = Some(Rc::new(move || Box::new(f()) as Box<dyn Component>));
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
    fn trailing_width(&self, font: f32) -> f64 {
        let adv = (font * crate::font::MONO_ADVANCE_RATIO) as f64;
        let mut w = 0.0;
        if let Some(s) = &self.shortcut {
            w += SHORTCUT_GAP + s.chars().count() as f64 * adv;
        }
        if let Some(k) = self.key {
            w += SHORTCUT_GAP
                + keycap_size(font * KEYCAP_FONT_SCALE, &k.to_string()).w
                + KEYCAP_INSET;
        }
        w
    }

    /// Build this row's widget subtree: the composed content if there is any, else the sugar row.
    ///
    /// **Children win over sugar** — the same precedence [`Button`](crate::widgets::Button) has. The row box
    /// itself carries the padding and the trailing reservation, so composed content is laid out in
    /// the space that is actually free.
    pub(super) fn build_row(&self, font: f32) -> Box<dyn Component> {
        let inner: Box<dyn Component> = match &self.content {
            Some(build) => build(),
            None => {
                let mut row = crate::widgets::container()
                    .direction("row")
                    .align("center")
                    .gap(ICON_GAP);
                if let Some(g) = self.icon {
                    row = row.child(Icon::new(g).size(font * 1.05));
                }
                if let Some(l) = &self.label {
                    row = row.child(Label::new(l));
                }
                Box::new(row)
            }
        };
        Box::new(
            crate::widgets::container()
                .direction("row")
                .align("center")
                .padding_xy(ROW_PAD_X, ROW_PAD_Y)
                .padding_right(ROW_PAD_X + self.trailing_width(font) as f32)
                .child(inner),
        )
    }
}
