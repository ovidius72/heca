//! **What kind of act each action is** — `ActionPolicy`, and the one exhaustive table that classifies every action.

use crate::input::WmAction;


/// Policy classification for a WM action.
///
/// This determines how the action behaves under different [`Domain`]s. Callers should use
/// `dispatch_action()` or `route_interaction()` and never check policy directly — it is `pub`
/// only because a component **declares** one on every action it registers
/// (`ActionMeta::policy`), and a plugin outside this crate must be able to name what it is
/// required to declare (F003/P086/T371 step 6; the plugin-facing crate itself is F003/P017/T9).
///
/// Its wire name (RPC introspection) is the variant in `snake_case` — `TiledOnly` is `tiled_only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionPolicy {
    /// True global app action — always allowed in EVERY focus domain, including
    /// Floating. Reserved for actions with no tiled/floating layout impact
    /// (e.g. `ReloadConfig`). Distinct from [`AlwaysAllowed`](Self::AlwaysAllowed),
    /// which the router blocks when floating.
    Global,
    /// Always allowed regardless of focus domain.
    AlwaysAllowed,
    /// Only meaningful in Tiled domain — blocked when Floating.
    TiledOnly,
    /// Operates on the focused pane regardless of domain (close, rename, float/unfloat).
    FocusedPaneLocal,
    /// Affects workspace structure — blocked when Floating by current policy.
    /// If that product rule changes, update this classification together with
    /// the routing tests; do not rely on an implicit future relaxation.
    WorkspaceLevel,
    /// Policy depends on the interaction source.
    SourceDependent,
    /// Only while **that component's container holds the keyboard** — what a component's
    /// selection-dependent actions actually mean (F003/P086/T371).
    ///
    /// "Delete the row my cursor is on" is not a request anyone should be able to make from the
    /// command palette or over RPC while the dock is not being driven; it only had no gate because
    /// its *keys* live in a layer consulted only when the dock is focused, which is a property of
    /// key routing, not a declared rule. This is the rule.
    ContainerFocused,
}

/// Classify a WM action into its policy category.
pub(crate) fn action_policy(action: &WmAction) -> ActionPolicy {
    match action {
        // ── Tiled-only: blocked when Floating ──
        WmAction::FocusLeft
        | WmAction::FocusRight
        | WmAction::FocusUp
        | WmAction::FocusDown
        | WmAction::NextPane
        | WmAction::PrevPane
        | WmAction::FocusToggleLocal
        | WmAction::FocusToggleGlobal
        | WmAction::SplitHorizontal
        | WmAction::SplitVertical
        | WmAction::ZoomColumn
        | WmAction::ScrollViewLeft
        | WmAction::ScrollViewRight
        | WmAction::ResizeIncrease
        | WmAction::ResizeDecrease
        | WmAction::PaneHeightIncrease
        | WmAction::PaneHeightDecrease
        | WmAction::SwapLeft
        | WmAction::SwapRight
        | WmAction::SwapUp
        | WmAction::SwapDown
        | WmAction::MovePaneLeft { .. }
        | WmAction::MovePaneRight { .. }
        | WmAction::MoveColumnUp
        | WmAction::MoveColumnDown
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::MovePaneToWorkspace { .. }
        | WmAction::MovePaneToColumn { .. }
        | WmAction::PlacePane { .. }
        | WmAction::MoveColumnToWorkspace { .. }
        | WmAction::MoveColumn { .. }
        | WmAction::SwapColumns { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeColumnBy { .. }
        | WmAction::ResizePaneHeightBy { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::RenameColumn
        | WmAction::RenameColumnByIdx { .. }
        | WmAction::RenameColumnTo { .. }
        | WmAction::DeleteColumn { .. }
        | WmAction::ZoomColumnAtIndex { .. }
        | WmAction::DeleteCurrentColumn
        | WmAction::AddPaneToColumn { .. }
        | WmAction::AddColumnToWorkspace { .. }
        // Sidebar toggles: blocked when Floating
        | WmAction::SidebarLeft
        // **Taking** chrome focus, likewise. This was `Global`, justified as "unlike `SidebarFocus`,
        // which enters a nav mode that moves pane focus". That stopped being true when
        // `sidebar_focus` was retired: a focused container's own `activate`/`hint` move pane focus,
        // and they are reached through this (F003/P085/T356). It also matches the two lines below —
        // a sidebar cannot even be toggled while floating, so being able to focus and drive one was
        // the stranger half.
        //
        // It costs a dock that merely scrolls, which would be harmless during a float. Telling the
        // two apart needs to know whether *this* dock is `keyboard_navigable`, and `policy_allows`
        // only receives the session — the widening tracked as F003/P086/T371.
        | WmAction::FocusDock { .. }
        | WmAction::ToggleDock { .. }
        | WmAction::SidebarRight
        | WmAction::CollapseCurrentWorkspace
        | WmAction::ExpandCurrentWorkspace
        | WmAction::ToggleCurrentWorkspaceCollapsed
        | WmAction::CollapseCurrentColumn
        | WmAction::ExpandCurrentColumn
        | WmAction::ToggleCurrentColumnCollapsed
        // Pane select/swap/take overlays: blocked when Floating
        | WmAction::PaneSelect
        | WmAction::SwapPane
        | WmAction::SwapAndFocusPane
        | WmAction::PaneTake
        | WmAction::PaneTakeAndFocus
        | WmAction::TakePane { .. }
        // Move-to-workspace overlays (active column / pane → picked workspace)
        | WmAction::MoveColumnToWorkspacePick
        | WmAction::MovePaneToWorkspacePick
        // Move-to-column overlay (active pane → picked column)
        | WmAction::MovePaneToColumnPick
        | WmAction::MovePaneToNewColumn
        // FloatAt: spawns new floating pane — blocked when already floating
        | WmAction::FloatAt { .. } => ActionPolicy::TiledOnly,

        // ── Focused-pane-local: allowed in both domains ──
        WmAction::Float
        | WmAction::ClosePane
        | WmAction::ClosePaneById { .. }
        | WmAction::RenamePane
        | WmAction::RenamePaneById { .. }
        | WmAction::ResetPaneName
        | WmAction::ResetPaneNameById { .. }
        | WmAction::RenameTarget { .. }
        // Scrollback operates on the focused pane and is allowed in both
        // tiled and floating domains.
        | WmAction::ScrollbackPageUp
        | WmAction::ScrollbackPageDown
        | WmAction::ScrollbackLineUp { .. }
        | WmAction::ScrollbackLineDown { .. }
        | WmAction::ScrollbackToTop
        | WmAction::ScrollbackToBottom
        | WmAction::ExitScrollback
        // Direct scroll (no selection / caret — same policy)
        | WmAction::ScrollLineUp
        | WmAction::ScrollLineDown
        | WmAction::ScrollPageUp
        | WmAction::ScrollPageDown
        | WmAction::ScrollToTop
        | WmAction::ScrollToBottom
        | WmAction::ScrollToOffset { .. }
        // Selection acts on the focused pane and is allowed in both
        // tiled and floating domains.
        | WmAction::EnterSelectionMode
        | WmAction::SelectionLeft
        | WmAction::SelectionRight
        | WmAction::SelectionUp
        | WmAction::SelectionDown
        | WmAction::ClearSelection
        | WmAction::CopySelection
        | WmAction::PasteClipboard
        | WmAction::BeginSelection
        | WmAction::ToggleSelectionEndpoint
        | WmAction::OpenLinkAtCaret
        | WmAction::SearchScrollback
        | WmAction::SearchNextMatch
        | WmAction::SearchPrevMatch
        // Per-pane font zoom operates on the focused (or specified) pane with no
        // layout impact — allowed in both tiled and floating domains.
        | WmAction::PaneTerminalFontZoom { .. }
        // Typing into a terminal, or ending it, acts on one pane and moves nothing else.
        | WmAction::TerminalRun { .. }
        | WmAction::TerminalKill { .. } => ActionPolicy::FocusedPaneLocal,

        // ── Workspace-level: blocked when Floating ──
        WmAction::WorkspaceNext
        | WmAction::WorkspacePrev
        | WmAction::FocusWorkspace { .. }
        | WmAction::CreateWorkspace
        | WmAction::RenameWorkspace
        | WmAction::RenameWorkspaceByIdx { .. }
        | WmAction::RenameWorkspaceTo { .. }
        | WmAction::ResetWorkspaceName
        | WmAction::ResetWorkspaceNameByIdx { .. }
        | WmAction::DeleteWorkspace { .. } => ActionPolicy::WorkspaceLevel,

        // ── Always-allowed: work regardless of domain (but blocked when Floating) ──
        WmAction::CommandPalette { .. }
        | WmAction::SpawnCommand { .. }
        => ActionPolicy::AlwaysAllowed,

        // ── Global: true app-level action, allowed even when Floating ──
        // ReloadConfig reloads config from disk — no tiled/floating layout impact,
        // so it must stay reachable while a floating pane is active (hot-reload).
        WmAction::ReloadConfig => ActionPolicy::Global,
        // Dismissing/picking a notification touches no layout and no pane — a toast can be up
        // over a floating pane just as easily as a tiled one, and must stay reachable (F009).
        WmAction::NotificationDismissOne { .. }
        | WmAction::NotificationDismissAll
        | WmAction::NotificationDismissLast
        | WmAction::NotificationPick
        | WmAction::NotificationActionRelay { .. } => ActionPolicy::Global,
        // Forgetting a search memory touches no layout and no pane, so there is no domain in which
        // it should be refused.
        WmAction::ClearSearchHistory { .. } | WmAction::ClearSearchRanking { .. } => {
            ActionPolicy::Global
        }
        // Opening a URL launches the OS handler — no layout impact, must work from
        // any focus domain (a link in a floating pane opens too).
        WmAction::OpenLink { .. } => ActionPolicy::Global,
        // Follow-link overlay targets the focused terminal's links; no layout
        // impact, so it stays reachable from any focus domain (incl. floating).
        WmAction::FollowLink => ActionPolicy::Global,
        // Entering the universal hint picker is a harmless overlay; the chosen
        // target's intent is separately policy-checked when it dispatches.
        WmAction::HintPick => ActionPolicy::Global,
        // **Asking a surface for its menu is surface-agnostic** — it acts on whichever context is
        // live, so there is no domain in which it should be refused (F003/P082/T416). It was
        // `FocusedPaneLocal`, which was true of it while a pane was the only thing that had a menu;
        // once a context surface can own the keyboard, that classification would refuse the exposé
        // its own menu while permitting the pane's behind it. Bubbling already decides *whose* menu
        // opens: it stops at the nearest declaration and nothing declared means nothing opens
        // (AGENTS, the menu model). The entries chosen from it keep their own policies.
        WmAction::OpenContextMenu => ActionPolicy::Global,
        // App-wide terminal font zoom changes only font metrics/PTY reflow — no
        // tiled/floating layout impact, so it must work in any focus domain.
        WmAction::AppFontZoom { .. } => ActionPolicy::Global,
        // Chrome container placement acts on chrome regions, independent of the
        // pane tiled/floating domain, so it stays reachable in any focus domain.
        WmAction::MoveContainerToRegion { .. }
        | WmAction::ReorderContainerBefore { .. }
        | WmAction::ReorderContainerAfter { .. }
        | WmAction::SetRegionVisible { .. } => ActionPolicy::Global,
        // **Releasing** chrome focus is always allowed, in every domain. It is the way back to the
        // main region, and a way out that can be blocked is not a way out — the same reason `Esc`
        // is a guarantee rather than a default (F003/P086/T363).
        // **The container-cursor family** — permitted only while a dock is being driven
        // (`Domain::Container`), which is what keeps "put the cursor on that row" out of the
        // command palette and RPC while nobody is in a dock. A click on a row emits
        // [`WmAction::FocusDock`] first, so the domain is `Container` by the time this runs.
        // **Moving a container's cursor is the container's own state, not an act on anything.**
        // It was `ContainerFocused`, which exists to stop "delete the row my cursor is on" being
        // asked from the palette or over RPC while the dock is not being driven. This is not that
        // kind of request: it changes no pane, no column and no layout — it only records which row
        // the cursor is on, so there is nothing for a focus domain to protect.
        //
        // The gate had a real cost. Clicking a sidebar row focuses the *pane*, so by the time the
        // cursor move was judged the container no longer held the keyboard and it was refused —
        // the click focused the pane but left the cursor behind, and arrowing afterwards resumed
        // from wherever it had been. A row should behave like a file manager's: click it, then go
        // up and down from there.
        //
        // `Global`, not `AlwaysAllowed`: a floating pane is no reason to refuse moving a dock's
        // cursor, and `AlwaysAllowed` is refused while something covers the content.
        WmAction::CursorTo { .. } => ActionPolicy::Global,
        WmAction::UnfocusDock => ActionPolicy::Global,
        // Horizontal scroll reaches a chrome container's scroll area only — chrome state, no pane
        // layout impact — so it stays reachable while a floating pane is active, unlike the vertical
        // four which also drive the focused pane's scrollback.
        WmAction::ScrollPageLeft
        | WmAction::ScrollPageRight
        | WmAction::ScrollToLeftEdge
        | WmAction::ScrollToRightEdge => ActionPolicy::Global,
        // Overlay control (§2.7.2): classified Global for match completeness, but never
        // actually consulted — `dispatch_intent` intercepts these before routing (they carry
        // an overlay id and resolve the `OverlayHost`, not a focus-domain-sensitive action).
        // **Overlay and layer control is `Global`, not `AlwaysAllowed`.** `AlwaysAllowed` is
        // refused while something covers the content — so an `AlwaysAllowed` close would be blocked
        // by the very overlay it exists to close, which is exactly what happened to the exposé
        // ("blocked intent from Keyboard" on Esc). Showing/hiding a layer is app-level control, not
        // an act on the panes, so it is allowed in every domain.
        WmAction::SubmitOverlay { .. }
        | WmAction::CloseOverlay { .. }
        | WmAction::ShowLayer { .. }
        | WmAction::HideLayer { .. }
        | WmAction::ToggleLayer { .. } => ActionPolicy::Global,

        // ── Source-dependent: may be allowed from some sources ──
        WmAction::FocusPane { .. } => ActionPolicy::SourceDependent,
    }
}

/// Whether `action` is permitted while the focused pane is in the **floating**
/// domain — the same rule the router applies (`FocusedPaneLocal` + `Global` pass;
/// `TiledOnly` / `WorkspaceLevel` / `AlwaysAllowed` are blocked when floating).
///
/// Used by the pane info-bar to hide buttons that don't apply to a floating pane
/// (split / zoom / move are `TiledOnly`; float / close are `FocusedPaneLocal`), so
/// the visible set follows the policy rather than a hardcoded list.
pub(crate) fn action_allowed_when_floating(action: &WmAction) -> bool {
    matches!(
        action_policy(action),
        ActionPolicy::FocusedPaneLocal | ActionPolicy::Global
    )
}
