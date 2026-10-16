//! Context-menu resolution: a dotted [`ContextPath`] + opaque [`ContextTarget`] resolve to a
//! merged dropdown via the [`ContextMenuRegistry`]. The host seeds the content pane; components and
//! plugins attach through `Provider::context_menus`.
//!
//! Both the mouse right-click (explicit target from the hit-test) and the keyboard
//! `OpenContextMenu` (implicit target via `resolve_active_context`) funnel through
//! [`open_context_menu_for`]. The host — not the handler — decides which menu applies for a given
//! context, so a plugin only declares *where* (`context_path`) and *what* (`build`); it never
//! decides *when* to open.
//!
//! Nothing is restored when a menu closes: a container's keyboard focus is not a mode, an overlay
//! never takes it away, and it is still there afterwards.
//!
//! | | owns |
//! |---|---|
//! | this module | what a menu is opened on: [`ContextPath`], [`ContextTarget`] |
//! | [`registry`](mod@registry) | who contributes entries to a path, and in what order |
//! | [`open`](mod@open) | opening a menu, and turning a declarer's identity into a target |
//! | [`entries`](mod@entries) | the vocabulary every menu writes its entries with |
//! | [`pane_menu`](mod@pane_menu) | the content pane's own menu — the app's domain |

mod entries;
mod open;
mod pane_menu;
mod registry;

#[cfg(test)]
mod tests;

use heca_core::layout::PaneId;

pub(crate) use entries::{item_running, menu_from_items};
pub(crate) use open::{open_context_menu_for, resolve_active_context, target_from_subject};
pub use registry::MenuBuild;
pub(crate) use registry::{ContextMenuRegistry, plugin_providers_for};

/// Dotted context identifier — the registry key. Built-in paths are constants; plugins use
/// arbitrary dotted strings (e.g. `"docker.container"`). Compared as a plain string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextPath(pub String);

impl ContextPath {
    /// A content pane (right-click on a pane, or keyboard `OpenContextMenu` from Normal).
    pub const PANE: &'static str = "pane";
}

/// What the menu was opened on. The host fills it from the hit-test (mouse) or from the focused
/// container's cursor (keyboard); providers match the arm they handle and return `vec![]` for any
/// other.
///
/// **Two arms, and the second one is generic.** A row is named the way its
/// component named it — the container it is in and the `key` the row declared — and nothing
/// else. It used to carry three workspace-shaped variants filled with facts the *host* had
/// resolved (a pane's column, a workspace's custom name), which is why a Docker row could not be
/// right-clicked at all: there was no variant for it and no way to add one without the host
/// learning what Docker is. The component that wrote the key is the one that reads it back, against
/// its own model, so those facts are its own to look up.
#[derive(Clone, Debug)]
pub enum ContextTarget {
    /// A content pane. `hyperlink` is `Some(url)` only for the mouse path (the clicked cell);
    /// a keyboard-opened menu has `None` (no target cell) so no "Open link" entry is added.
    Pane {
        pane_id: PaneId,
        hyperlink: Option<String>,
    },
    /// A **contribution to a named menu** — a row another component is adding to a menu it does
    /// not own ([`Menu::name`](heca_grid_ui::widgets::Menu::name)).
    ///
    /// It carries nothing, and that is the decision, not an omission: since a menu is declared on
    /// the widget it belongs to, the host never resolves *which row* was clicked,
    /// so it has no row payload to hand on. A contributed entry acts on **app state** — the focused
    /// pane, the selected row — rather than on the row the menu was opened for. The menu's name is
    /// passed beside this, as the `path`.
    ///
    /// It used to be `Row { container, key }`, filled by the host from a hit-test. Both fields
    /// became unreadable the moment rows started declaring their own menus, and were being passed
    /// an empty string.
    Contribution,
}
