//! [`ToastCorner`] and [`ToastSpec`] — the plain data a host hands the
//! [`ToastStack`](super::ToastStack): where the stack sits, and what it should show.
//!
//! No callbacks and no widgets: a spec is a value the app can keep in its own list, compare,
//! de-duplicate and time out. The stack turns each one into a [`Toast`](super::Toast) and reports
//! interactions back by **id**, so the host never holds a widget handle.

use crate::widgets::{Glyph, ToastSeverity};

/// Which viewport corner the stack anchors to (and the direction it grows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastCorner {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[heca_grid_ui_macros::props]
impl ToastCorner {
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn is_right(self) -> bool {
        matches!(self, ToastCorner::TopRight | ToastCorner::BottomRight)
    }
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub(crate) fn is_top(self) -> bool {
        matches!(self, ToastCorner::TopRight | ToastCorner::TopLeft)
    }
}

/// A single notification the host wants shown — plain data (no callbacks). The
/// host owns these in a `Signal<Vec<ToastSpec>>`; the stack renders them.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastSpec {
    /// Stable identity — used to reconcile widgets across frames and reported
    /// back by `on_dismiss`/`on_action`.
    pub id: u64,
    pub severity: ToastSeverity,
    pub icon: Option<Glyph>,
    pub title: String,
    pub body: Option<String>,
    /// Inline action button label, if any.
    pub action: Option<String>,
    /// Whether the × dismiss affordance is shown.
    pub dismissible: bool,
}

impl ToastSpec {
    /// A new info spec with `id` + `title`. Chain the setters for the rest.
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            severity: ToastSeverity::Info,
            icon: None,
            title: title.into(),
            body: None,
            action: None,
            dismissible: true,
        }
    }
    #[heca_grid_ui_macros::prop]
    pub fn severity(mut self, s: ToastSeverity) -> Self {
        self.severity = s;
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, g: Glyph) -> Self {
        self.icon = Some(g);
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = Some(b.into());
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }
}

