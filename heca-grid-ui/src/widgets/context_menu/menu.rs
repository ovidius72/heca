use super::MenuItem;

/// **A titled list of [`MenuItem`]s.** Content, and nothing else: it does not know what opens it,
/// where it appears, or which key summons it — those belong to whatever *presents* it.
///
/// Build it where the data is, so its items simply capture what they act on:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
///
/// let id = 7u64;
/// let menu = Menu::new("Pane", "What you can do with this pane")
///     .child(MenuItem::new().label("Rename").on_click(move || { let _ = id; }))
///     .child(MenuItem::new().label("Close").danger(true).on_click(move || { let _ = id; }));
///
/// let row = Row::new().child(Label::new("nvim")).context_menu(ContextMenu::new("pane").child(menu));
/// ```
///
/// **One `Menu` per [`ContextMenu`](super::ContextMenu), several items in it.** A context menu does not hold sections:
/// two lists in one panel would make the entries mean different targets in the same place.
#[derive(Clone, Default)]
pub struct Menu {
    title: String,
    description: String,
    name: Option<String>,
    pub(super) items: Vec<MenuItem>,
}

impl Menu {
    /// A menu titled `title`, described by `description`.
    ///
    /// The title heads the panel — and would be the strip label in a menu bar, which is why it
    /// lives on the content rather than on a presenter. The description is what makes a menu
    /// self-documenting instead of needing a declaration somewhere else.
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            name: None,
            items: Vec::new(),
        }
    }

    /// Add a row.
    pub fn child(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// **Optional. Give this menu a name so other components can add rows to it.**
    ///
    /// The single thing left of the contribution design: a named menu is one a host can offer to
    /// everything mounted before it is shown, so a plugin's "Open in container" can appear on a row
    /// it does not own. A menu without a name is closed, and needs nothing.
    ///
    /// A contributed row has **no per-row payload**, so it acts on app state (the focused pane, the
    /// selected row) rather than on the row the menu was opened for. Opaque here — this library
    /// neither parses the name nor knows who answers it.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// The menu's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// What this menu is for, in a sentence.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The name other components may add rows to, if this menu has one.
    pub fn declared_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// **Take this menu's rows.** For a caller merging two menus into one: rows built by the same
    /// conversion everything else goes through, moved onto another menu rather than rebuilt by hand
    /// beside it — which is how a second, subtly different way of wiring a row gets written.
    pub fn into_items(self) -> Vec<MenuItem> {
        self.items
    }

    /// The rows' labels, in order — what a caller (or a test) reads back without reaching into the
    /// items. A composed row has no label of its own and reads back as `None`.
    pub fn item_labels(&self) -> Vec<Option<String>> {
        self.items.iter().map(|i| i.label.clone()).collect()
    }

    /// How many rows this menu has.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this menu has no rows at all.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Append rows a host collected from elsewhere (contributions to a [named](Menu::name) menu).
    pub fn extend(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items.extend(items);
        self
    }
}
