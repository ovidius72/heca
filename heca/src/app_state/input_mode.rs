//! What the keyboard is doing: the input mode, what it is picking, and what a pick acts on.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarItemState {
    /// Currently active (focused workspace / pane).
    Active,
    /// Was visited previously this session (last focused before current).
    Visited,
    /// Not visited this session.
    None,
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenameTarget {
    Workspace(usize),
    Column { ws_idx: usize, col_idx: usize },
    Pane(PaneId),
}


impl RenameTarget {
    /// The catalogued action that renames this kind of thing — whose label titles the rename
    /// dialog, so the dialog says what the menu entry and the palette already say.
    pub fn action_name(self) -> &'static str {
        match self {
            RenameTarget::Pane(_) => "rename_pane",
            RenameTarget::Column { .. } => "rename_column",
            RenameTarget::Workspace(_) => "rename_workspace",
        }
    }
}


/// **Where a picked pane goes** — a column that exists, or one that does not yet.
///
/// A new column is a destination like any other, so it is named here rather than signalled by a
/// magic id: the pick offers it a letter beside the real columns, and pressing that letter runs the
/// action that makes one. Without it "put this somewhere new" was a separate binding you had to
/// know, while the pick was already on screen asking where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnPickTarget {
    /// A column already in the strip.
    Existing {
        ws_idx: usize,
        col_idx: usize,
        col_id: heca_core::layout::ColumnId,
    },
    /// **A new column, right of the one the pane is in now** — "you keep your place in the strip",
    /// the same rule `move_pane_to_new_column` already follows.
    New,
}


#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Prefix,
    /// Quick-select: each visible pane is assigned a letter; next keypress selects it.
    PaneSelect {
        candidates: Vec<(char, PaneId)>,
    },
    /// Quick-swap: each visible pane is assigned a letter; next keypress swaps with it.
    /// `focus_after` determines whether focus follows the swapped pane.
    PaneSwap {
        candidates: Vec<(char, PaneId)>,
        focus_after: bool,
    },
    /// Sidebar navigation: keyboard navigation within the sidebar tree.
    /// Chord sequence: multi-key binding (e.g. prefix → w → 1).
    /// `sequence` holds the keys pressed so far (after prefix).
    ///
    /// Partially wired: render and input handling exist, but no command path
    /// constructs this variant yet. See handle_chord_mode() in app/input.rs.
    #[expect(
        dead_code,
        reason = "Reserved for multi-key chord UX; will be constructed when chord entry is implemented."
    )]
    Chord {
        sequence: Vec<String>,
    },
    /// Custom mode (e.g. resize mode). Stay in mode until Esc.
    /// `name` is the mode identifier from config.
    Mode {
        name: String,
    },
    /// Host-owned selection mode. Entered via `WmAction::EnterSelectionMode`.
    /// The actual selection data lifecycle (begin/update_focus/end) is driven
    /// by mouse/UI/RPC adapters and `WmAction::ClearSelection`; this mode
    /// represents the input state where selection semantics are active. Esc
    /// clears the selection and returns to `Normal`; Enter confirms the
    /// selection and returns to `Normal`. Other keys are ignored.
    Selection,
    /// Status-bar marker while a destructive-confirm modal is up. The prompt itself is a
    /// host-owned overlay [`Dialog`](heca_grid_ui::Dialog) layer (see
    /// [`handlers::request_destructive`](crate::handlers::request_destructive)) which owns
    /// input and carries the action/resume in its completion — this variant just drives the
    /// "CONFIRM" status word.
    ConfirmDelete,
    /// Take-pane letter selection mode.
    /// User picks a pane which gets moved to the active column bottom.
    PaneTake {
        candidates: Vec<(char, PaneId)>,
        focus_after: bool,
    },
    /// Move-to-workspace letter pick: each workspace is assigned a letter (shown as a
    /// universal `KeyHint` over its sidebar dock); the next keypress moves `target`
    /// (the active column or pane, captured on entry) into that workspace.
    WorkspacePick {
        /// `(letter, ws_idx, ws_id)` — the position the action acts on, and the identity the
        /// letter is offered against.
        candidates: Vec<(char, usize, heca_core::layout::WorkspaceId)>,
        target: WorkspacePickTarget,
    },
    /// Move-to-column letter pick: each column (across all workspaces) is assigned a
    /// letter (shown as a `KeyHint` over its sidebar column); the next keypress moves
    /// the active pane into that `(ws_idx, col_idx)` column (stacking with its panes).
    ColumnPick {
        /// `(letter, where it goes)` — see [`InputMode::WorkspacePick`].
        candidates: Vec<(char, ColumnPickTarget)>,
        pane_id: PaneId,
    },
    /// Follow-link letter pick: each visible terminal hyperlink across **all
    /// visible panes** is assigned a letter (a keycap drawn over the link's first
    /// cell); the next keypress opens that link via [`WmAction::OpenLink`]. Entered
    /// with `prefix+Shift+o`. Each candidate carries its own `pane_id`.
    FollowLink {
        candidates: Vec<LinkHint>,
    },
    /// Scrollback-search query entry (entered with `/` in selection mode). Typing
    /// edits `AppState.search`'s query and re-runs the search live; Enter keeps the
    /// matches (so `n`/`N` navigate in selection mode), Esc cancels. The query +
    /// matches live in [`SearchState`], not here.
    Search,
    /// Dock (chrome container) letter pick, entered with a bare `focus_dock`: every dock on
    /// screen gets a letter (a `KeyHint` keycap over its body) and the next keypress gives it
    /// chrome **keyboard focus**. Candidates carry a **container id**, so the pick is
    /// position-agnostic — a dock is picked wherever it is seated (F003/P011/T020).
    DockPick {
        candidates: Vec<(char, crate::chrome::ContainerId)>,
    },
    /// Universal leader/vimium **picker** (entered with `prefix+/`): every region that said what a
    /// pick does to it gets a letter (a keycap stamped over its bounds), and the next keypress runs
    /// that region's own declaration. Each candidate carries a
    /// [`HintTarget`](crate::chrome::HintTarget) — the tree it lives in and its path in it, valid
    /// for exactly as long as the letters are up. Any other key / Esc exits.
    HintPick {
        candidates: Vec<(char, crate::chrome::HintTarget)>,
    },
}


/// What a [`InputMode::WorkspacePick`] moves into the picked workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspacePickTarget {
    /// A column, captured by its **full address** (`ws_idx` + `col_idx`) at pick entry,
    /// so resolution is correct even if the active workspace drifts mid-pick. For
    /// `MoveColumnToWorkspace`.
    Column { ws_idx: usize, col_idx: usize },
    /// A specific pane (by stable id), for `MovePaneToWorkspace`.
    Pane(PaneId),
}


impl InputMode {
    pub fn candidates(&self) -> Option<&[(char, PaneId)]> {
        match self {
            InputMode::PaneSelect { candidates }
            | InputMode::PaneSwap { candidates, .. }
            | InputMode::PaneTake { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// **Is a letter picker up, waiting for one keystroke?**
    ///
    /// The family, named once. Every one of these modes ends on the next key — picked, wrong key,
    /// or Esc — which means a modifier being held would end it too: reaching for Shift to type a
    /// capital is *part of* pressing that letter, and the picker would vanish before the letter
    /// arrived.
    ///
    /// **Exhaustive on purpose — no wildcard.** Adding an `InputMode` variant does not compile until
    /// it is classified here, which is the same technique `action_policy()` uses for `WmAction` and
    /// for the same reason. Asking the question per handler first put the guard in three of eight
    /// modes; writing it as a `matches!` list then left `DockPick` out on the day it was written,
    /// and nothing failed — a wildcard answers `false` for whatever nobody listed, silently. A
    /// compile error is the only form of this rule that cannot be forgotten.
    pub fn awaits_pick_letter(&self) -> bool {
        match self {
            // A letter is on screen and the next keystroke chooses one.
            InputMode::PaneSelect { .. }
            | InputMode::PaneSwap { .. }
            | InputMode::PaneTake { .. }
            | InputMode::WorkspacePick { .. }
            | InputMode::ColumnPick { .. }
            | InputMode::DockPick { .. }
            | InputMode::FollowLink { .. }
            | InputMode::HintPick { .. } => true,
            // Everything else reads keys for something other than picking a letter, or reads none.
            InputMode::Normal
            | InputMode::Prefix
            | InputMode::Chord { .. }
            | InputMode::Mode { .. }
            | InputMode::Selection
            | InputMode::ConfirmDelete
            | InputMode::Search => false,
        }
    }

    /// Workspace pick candidates (letter → `ws_idx`) while a `WorkspacePick` is active.
    pub fn ws_candidates(&self) -> Option<&[(char, usize, heca_core::layout::WorkspaceId)]> {
        match self {
            InputMode::WorkspacePick { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// Column pick candidates (letter → where the pane goes) while a `ColumnPick` is active.
    pub fn col_candidates(&self) -> Option<&[(char, ColumnPickTarget)]> {
        match self {
            InputMode::ColumnPick { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// **How many things this pick has to offer** — `None` for a mode that is not a pick.
    ///
    /// **Exhaustive on purpose — no wildcard**, for the reason
    /// [`awaits_pick_letter`](Self::awaits_pick_letter) is: a new pick variant that nobody
    /// classified would answer "not a pick" silently, and its refusal would go back to being
    /// invisible.
    pub fn pick_candidate_count(&self) -> Option<usize> {
        match self {
            InputMode::PaneSelect { candidates }
            | InputMode::PaneSwap { candidates, .. }
            | InputMode::PaneTake { candidates, .. } => Some(candidates.len()),
            InputMode::WorkspacePick { candidates, .. } => Some(candidates.len()),
            InputMode::ColumnPick { candidates, .. } => Some(candidates.len()),
            InputMode::DockPick { candidates } => Some(candidates.len()),
            InputMode::FollowLink { candidates } => Some(candidates.len()),
            InputMode::HintPick { candidates } => Some(candidates.len()),
            InputMode::Normal
            | InputMode::Prefix
            | InputMode::Chord { .. }
            | InputMode::Mode { .. }
            | InputMode::Selection
            | InputMode::ConfirmDelete
            | InputMode::Search => None,
        }
    }

    /// Dock pick candidates (letter → container id) while a `DockPick` is active.
    pub fn dock_candidates(&self) -> Option<&[(char, crate::chrome::ContainerId)]> {
        match self {
            InputMode::DockPick { candidates } => Some(candidates),
            _ => None,
        }
    }

    /// The keyboard pick currently in progress (move / select / swap / take), if any —
    /// a structured description of the pending action. Mirrored into the reactive chrome
    /// store (and emitted as `PendingPickChanged`) so any component or plugin can react
    /// to it (e.g. render its own prompt overlay). `None` when no pick is active.
    pub fn pending_pick(&self, catalog: &crate::actions::ActionCatalog) -> Option<PendingPick> {
        // Map the active pick mode to its enter-mode action; the human prompt + label
        // come from that action's `ActionDescriptor` (the catalog is the single source
        // of truth for action text — no duplicated strings here).
        let (kind, action_name) = match self {
            InputMode::PaneSelect { .. } => (PickKind::SelectPane, "pane_select"),
            InputMode::PaneSwap {
                focus_after: true, ..
            } => (PickKind::SwapPane, "swap_and_focus_pane"),
            InputMode::PaneSwap {
                focus_after: false, ..
            } => (PickKind::SwapPane, "swap_pane"),
            InputMode::PaneTake {
                focus_after: true, ..
            } => (PickKind::TakePane, "pane_take_and_focus"),
            InputMode::PaneTake {
                focus_after: false, ..
            } => (PickKind::TakePane, "pane_take"),
            InputMode::WorkspacePick {
                target: WorkspacePickTarget::Pane(_),
                ..
            } => (PickKind::MovePaneToWorkspace, "move_pane_to_workspace_pick"),
            InputMode::WorkspacePick {
                target: WorkspacePickTarget::Column { .. },
                ..
            } => (
                PickKind::MoveColumnToWorkspace,
                "move_column_to_workspace_pick",
            ),
            InputMode::ColumnPick { .. } => {
                (PickKind::MovePaneToColumn, "move_pane_to_column_pick")
            }
            InputMode::DockPick { .. } => (PickKind::FocusDock, "focus_dock"),
            _ => return None,
        };
        let meta = catalog.find(action_name)?;
        Some(PendingPick {
            kind,
            action_name,
            label: meta.label.clone(),
            prompt: meta.description.clone(),
        })
    }
}


/// A keyboard pick (target-selection) currently in progress. Exposed reactively so
/// components/plugins can render their own UI for the pending action.
///
/// The text is **owned**, not `&'static str`: it is sourced from the action's
/// [`ActionMeta`](crate::actions::ActionMeta) in the runtime catalog, which holds owned metadata so
/// that a plugin-registered action can live there too. `Clone`, not `Copy`, for the same reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPick {
    /// Stable machine-readable kind (match on this in plugins).
    pub kind: PickKind,
    /// The enter-mode action's **config name** string (e.g. `"move_pane_to_column_pick"`) —
    /// not a `WmAction` value.
    pub action_name: &'static str,
    /// The action's command-palette label (from its `ActionMeta`).
    pub label: String,
    /// The action's description, used as the pick prompt (from its `ActionMeta`).
    pub prompt: String,
}


/// The kind of in-progress keyboard pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    SelectPane,
    SwapPane,
    TakePane,
    MovePaneToWorkspace,
    MoveColumnToWorkspace,
    MovePaneToColumn,
    /// Pick a chrome container to give keyboard focus to.
    FocusDock,
}


impl PickKind {
    /// **What this pick offers, as a word** — so a refusal can say what there was none of without
    /// each pick carrying its own sentence. The label and prompt still come from the
    /// [`ActionCatalog`](crate::actions::ActionCatalog); this is the one thing the catalog does not
    /// know, because it describes what an action *does* rather than what it picks among.
    pub fn subject(self) -> &'static str {
        match self {
            PickKind::SelectPane | PickKind::SwapPane | PickKind::TakePane => "pane",
            PickKind::MovePaneToWorkspace | PickKind::MoveColumnToWorkspace => "workspace",
            PickKind::MovePaneToColumn => "column",
            PickKind::FocusDock => "dock",
        }
    }
}

