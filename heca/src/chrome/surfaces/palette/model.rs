//! Palette rows and the component that owns an action.

use super::*;

/// One row of the palette: what it shows, and what it runs.
///
/// Assembled in [`entries`] from the catalog and resolved once at open time rather than per
/// keystroke — the widget re-filters the list on every character, and re-deriving which placement
/// owns an action inside that loop would make the answer depend on how fast someone types.
#[derive(Debug, Clone)]
pub(crate) struct PaletteEntry {
    /// The entry's identity for the overlay round-trip, unique across **every** mode — an action's
    /// name, or `"pane:3"` / `"ws:0"`. It is not what frecency counts; see
    /// [`rank_id`](Self::rank_id).
    pub(crate) id: String,
    /// What past use is counted against, when the entry has a durable identity at all.
    ///
    /// Separate from [`id`](Self::id) because the two answer different questions, and for actions
    /// they happened to be the same string. A `PaneId` restarts at 1 every launch and a workspace
    /// index is reused, so counting either would attach yesterday's habits to a **different** pane
    /// or workspace — the name is the only part that means the same thing tomorrow. `None` for an
    /// unnamed workspace, which has nothing durable to be: the row still matches and sorts, it
    /// simply carries no boost.
    pub(crate) rank_id: Option<String>,
    /// The pane this row stands for, when it is a pane row — the handle used to keep its label
    /// following the pane's live name while the palette is open.
    pub(crate) pane: Option<heca_core::layout::PaneId>,
    /// Is this the pane/workspace the user is already on? Drawn in the accent, so a list of
    /// near-identical names still says where you are standing before you pick where to go.
    pub(crate) current: bool,
    /// Should the selection start here? The pane/workspace you were **last** in — so Enter takes
    /// you back where you just were, instead of re-focusing what is already focused.
    pub(crate) preselect: bool,
    /// What the user reads: the action's label, prefixed with its component's title when it belongs
    /// to one (`"Workspaces › Delete Row"`).
    pub(crate) label: String,
    /// The action's description — the muted second line.
    pub(crate) description: String,
    pub(crate) icon: Option<Glyph>,
    /// Every binding of the action, already display-formatted (`"λ ⇧ e"`), one per row of keycaps —
    /// empty when unbound. The list, not a joined line: an action bound twice shows two rows.
    pub(crate) shortcuts: Vec<String>,
    /// What choosing it dispatches. A built-in is a plain `View` intent; a component's action is
    /// aimed at the placement that owns it, focus first.
    pub(crate) intent: InteractionIntent,
}

/// What is known about the component owning an action, at the moment the palette opens.
///
/// Resolved from the host by [`owners`] so [`entries`] stays a pure function of its inputs — it can
/// be tested without an `AppState`, which cannot be built without a window.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OwnerInfo {
    /// The mount the action would land on — `owning_mount`'s answer, the one rule shared with the
    /// `perform` bridge and RPC.
    pub(crate) mount: String,
    /// The component's own title, as it names itself. The palette never invents a scope name.
    pub(crate) title: String,
    /// Does this mount currently hold chrome focus? Its actions sort first.
    pub(crate) focused: bool,
}
