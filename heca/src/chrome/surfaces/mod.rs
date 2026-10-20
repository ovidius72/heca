//! **Surfaces** — what is drawn over the panes: the layer registry, overlays, the command palette,
//! context menus, the exposé and the notification stack.

pub(crate) mod context_menu;
pub(super) mod expose;
pub(super) mod glue;
pub(super) mod layers;
pub(super) mod notifications;
pub(super) mod overlay;
pub(super) mod palette;
