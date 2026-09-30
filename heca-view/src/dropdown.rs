//! One entry of a dropdown or context menu.

use crate::{Intent, ViewOrder};

/// One entry of a dropdown / context menu. The author supplies id/label/action; the host resolves
/// the icon from the action registry (`ActionCatalog::icon`) and wires the intent + quick-pick —
/// the same centralized path as [`ModalAction`], with **no hand-picked glyph and no `prefix+X`
/// label** (the leader doesn't work while the menu is open; a host-assigned single-letter quick-pick
/// that *does* work replaces it).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DropdownItem {
    /// Stable id returned in [`ModalResult::Action`]; also the **catalog name** the icon and label
    /// resolve from — the entry's visual identity (e.g. `"close"`).
    ///
    /// It is deliberately **not** the same thing as what the entry runs: a sidebar "Close pane"
    /// entry has id `close` (so it shows the close icon) but dispatches `close_pane_by_id` with the
    /// row's pane. Identity and behaviour are separate fields.
    pub id: String,
    pub label: String,
    /// What the entry dispatches when chosen: an [`Intent`] — an action **name + args** — routed
    /// through the one dispatch door, so the interaction policy and the confirm gate apply exactly
    /// as they would for a keypress.
    ///
    /// An `Intent` rather than a `WmAction` because `WmAction` is a **closed enum**: a plugin cannot
    /// add a variant, so a menu entry carrying one could only ever run actions heca already has —
    /// which is precisely what blocked plugin-contributed menus (context-menu-5). A name resolves to
    /// a built-in *or* to a plugin's own registered action, indifferently.
    pub intent: Intent,
    pub danger: bool,
    pub enabled: bool,
    /// **Where this entry sits among all the others** — CSS `order`, the same idea and the same
    /// spelling as `.order(..)` on any widget — or `None` to sit where its block sits.
    ///
    /// A menu is filled by several sources at once — heca's own entries and any plugin's — and each
    /// source declares an order for its whole block. That is the right granularity most of the
    /// time: a plugin thinks in "my entries". It is not enough when one entry belongs at the very
    /// top and the rest belong at the bottom, because a block can only move whole.
    ///
    /// So an entry may say where it goes, and **one that says nothing takes its block's order**.
    /// There is a single ordering rule rather than "sort the blocks, then sort inside them": every
    /// entry has an order, most simply do not spell it, and the whole menu is one sorted list.
    ///
    /// A number or a list, lower first, so an entry can be slotted *between* two neighbours without
    /// renumbering either — `[1, 1, 1]` lands between `[1, 1]` and `[1, 2]`. Ties keep the order the
    /// entries were produced in.
    ///
    /// It was called `weight`, a second word for the same idea — and in the widget library `weight`
    /// means font weight. `"weight"` is still read on the wire so no existing plugin breaks.
    #[serde(default, alias = "weight")]
    pub order: Option<ViewOrder>,
}

impl DropdownItem {
    /// An enabled, non-destructive entry whose id is also the action it runs (the common case: the
    /// entry's catalog identity and its behaviour coincide, e.g. `zoom_column`).
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let intent = Intent::new(id.clone());
        Self::with_intent(id, label, intent)
    }

    /// An entry whose behaviour differs from its visual identity — the id keeps the icon/label
    /// (`close`), while the intent carries the action actually run, with its args
    /// (`close_pane_by_id` + `pane_id`).
    pub fn with_intent(id: impl Into<String>, label: impl Into<String>, intent: Intent) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            intent,
            danger: false,
            enabled: true,
            order: None,
        }
    }
    /// Tint destructive (red) — the confirm gate still applies on dispatch.
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Enable/disable (a disabled entry is dimmed + unselectable).
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// **Put this entry somewhere other than where its block sits** — see
    /// [`order`](DropdownItem::order).
    ///
    /// ```ignore
    /// DropdownItem::new("myplugin.pin", "Pin this").order(-1)       // above every built-in
    /// DropdownItem::new("myplugin.copy", "Copy path").order([0, 5]) // between two built-ins
    /// ```
    pub fn order(mut self, order: impl Into<ViewOrder>) -> Self {
        self.order = Some(order.into());
        self
    }
}
