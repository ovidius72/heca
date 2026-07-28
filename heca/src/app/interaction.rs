//! Central interaction policy layer.
//!
//! Every user-initiated action that changes WM state must flow through
//! `dispatch_action()` — the single chokepoint that decides whether an
//! interaction is allowed based on the current focus domain, input mode,
//! and interaction source.
//!
//! # Architecture
//!
//! ```text
//! User input → InteractionIntent → route_interaction() → RouteDecision
//!                                                        ├─ Allow(intent) → registry.execute()
//!                                                        └─ Block          → no-op
//! ```
//!
//! For keyboard actions:
//! ```text
//! KeyCombo → WmAction → dispatch_action(state, Keyboard, &action)
//! ```
//!
//! For mouse/sidebar actions:
//! ```text
//! Click/Drag → InteractionIntent::FocusPane { .. } → dispatch_action(state, MouseContent, &WmAction)
//! ```
//!
//! Handler-to-handler calls bypass the router and use `registry.execute()` directly.
//!
//! # Floating domain policy
//!
//! When `FocusDomain::Floating` is active, only `FocusedPaneLocal` actions
//! (Float/Unfloat, ClosePane, RenamePane) are allowed. Everything else is
//! blocked — tiled layout actions, sidebar, workspace switching, command
//! palette, mouse drag, and pane selection overlays.
//!
//! The only escape from floating is `prefix+f` (Float toggle) or closing the
//! floating pane (ClosePane).

use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use crate::keymap::ActionRef;
use heca_core::layout::{FocusDomain, PaneId};

// ═══════════════════════════════════════════════════════════════════════════
// Interaction source
// ═══════════════════════════════════════════════════════════════════════════

/// Where did the interaction come from?
///
/// Different sources may have different policy for the same action.
/// Example: a keyboard `FocusLeft` is blocked when floating, but a
/// future top-menu-bar "Focus Left" button might be allowed even while
/// floating if it explicitly refocuses the tiled domain first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionSource {
    /// Keyboard shortcut (prefix mode, global binding, mode binding).
    Keyboard,
    /// Mouse click or drag in the content area (tiled panes).
    MouseContent,
    /// Mouse click or drag in the left sidebar.
    MouseLeftSidebar,
    // Future sources — not implemented yet:
    // MouseRightSidebar,
    // MouseTopMenu,
    // MouseStatusBar,
    // Rpc,
}

// ═══════════════════════════════════════════════════════════════════════════
// Interaction intent
// ═══════════════════════════════════════════════════════════════════════════

/// What the interaction is trying to do.
///
/// Keyboard actions are wrapped in `ActivateAction`. Mouse and sidebar
/// interactions use specific intent variants because they carry semantic
/// information that a raw `WmAction` wouldn't capture (e.g., sidebar drag
/// start has no `WmAction` equivalent).
///
/// Intent variants are dispatched in `dispatch_action()`:
/// - `FocusPane` → `WmAction::FocusPane`
/// - `FocusWorkspace` → `WmAction::FocusWorkspace`
/// - `EnterSidebarNav` → `WmAction::SidebarFocus`
/// - `ToggleWorkspaceCollapsed` → `handlers::apply_ws_collapse`
/// - `StartSidebarDrag` → mouse-layer drag (no registry dispatch)
#[derive(Debug, Clone)]
pub(crate) enum InteractionIntent {
    /// A keyboard shortcut resolved to a WM action.
    ActivateAction(WmAction),
    /// Focus a specific pane, then run an action on it — the single-intent form of
    /// what an **active-targeted** pane button does on click (focus first so the
    /// action lands on the clicked pane, not whatever was active). Used by the pane
    /// header's KeyHint targets for `zoom`/`float`, whose `WmAction` acts on the
    /// focused pane and carries no pane id of its own. `dispatch_intent` expands it
    /// into a `FocusPane` then the action, each policy-routed on its own.
    FocusPaneThenAction {
        pane_id: PaneId,
        action: Box<WmAction>,
    },
    /// Focus a specific pane (from sidebar click, content click, or RPC).
    ///
    /// Dispatched to `WmAction::FocusPane` in `dispatch_action`.
    FocusPane { pane_id: PaneId },
    /// Focus a specific workspace (from a sidebar click or the hint picker).
    ///
    /// Dispatched to `WmAction::FocusWorkspace` in `dispatch_action`.
    FocusWorkspace { ws_idx: usize },
    /// Enter sidebar navigation mode (from keyboard shortcut or click).
    ///
    /// Dispatched to `WmAction::SidebarFocus` in `dispatch_action`.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "constructed from mouse/sidebar in future wiring pass")
    )]
    EnterSidebarNav,
    /// Toggle a specific workspace header's collapsed state from the sidebar.
    ToggleWorkspaceCollapsed { ws_idx: usize },
    /// Start dragging a sidebar item (no WmAction equivalent).
    ///
    /// Policy-routed only — the drag itself is initiated in the mouse layer.
    #[expect(dead_code, reason = "constructed from mouse/sidebar in future wiring pass")]
    StartSidebarDrag {
        pane_id: PaneId,
    },
    /// A declarative [`ViewNode`](crate::chrome::ViewNode) intent — the universal
    /// invocation currency for click / KeyHint / RPC / plugin (plan §2.7.2, "everything
    /// is an action"). Carries a `view::Intent { action, args }`; `dispatch_intent`
    /// resolves it via [`dispatch_view_intent`] (name → `WmAction` → policy-routed
    /// dispatch). Constructed by [`realize`](crate::chrome::realize) for actionable nodes.
    View(ViewIntent),
}

// ═══════════════════════════════════════════════════════════════════════════
// Route decision
// ═══════════════════════════════════════════════════════════════════════════

/// Decision returned by the interaction router.
///
/// `Allow(intent)` carries the intent forward so the router can
/// transform it in the future (e.g., retarget a focus change).
/// `Block` silently discards the interaction — no state change occurs.
#[derive(Debug, Clone)]
pub(crate) enum RouteDecision {
    Allow(InteractionIntent),
    Block,
}

// ═══════════════════════════════════════════════════════════════════════════
// Action policy (private — only used internally by the router)
// ═══════════════════════════════════════════════════════════════════════════

/// Policy classification for a WM action.
///
/// This determines how the action behaves under different focus domains.
/// It is **private** to the interaction module — callers should use
/// `dispatch_action()` or `route_interaction()` and never check policy
/// directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionPolicy {
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
        | WmAction::MoveColumnToWorkspace { .. }
        | WmAction::MoveColumn { .. }
        | WmAction::SwapColumns { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeColumnBy { .. }
        | WmAction::ResizePaneHeightBy { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::RenameColumn
        | WmAction::RenameColumnByIdx { .. }
        | WmAction::DeleteColumn { .. }
        | WmAction::DeleteCurrentColumn
        | WmAction::AddPaneToColumn { .. }
        | WmAction::AddColumnToWorkspace { .. }
        // Sidebar actions: blocked when Floating
        | WmAction::SidebarLeft
        | WmAction::SidebarRight
        | WmAction::SidebarFocus
        | WmAction::SidebarUp
        | WmAction::SidebarDown
        | WmAction::SidebarLeftNav
        | WmAction::SidebarRightNav
        | WmAction::SidebarPeek
        | WmAction::SidebarExpandToggle
        | WmAction::SidebarCreateWorkspace
        | WmAction::SidebarCreateColumn
        | WmAction::SidebarSplitInColumn
        | WmAction::SidebarZoomSelectedColumn
        | WmAction::SidebarDeleteSelected
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
        // OpenContextMenu operates on the focused pane (mouse: the clicked one; keyboard:
        // the focused one) and is allowed in both tiled and floating domains — a floating pane
        // still has a context menu. Individual menu entries (Split/Zoom/etc.) keep their own
        // policy when chosen; opening the menu is pane-local.
        | WmAction::OpenContextMenu
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
        | WmAction::PaneTerminalFontZoom { .. } => ActionPolicy::FocusedPaneLocal,

        // ── Workspace-level: blocked when Floating ──
        WmAction::WorkspaceNext
        | WmAction::WorkspacePrev
        | WmAction::FocusWorkspace { .. }
        | WmAction::CreateWorkspace
        | WmAction::RenameWorkspace
        | WmAction::RenameWorkspaceByIdx { .. }
        | WmAction::ResetWorkspaceName
        | WmAction::ResetWorkspaceNameByIdx { .. }
        | WmAction::DeleteWorkspace { .. } => ActionPolicy::WorkspaceLevel,

        // ── Always-allowed: work regardless of domain (but blocked when Floating) ──
        WmAction::CommandPalette
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. } => ActionPolicy::AlwaysAllowed,

        // ── Global: true app-level action, allowed even when Floating ──
        // ReloadConfig reloads config from disk — no tiled/floating layout impact,
        // so it must stay reachable while a floating pane is active (hot-reload).
        WmAction::ReloadConfig => ActionPolicy::Global,
        // Opening a URL launches the OS handler — no layout impact, must work from
        // any focus domain (a link in a floating pane opens too).
        WmAction::OpenLink { .. } => ActionPolicy::Global,
        // Follow-link overlay targets the focused terminal's links; no layout
        // impact, so it stays reachable from any focus domain (incl. floating).
        WmAction::FollowLink => ActionPolicy::Global,
        // Entering the universal hint picker is a harmless overlay; the chosen
        // target's intent is separately policy-checked when it dispatches.
        WmAction::HintPick => ActionPolicy::Global,
        // App-wide terminal font zoom changes only font metrics/PTY reflow — no
        // tiled/floating layout impact, so it must work in any focus domain.
        WmAction::AppFontZoom { .. } => ActionPolicy::Global,
        // Chrome container placement acts on chrome regions, independent of the
        // pane tiled/floating domain, so it stays reachable in any focus domain.
        WmAction::MoveContainerToRegion { .. }
        | WmAction::ReorderContainerBefore { .. }
        | WmAction::ReorderContainerAfter { .. }
        | WmAction::SetRegionVisible { .. } => ActionPolicy::Global,
        // Chrome keyboard focus is chrome state too: it decides which dock the scroll keys reach and
        // has no effect on the pane layout, so — unlike `SidebarFocus`, which enters a nav mode that
        // moves pane focus — it stays reachable while a floating pane is active.
        WmAction::FocusDock { .. } => ActionPolicy::Global,
        // Overlay control (§2.7.2): classified Global for match completeness, but never
        // actually consulted — `dispatch_intent` intercepts these before routing (they carry
        // an overlay id and resolve the `OverlayHost`, not a focus-domain-sensitive action).
        WmAction::SubmitOverlay { .. } | WmAction::CloseOverlay { .. } => ActionPolicy::Global,
        // Chrome shell region show/hide (sidebar-fu-6): acts on chrome geometry,
        // independent of the pane tiled/floating domain — reachable from any focus.
        WmAction::ShowLeftSidebar
        | WmAction::HideLeftSidebar
        | WmAction::ToggleLeftSidebar
        | WmAction::ShowRightSidebar
        | WmAction::HideRightSidebar
        | WmAction::ToggleRightSidebar
        | WmAction::ShowTopBar
        | WmAction::HideTopBar
        | WmAction::ToggleTopBar
        | WmAction::ShowBottomBar
        | WmAction::HideBottomBar
        | WmAction::ToggleBottomBar => ActionPolicy::Global,

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

// ═══════════════════════════════════════════════════════════════════════════
// Route interaction
// ═══════════════════════════════════════════════════════════════════════════

/// Decide whether an interaction is allowed under the current state.
///
/// Two policy layers, outermost first:
/// 1. **Overlay-capture policy** — while a modal overlay (a confirm `Dialog`, a context menu /
///    dropdown) owns input, every interaction is **Blocked** here. This is what stops a stray
///    keybinding / mouse / RPC (e.g. `prefix+e`, `prefix+>`, focus/split) from driving the app
///    behind an open dialog. Overlay *control* (`SubmitOverlay` / `CloseOverlay`) never reaches
///    here — it is intercepted in [`dispatch_intent`] before routing — and the overlay's own
///    Esc / Space / Enter / nav reach the widget through the widget-keymap path, not the WM action
///    system. This is state-level (which overlay is open), so it lives here rather than in the
///    session-only [`route_interaction_for_session`].
/// 2. **Focus-domain policy** — delegated to [`route_interaction_for_session`] with
///    `state.session`: when `FocusDomain::Floating` is active, only `FocusedPaneLocal` actions
///    are allowed from keyboard/mouse sources.
pub(crate) fn route_interaction(
    state: &AppState,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    if crate::chrome::top_modal(state).is_some() {
        return RouteDecision::Block;
    }
    route_interaction_for_session(&state.session, source, intent)
}

/// Session-only routing logic, extracted for testability.
///
/// This is the core policy function. `route_interaction()` delegates here,
/// passing `state.session`. Tests call this directly.
///
/// # Floating domain policy
///
/// When `FocusDomain::Floating` is active, only `FocusedPaneLocal` actions
/// (Float/Unfloat, ClosePane, RenamePane) are allowed. Everything else is
/// blocked — tiled layout actions, sidebar, workspace switching, command
/// palette, mouse drag, and pane selection overlays.
///
/// The only escape from floating is `prefix+f` (Float toggle) or closing the
/// floating pane (ClosePane).
pub(crate) fn route_interaction_for_session(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    match &intent {
        InteractionIntent::ActivateAction(action) => route_action(session, source, action),
        // Defensive: `dispatch_intent` expands this into FocusPane + the action before
        // routing, so the router should not normally see it. If it does, route by the
        // inner action's policy (the focus half is always benign).
        InteractionIntent::FocusPaneThenAction { action, .. } => {
            route_action(session, source, action)
        }
        InteractionIntent::FocusPane { .. } => {
            // FocusPane from mouse content/sidebar: blocked when floating.
            // Only the active floating pane can receive focus in floating domain.
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
        InteractionIntent::FocusWorkspace { .. } => {
            // Workspace switching: blocked when floating.
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
        InteractionIntent::EnterSidebarNav => {
            // Sidebar navigation: blocked when floating.
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
        InteractionIntent::ToggleWorkspaceCollapsed { .. } => {
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
        InteractionIntent::StartSidebarDrag { .. } => {
            // Sidebar drag: blocked when floating.
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
        // A View intent is a pass-through here: it carries only an action *name*, so its
        // real focus-domain policy is applied when `dispatch_view_intent` resolves it to a
        // `WmAction` and re-dispatches through `dispatch_action` (which routes it).
        InteractionIntent::View(_) => RouteDecision::Allow(intent),
    }
}

/// Route a WmAction based on the current focus domain and interaction source.
fn route_action(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    action: &WmAction,
) -> RouteDecision {
    if policy_allows(session, source, action_policy(action), Some(action)) {
        RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
    } else {
        RouteDecision::Block
    }
}

/// Does `policy` permit an action in the current focus domain, from this source?
///
/// The **single** place the six policies are interpreted, so a built-in and a plugin action are
/// judged by identical rules. The difference is only where the policy came from: a built-in's is
/// produced by [`action_policy`]'s exhaustive `match` (forgetting a variant is a compile error); a
/// name-keyed action's is **declared** on its `DynActionMeta` (a required field, so forgetting is
/// impossible there too).
///
/// `action` is `Some` only for built-ins — it exists solely for [`ActionPolicy::SourceDependent`],
/// which inspects `FocusPane`'s target. A dynamic action cannot be `FocusPane`, so it takes the
/// conservative branch (blocked while floating), which is the right default for an action the host
/// cannot introspect.
fn policy_allows(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    policy: ActionPolicy,
    action: Option<&WmAction>,
) -> bool {
    let floating = is_floating_domain(session);

    match policy {
        // True global app actions (ReloadConfig) — allowed in every focus domain, including
        // Floating. They have no tiled/floating layout impact, so blocking them when floating only
        // breaks hot-reload.
        ActionPolicy::Global => true,
        // AlwaysAllowed (CommandPalette, SpawnCommand, EnterMode) is a misnomer: it is blocked when
        // floating from the current sources. Future chrome sources (MouseTopMenu, MouseStatusBar)
        // may allow these even while floating.
        ActionPolicy::AlwaysAllowed => !floating,
        ActionPolicy::FocusedPaneLocal => true,
        ActionPolicy::TiledOnly => !floating,
        ActionPolicy::WorkspaceLevel => !floating,
        ActionPolicy::SourceDependent => {
            // FocusPane: allowed if it targets the active floating pane, otherwise blocked.
            if let Some(WmAction::FocusPane { pane_id }) = action {
                if !floating {
                    return true;
                }
                let active_floating = session
                    .active_workspace()
                    .and_then(|ws| ws.floating_panes.iter().find(|f| f.is_active))
                    .map(|f| f.pane.id);
                return active_floating == Some(*pane_id);
            }
            // Other source-dependent actions: defer to source when floating.
            if floating {
                match source {
                    InteractionSource::Keyboard => false,
                    InteractionSource::MouseContent => false,
                    InteractionSource::MouseLeftSidebar => false,
                }
            } else {
                true
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Focus-target helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Returns `true` when the active workspace is in `FocusDomain::Floating`.
pub(crate) fn is_floating_domain(session: &heca_core::layout::Session) -> bool {
    session
        .active_workspace()
        .map(|ws| ws.focus_domain == FocusDomain::Floating)
        .expect("active workspace must exist when checking focus domain")
}

/// Returns the `FocusDomain` of the active workspace.
///
/// Convenience wrapper for code that needs to branch on the domain directly
/// rather than just checking `is_floating_domain()`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by handlers and sidebar routing in Phase E")
)]
pub(crate) fn active_focus_domain(session: &heca_core::layout::Session) -> FocusDomain {
    session
        .active_workspace()
        .map(|ws| ws.focus_domain)
        .expect("active workspace must exist when checking focus domain")
}

/// Returns the focused pane ID from `AppState`, if any.
///
/// This is the canonical accessor — handlers should read
/// `state.focused_pane` through this helper rather than touching
/// the field directly, so the access pattern is traceable.
pub(crate) fn focused_pane_id(state: &AppState) -> Option<PaneId> {
    state.focused_pane
}

/// Checks whether a specific pane can receive focus from the given source.
///
/// When in floating domain, only the active floating pane can receive focus
/// from mouse/sidebar sources. Keyboard focus changes are blocked entirely
/// (they go through `dispatch_action` which handles policy).
///
/// Checks whether `pane_id` can receive focus from the given source.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by handlers and sidebar routing in Phase E")
)]
pub(crate) fn can_focus_pane(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    pane_id: PaneId,
) -> bool {
    if is_floating_domain(session) {
        // In floating domain, only the active floating pane can receive focus.
        let active_floating = session
            .active_workspace()
            .and_then(|ws| ws.floating_panes.iter().find(|f| f.is_active))
            .map(|f| f.pane.id);
        active_floating == Some(pane_id)
    } else {
        // In tiled domain, any pane can receive focus from any source.
        // (Individual sources may still block via route_interaction, but
        // this helper only answers the pane-targeting question.)
        match source {
            InteractionSource::Keyboard => true,
            InteractionSource::MouseContent => true,
            InteractionSource::MouseLeftSidebar => true,
        }
    }
}

/// Checks whether `pane_id` belongs to the floating panes in the active workspace.
///
/// Used by `handle_float` and `handle_close_pane_by_id` to decide
/// whether to process the pane as floating or tiled.
pub(crate) fn pane_is_floating(session: &heca_core::layout::Session, pane_id: PaneId) -> bool {
    session
        .active_workspace()
        .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════════
// Dispatch action
// ═══════════════════════════════════════════════════════════════════════════

/// The single public entry point for user-initiated WM actions.
///
/// Routes the action through the interaction policy layer. If allowed,
/// executes via the registry. If blocked, silently discards.
///
/// Handler-to-handler calls should use `registry.execute()` directly —
/// they bypass the router because they're inside an already-allowed
/// interaction.
pub(crate) fn dispatch_intent(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    intent: InteractionIntent,
) {
    // Overlay control (§2.7.2): `SubmitOverlay`/`CloseOverlay` are resolved here — not via the
    // `ActionRegistry` — because resolving an overlay runs its completion, which needs the
    // registry to dispatch a follow-up action (a handler gets no registry). Intercepted before
    // routing, like the `FocusPaneThenAction` composite below. Always allowed (overlay control
    // has no focus-domain policy; the follow-up action it dispatches is routed on its own).
    if let InteractionIntent::ActivateAction(WmAction::SubmitOverlay { overlay, action }) = &intent
    {
        let (overlay, action) = (*overlay, action.clone());
        // Marshal the modal body's named value fields (input/toggle/checkbox) into the result
        // `data` before the overlay is popped (plugin-task-ui-4).
        let data = crate::chrome::collect_overlay_form(state, overlay);
        crate::chrome::resolve_overlay(
            state,
            registry,
            overlay,
            crate::chrome::ModalResult::Action { id: action, data },
        );
        return;
    }
    if let InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay }) = &intent {
        let overlay = *overlay;
        crate::chrome::resolve_overlay(state, registry, overlay, crate::chrome::ModalResult::Dismissed);
        return;
    }

    // Composite: focus the pane, then run the action — each half policy-routed on its
    // own (mirrors what an active-targeted pane button does across two events on click).
    if let InteractionIntent::FocusPaneThenAction { pane_id, action } = intent {
        dispatch_intent(state, registry, source, InteractionIntent::FocusPane { pane_id });
        dispatch_intent(
            state,
            registry,
            source,
            InteractionIntent::ActivateAction(*action),
        );
        return;
    }
    let decision = route_interaction(state, source, intent);

    match decision {
        RouteDecision::Allow(InteractionIntent::ActivateAction(act)) => {
            // Central destructive-action gate: close-pane / delete-column / delete-workspace go
            // through the confirm chokepoint FIRST, so the confirm guard lives on the action and
            // every surface (keyboard, pane-header close button, context menu / dropdown, RPC)
            // confirms identically — never per call site. Returns true when it handled (confirmed
            // or ran) the action; the confirm dialog runs the raw action directly, never re-entering
            // this gate.
            if !crate::handlers::maybe_confirm_destructive(state, &act) {
                registry.execute(&act, state);
            }
        }
        // Expanded to FocusPane + the action above (before routing), so this is
        // unreachable in practice; handle it defensively as focus-then-act.
        RouteDecision::Allow(InteractionIntent::FocusPaneThenAction { pane_id, action }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
            registry.execute(&action, state);
        }
        RouteDecision::Allow(InteractionIntent::FocusPane { pane_id }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
        }
        RouteDecision::Allow(InteractionIntent::FocusWorkspace { ws_idx }) => {
            registry.execute(&WmAction::FocusWorkspace { ws_idx }, state);
        }
        RouteDecision::Allow(InteractionIntent::EnterSidebarNav) => {
            registry.execute(&WmAction::SidebarFocus, state);
        }
        RouteDecision::Allow(InteractionIntent::ToggleWorkspaceCollapsed { ws_idx }) => {
            crate::handlers::apply_ws_collapse(state, ws_idx, None);
        }
        RouteDecision::Allow(InteractionIntent::StartSidebarDrag { .. }) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] interaction: StartSidebarDrag intent allowed but not dispatched (drag initiated in mouse layer)"
            );
        }
        RouteDecision::Allow(InteractionIntent::View(vi)) => {
            dispatch_view_intent(state, registry, source, &vi);
        }
        RouteDecision::Block => {
            #[cfg(debug_assertions)]
            eprintln!("[heca] interaction: blocked intent from {:?}", source);
        }
    }
}

pub(crate) fn dispatch_action(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &WmAction,
) {
    dispatch_intent(
        state,
        registry,
        source,
        InteractionIntent::ActivateAction(action.clone()),
    );
}

/// Dispatch whatever a key was bound to — the press-time half of [`ActionRef`].
///
/// A `Builtin` was already resolved at config load and dispatches exactly as it always has. A
/// `Dynamic` names an action that may only have been registered *after* config was read (a provider,
/// a plugin); it is resolved **now**, through the same one door as every other named intent, so it
/// is policy-routed identically. Still unknown at press ⇒ a debug warning inside
/// `dispatch_view_intent`, never a crash.
pub(crate) fn dispatch_action_ref(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &ActionRef,
) {
    match action {
        ActionRef::Builtin(a) => dispatch_action(state, registry, source, a),
        ActionRef::Dynamic(intent) => dispatch_intent(
            state,
            registry,
            source,
            InteractionIntent::View(intent.clone()),
        ),
    }
}

/// Resolve and dispatch a declarative [`ViewNode`](crate::chrome::ViewNode) intent.
///
/// A `view::Intent` carries an action *name* (the same identifier config keys and RPC use)
/// plus optional args.
///
/// **This is the one front door** (`pluggable-chrome-plugin-plan.md` §2.7.2: "No parallel dispatch
/// path"). A name resolves down one of two back ends, and both are policy-routed:
///
/// 1. **Built-in** — [`action_from_name`](crate::input::action_from_name) maps it to a [`WmAction`],
///    which goes through [`dispatch_action`] exactly like a keypress. Policy comes from
///    [`action_policy`]'s exhaustive match.
/// 2. **Name-keyed** (plugin-04) — an action registered at runtime by a provider/plugin, which has
///    no `WmAction` variant because the enum is closed. Policy comes from its **declared**
///    `DynActionMeta.policy`, and both paths converge on the same [`policy_allows`], so a plugin
///    action is judged by identical rules.
///
/// An unknown name is not a crash: a binding or a menu item may legitimately name an action whose
/// provider is not mounted.
///
/// **Args are carried on both paths.** A name-keyed handler reads them off the `Intent` itself. A
/// built-in **parameterized** variant is constructed from them via
/// [`build_action`](crate::input::build_action) — so `{"action":"resize","args":{…}}` produces the
/// very same `WmAction` a config binding would. Unit built-ins ignore args, as they always did.
fn dispatch_view_intent(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    intent: &ViewIntent,
) {
    // 0. Judge the args against what the action DECLARES it takes, and say what is wrong. Without
    //    this the two failures below are indistinguishable and both silent: a misspelled required
    //    argument makes `build_action` return `None` (so the intent looks like an unknown action),
    //    and a misspelled optional one is simply dropped, leaving the action to run with a default
    //    nobody asked for.
    let args = intent_args_as_strings(intent);
    report_arg_problems(&state.action_catalog, &intent.action, &args);

    // 1. Built-in. Parameterized variants are built from the intent's args (`build_action`, the same
    //    constructor a config binding uses); unit variants come straight from the name.
    let builtin = crate::input::build_action(&intent.action, &args)
        .or_else(|| crate::input::action_from_name(&intent.action));
    if let Some(action) = builtin {
        dispatch_action(state, registry, source, &action);
        return;
    }

    // 2. Name-keyed (provider/plugin), routed by its DECLARED policy — the same `policy_allows` the
    //    built-in path reaches through `route_action`, so a plugin action is judged by identical
    //    rules.
    let Some(policy) = state.action_catalog.policy(&intent.action) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: view intent '{}' did not resolve to a known action",
            intent.action
        );
        return;
    };

    if crate::chrome::top_modal(state).is_some()
        || !policy_allows(&state.session, source, policy, None)
    {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: blocked dynamic action '{}' from {source:?}",
            intent.action
        );
        return;
    }
    if !registry.execute_dynamic(&intent.action, state, intent) {
        // Declared but host-unrunnable (`Dispatch::Declarative`): its owner lives across the plugin
        // boundary and forwarding lands with the WASM bridge (plugin-08). Never a crash.
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: action '{}' is declared but has no host handler",
            intent.action
        );
    }
}

/// Report a dispatched intent's argument mistakes against the action's declared
/// [`args`](crate::actions::ActionMeta::args).
///
/// Built-in and name-keyed actions alike — they share one catalog, so they are judged by one rule,
/// the same way [`policy_allows`] judges them by one rule. An action the catalog does not know is
/// not this function's business: the caller already reports an unresolved name.
///
/// This reports and does not decide. A missing required argument stops the action anyway (nothing
/// can build it); an unknown or malformed one costs only itself and the rest of the call still
/// stands — the rule the declarative UI model already applies to a widget property.
fn report_arg_problems(
    catalog: &crate::actions::ActionCatalog,
    name: &str,
    args: &std::collections::HashMap<String, String>,
) {
    let Some(meta) = catalog.find(name) else {
        return;
    };
    for problem in crate::actions::check_args(&meta.args, args) {
        eprintln!("[heca] action '{name}': {problem}");
    }
}

/// Flatten an [`Intent`](crate::chrome::Intent)'s typed args into the `name -> string` map
/// [`build_action`](crate::input::build_action) parses, so a declarative intent and a `config.toml`
/// binding construct a parameterized built-in through **one** code path.
fn intent_args_as_strings(intent: &ViewIntent) -> std::collections::HashMap<String, String> {
    use crate::chrome::PropValue;
    intent
        .args
        .iter()
        .map(|(k, v)| {
            let s = match v {
                PropValue::Bool(b) => b.to_string(),
                PropValue::Int(i) => i.to_string(),
                PropValue::Float(f) => f.to_string(),
                PropValue::Text(t) | PropValue::Color(t) | PropValue::Glyph(t) => t.clone(),
                other => format!("{other:?}"),
            };
            (k.clone(), s)
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::column::Pane;
    use heca_core::layout::types::{Point, Rectangle};
    use heca_core::layout::workspace::FloatingPane;
    use heca_core::layout::{LayoutOptions, PaneId, SessionId, Size};

    /// Helper to create a minimal Session for routing tests.
    fn test_session() -> heca_core::layout::Session {
        heca_core::layout::Session::new(
            SessionId(0),
            Size::new(800.0, 600.0),
            1.0,
            LayoutOptions::default(),
        )
    }

    /// In Tiled domain, TiledOnly actions are allowed from any source.
    #[test]
    fn tiled_domain_allows_tiled_actions() {
        let session = test_session();
        let actions = [
            WmAction::FocusLeft,
            WmAction::FocusRight,
            WmAction::SplitHorizontal,
            WmAction::ZoomColumn,
            WmAction::SidebarLeft,
            WmAction::SidebarFocus,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Tiled domain should allow {:?} but got {:?}",
                action,
                decision,
            );
        }
    }

    /// A View intent is a pass-through at the router: it carries only an action name, so
    /// the router always allows it and real policy is applied when it resolves to a
    /// `WmAction` and re-dispatches. Also exercises constructing the `View` variant.
    #[test]
    fn view_intent_passes_through_router() {
        let session = test_session();
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::Keyboard,
            InteractionIntent::View(ViewIntent::new("focus_left")),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(InteractionIntent::View(_))),
            "router should pass a View intent through, got {:?}",
            decision,
        );
    }

    /// In Tiled domain, FocusedPaneLocal actions are allowed.
    #[test]
    fn tiled_domain_allows_focused_pane_local() {
        let session = test_session();
        let actions = [
            WmAction::Float,
            WmAction::ClosePane,
            WmAction::RenamePane,
            // Selection (host capability, Task 02).
            WmAction::EnterSelectionMode,
            WmAction::SelectionLeft,
            WmAction::SelectionRight,
            WmAction::SelectionUp,
            WmAction::SelectionDown,
            WmAction::ClearSelection,
            WmAction::CopySelection,
            WmAction::PasteClipboard,
            WmAction::OpenLinkAtCaret,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Tiled domain should allow {:?} but got {:?}",
                action,
                decision,
            );
        }
    }

    /// In Floating domain, TiledOnly actions are blocked.
    #[test]
    fn floating_domain_blocks_tiled_only() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [
            WmAction::FocusLeft,
            WmAction::FocusRight,
            WmAction::FocusUp,
            WmAction::FocusDown,
            WmAction::SplitHorizontal,
            WmAction::ZoomColumn,
            WmAction::SidebarFocus,
            WmAction::SidebarLeft,
            WmAction::WorkspaceNext,
            WmAction::CommandPalette,
            WmAction::FocusToggleLocal,
            WmAction::PaneSelect,
            WmAction::FloatAt {
                pane_id: PaneId(0),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Block),
                "Floating domain should block {:?} but got {:?}",
                action,
                decision,
            );
        }
    }

    /// In Floating domain, FocusedPaneLocal actions are allowed.
    #[test]
    fn floating_domain_allows_focused_pane_local() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [WmAction::Float, WmAction::ClosePane, WmAction::RenamePane, WmAction::OpenContextMenu];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Floating domain should allow {:?} but got {:?}",
                action,
                decision,
            );
        }
    }

    /// Intent variants are blocked when floating.
    #[test]
    fn floating_domain_blocks_intent_variants() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let intents = [
            InteractionIntent::FocusPane {
                pane_id: PaneId(99),
            },
            InteractionIntent::FocusWorkspace { ws_idx: 0 },
            InteractionIntent::EnterSidebarNav,
            InteractionIntent::StartSidebarDrag {
                pane_id: PaneId(42),
            },
        ];
        for intent in &intents {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                intent.clone(),
            );
            assert!(
                matches!(decision, RouteDecision::Block),
                "Floating domain should block {:?} but got {:?}",
                intent,
                decision,
            );
        }
    }

    /// Action policy classifications are exhaustive — every variant is matched.
    #[test]
    fn action_policy_covers_all_variants() {
        let unit_actions: Vec<WmAction> = vec![
            WmAction::FocusLeft,
            WmAction::FocusRight,
            WmAction::FocusUp,
            WmAction::FocusDown,
            WmAction::NextPane,
            WmAction::PrevPane,
            WmAction::FocusToggleLocal,
            WmAction::FocusToggleGlobal,
            WmAction::SplitHorizontal,
            WmAction::SplitVertical,
            WmAction::ZoomColumn,
            WmAction::ResizeIncrease,
            WmAction::ResizeDecrease,
            WmAction::PaneHeightIncrease,
            WmAction::PaneHeightDecrease,
            WmAction::SwapLeft,
            WmAction::SwapRight,
            WmAction::SwapUp,
            WmAction::SwapDown,
            WmAction::MovePaneLeft { pane_id: None },
            WmAction::MovePaneRight { pane_id: None },
            WmAction::MoveColumnUp,
            WmAction::MoveColumnDown,
            WmAction::PaneSelect,
            WmAction::SwapPane,
            WmAction::SwapAndFocusPane,
            WmAction::PaneTake,
            WmAction::PaneTakeAndFocus,
            WmAction::Float,
            WmAction::ClosePane,
            WmAction::RenamePane,
            WmAction::RenameColumn,
            WmAction::SidebarLeft,
            WmAction::SidebarRight,
            WmAction::SidebarFocus,
            WmAction::SidebarUp,
            WmAction::SidebarDown,
            WmAction::SidebarLeftNav,
            WmAction::SidebarRightNav,
            WmAction::SidebarPeek,
            WmAction::SidebarExpandToggle,
            WmAction::SidebarCreateWorkspace,
            WmAction::SidebarCreateColumn,
            WmAction::SidebarSplitInColumn,
            WmAction::SidebarZoomSelectedColumn,
            WmAction::SidebarDeleteSelected,
            WmAction::CollapseCurrentWorkspace,
            WmAction::ExpandCurrentWorkspace,
            WmAction::ToggleCurrentWorkspaceCollapsed,
            WmAction::CollapseCurrentColumn,
            WmAction::ExpandCurrentColumn,
            WmAction::ToggleCurrentColumnCollapsed,
            WmAction::WorkspaceNext,
            WmAction::WorkspacePrev,
            WmAction::CreateWorkspace,
            WmAction::RenameWorkspace,
            WmAction::CommandPalette,
            WmAction::ReloadConfig,
            // Scrollback
            WmAction::ScrollbackPageUp,
            WmAction::ScrollbackPageDown,
            WmAction::ScrollbackLineUp { amount: 3 },
            WmAction::ScrollbackLineDown { amount: 3 },
            WmAction::ScrollbackToTop,
            WmAction::ScrollbackToBottom,
            WmAction::ExitScrollback,
            // Direct scroll
            WmAction::ScrollLineUp,
            WmAction::ScrollLineDown,
            WmAction::ScrollPageUp,
            WmAction::ScrollPageDown,
            WmAction::ScrollToTop,
            WmAction::ScrollToBottom,
            // Selection (host capability, Task 02).
            WmAction::EnterSelectionMode,
            WmAction::ClearSelection,
            WmAction::SelectionLeft,
            WmAction::SelectionRight,
            WmAction::SelectionUp,
            WmAction::SelectionDown,
            WmAction::CopySelection,
            WmAction::PasteClipboard,
            WmAction::BeginSelection,
            WmAction::ToggleSelectionEndpoint,
            WmAction::AppFontZoom {
                step: crate::input::FontZoomStep::In,
            },
            WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: crate::input::FontZoomStep::In,
            },
        ];
        for action in &unit_actions {
            let _policy = action_policy(action);
        }

        let param_actions = [
            WmAction::FocusPane { pane_id: PaneId(0) },
            WmAction::FocusWorkspace { ws_idx: 0 },
            WmAction::Swap {
                a_id: PaneId(0),
                b_id: PaneId(0),
            },
            WmAction::Move {
                pane_id: PaneId(0),
                target_col: 0,
            },
            WmAction::MovePaneToWorkspace {
                pane_id: PaneId(0),
                ws_idx: 0,
            },
            WmAction::MovePaneToColumn {
                pane_id: PaneId(0),
                ws_idx: 0,
                col_idx: 0,
            },
            WmAction::MoveColumnToWorkspace {
                col_idx: 0,
                ws_idx: 0,
                focus: false,
            },
            WmAction::Resize {
                target: crate::input::ResizeTarget::Column,
                axis: crate::input::ResizeAxis::X,
                amount: 0.0,
            },
            WmAction::ResizeColumnBy {
                col_idx: 0,
                delta: 0.0,
            },
            WmAction::ResizePaneHeightBy {
                col_idx: 0,
                pane_idx: 0,
                delta: 0.0,
            },
            WmAction::ResizeTo {
                target: crate::input::ResizeTarget::Column,
                width: 0.0,
                height: 0.0,
            },
            WmAction::FloatAt {
                pane_id: PaneId(0),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            WmAction::ClosePaneById { pane_id: PaneId(0) },
            WmAction::RenameTarget {
                pane_id: PaneId(0),
                name: String::new(),
            },
            WmAction::RenameColumnByIdx {
                ws_idx: 0,
                col_idx: 0,
            },
            WmAction::ResetPaneNameById { pane_id: PaneId(0) },
            WmAction::ResetWorkspaceNameByIdx { ws_idx: 0 },
            WmAction::SpawnCommand {
                command: String::new(),
                kind: crate::input::SpawnKind::Terminal,
                float: false,
                close_policy: heca_core::runtime::PaneClosePolicy::default(),
            },
            WmAction::EnterMode {
                name: String::new(),
            },
            WmAction::AddPaneToColumn {
                ws_idx: 0,
                col_idx: 0,
            },
            WmAction::DeleteColumn {
                ws_idx: 0,
                col_idx: 0,
            },
            WmAction::DeleteWorkspace { ws_idx: 0 },
            WmAction::TakePane {
                pane_id: PaneId(0),
                focus_after: false,
            },
            WmAction::OpenLink {
                url: "https://example.com".into(),
            },
        ];
        for action in &param_actions {
            let _policy = action_policy(action);
        }

        // Spot-check specific classifications
        assert_eq!(action_policy(&WmAction::FocusLeft), ActionPolicy::TiledOnly);
        // Peek focuses a tiled pane/workspace from the sidebar — same domain as every
        // other Sidebar* action, so it must not fire while a floating pane is active.
        assert_eq!(action_policy(&WmAction::SidebarPeek), ActionPolicy::TiledOnly);
        // Column rename (active or by-idx) is a tiled-layout op → TiledOnly, like RenameColumn.
        assert_eq!(
            action_policy(&WmAction::RenameColumn),
            ActionPolicy::TiledOnly
        );
        assert_eq!(
            action_policy(&WmAction::RenameColumnByIdx {
                ws_idx: 0,
                col_idx: 0
            }),
            ActionPolicy::TiledOnly
        );
        // Reset-name mirrors rename: pane-local for panes, workspace-level for workspaces.
        assert_eq!(
            action_policy(&WmAction::ResetPaneName),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ResetWorkspaceNameByIdx { ws_idx: 0 }),
            ActionPolicy::WorkspaceLevel
        );
        assert_eq!(
            action_policy(&WmAction::Float),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ClosePane),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::OpenContextMenu),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ScrollbackPageUp),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ScrollbackToBottom),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ExitScrollback),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::ScrollToOffset { rows: 0 }),
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            action_policy(&WmAction::CommandPalette),
            ActionPolicy::AlwaysAllowed
        );
        assert_eq!(action_policy(&WmAction::ReloadConfig), ActionPolicy::Global);
        // Chrome keyboard focus is chrome state, not pane layout: it stays reachable while a floating
        // pane is active. `SidebarFocus` differs deliberately — it enters a nav mode that moves pane
        // focus, so it is TiledOnly.
        assert_eq!(
            action_policy(&WmAction::FocusDock { dock: None }),
            ActionPolicy::Global
        );
        assert_eq!(
            action_policy(&WmAction::FocusDock {
                dock: Some("workspaces".into())
            }),
            ActionPolicy::Global
        );
        assert_eq!(
            action_policy(&WmAction::SidebarFocus),
            ActionPolicy::TiledOnly
        );
        assert_eq!(
            action_policy(&WmAction::OpenLink {
                url: "https://example.com".into()
            }),
            ActionPolicy::Global
        );
        assert_eq!(
            action_policy(&WmAction::WorkspaceNext),
            ActionPolicy::WorkspaceLevel
        );
        assert_eq!(
            action_policy(&WmAction::FocusPane { pane_id: PaneId(0) }),
            ActionPolicy::SourceDependent
        );
    }

    /// is_floating_domain returns false for default (Tiled) workspace.
    #[test]
    fn is_floating_domain_default_is_tiled() {
        let session = test_session();
        assert!(!is_floating_domain(&session));
    }

    // ── plugin-04 / T3: a name-keyed action is judged by IDENTICAL rules ──

    /// A name-keyed (plugin) action's DECLARED policy runs through the very same `policy_allows`
    /// the built-in path reaches — so declaring `TiledOnly` blocks it when a floating pane owns the
    /// domain, exactly as if it were a `WmAction` classified `TiledOnly` in the exhaustive match.
    #[test]
    fn a_declared_tiled_only_action_is_blocked_when_floating() {
        let mut session = test_session();
        // Tiled: allowed.
        assert!(policy_allows(
            &session,
            InteractionSource::Keyboard,
            ActionPolicy::TiledOnly,
            None
        ));
        // Floating: blocked.
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert!(!policy_allows(
            &session,
            InteractionSource::Keyboard,
            ActionPolicy::TiledOnly,
            None
        ));
    }

    /// `Global` is the only policy that is truly always allowed (the `AlwaysAllowed` name is a trap
    /// — the router blocks THAT one when floating). A plugin action declaring `Global` keeps working
    /// while a floating pane owns the domain.
    #[test]
    fn a_declared_global_action_survives_the_floating_domain() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert!(policy_allows(
            &session,
            InteractionSource::Keyboard,
            ActionPolicy::Global,
            None
        ));
        assert!(
            !policy_allows(
                &session,
                InteractionSource::Keyboard,
                ActionPolicy::AlwaysAllowed,
                None
            ),
            "AlwaysAllowed is a misnomer: the router blocks it when floating"
        );
    }

    /// `SourceDependent` with no introspectable action (the name-keyed case) takes the conservative
    /// branch and is blocked when floating — the host cannot prove it targets the active pane.
    #[test]
    fn source_dependent_without_an_action_is_conservatively_blocked_when_floating() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert!(!policy_allows(
            &session,
            InteractionSource::Keyboard,
            ActionPolicy::SourceDependent,
            None
        ));
    }

    /// The args of a declarative `Intent` construct a parameterized built-in through the SAME
    /// `build_action` a `config.toml` binding uses — so a menu item, an RPC call and a keybinding
    /// all produce one identical `WmAction`. Before this, `dispatch_view_intent` dropped the args.
    #[test]
    fn intent_args_build_the_same_parameterized_action_as_a_config_binding() {
        use crate::chrome::{Intent, PropValue};

        let mut intent = Intent::new("scroll_to_offset");
        intent.args.insert("rows".to_string(), PropValue::Int(12));

        let from_intent =
            crate::input::build_action("scroll_to_offset", &intent_args_as_strings(&intent));

        let mut config_args = std::collections::HashMap::new();
        config_args.insert("rows".to_string(), "12".to_string());
        let from_config = crate::input::build_action("scroll_to_offset", &config_args);

        assert_eq!(from_intent, from_config);
        assert_eq!(from_intent, Some(WmAction::ScrollToOffset { rows: 12 }));
    }

    /// A unit built-in with no args still resolves by name (the `action_from_name` fallback), so the
    /// existing name-dispatch behaviour is unchanged.
    #[test]
    fn a_unit_builtin_still_resolves_by_name_with_no_args() {
        use crate::chrome::Intent;
        let intent = Intent::new("reload_config");
        let args = intent_args_as_strings(&intent);
        assert!(args.is_empty());
        assert!(crate::input::build_action("reload_config", &args).is_none());
        assert_eq!(
            crate::input::action_from_name("reload_config"),
            Some(WmAction::ReloadConfig)
        );
    }

    /// Setting focus_domain to Floating is detected by helpers.
    #[test]
    fn floating_domain_detected_after_set() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert!(is_floating_domain(&session));
    }

    /// MouseContent FocusPane is blocked when floating.
    #[test]
    fn floating_blocks_mouse_content_focus_pane() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        // MouseContent FocusPane should be blocked when floating
        // (only the active floating pane could receive focus, and this targets a tiled pane)
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::MouseContent,
            InteractionIntent::ActivateAction(WmAction::FocusPane {
                pane_id: PaneId(99),
            }),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "MouseContent FocusPane should be blocked when floating, got {:?}",
            decision,
        );
    }

    /// MouseLeftSidebar actions are blocked when floating (via the guard in mouse.rs,
    /// tested here as part of the router's policy).
    #[test]
    fn floating_blocks_mouse_left_sidebar_actions() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        // Sidebar actions should be blocked when floating.
        let actions = [
            WmAction::SidebarFocus,
            WmAction::SidebarLeft,
            WmAction::SidebarUp,
        ];
        for action in &actions {
            // Test via MouseLeftSidebar source (same result as Keyboard, but testing the source explicitly)
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::MouseLeftSidebar,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Block),
                "MouseLeftSidebar should block {:?} when floating, got {:?}",
                action,
                decision,
            );
        }
    }

    // ── Focus-target helper tests ──

    /// `active_focus_domain` returns Tiled for default workspace.
    #[test]
    fn active_focus_domain_default_is_tiled() {
        let session = test_session();
        assert_eq!(active_focus_domain(&session), FocusDomain::Tiled);
    }

    /// `active_focus_domain` returns Floating after setting.
    #[test]
    fn active_focus_domain_floating_after_set() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert_eq!(active_focus_domain(&session), FocusDomain::Floating);
    }

    /// `pane_is_floating` returns false for a pane that is in the scrolling columns.
    #[test]
    fn pane_is_floating_returns_false_for_tiled_pane() {
        let session = test_session();
        // Default session has pane ID 1 in scrolling columns, not floating.
        assert!(!pane_is_floating(&session, PaneId(1)));
    }

    /// `pane_is_floating` returns false for non-existent pane.
    #[test]
    fn pane_is_floating_returns_false_for_nonexistent() {
        let session = test_session();
        assert!(!pane_is_floating(&session, PaneId(9999)));
    }

    /// `can_focus_pane` allows any pane when in tiled domain.
    #[test]
    fn can_focus_pane_allows_any_in_tiled_domain() {
        let session = test_session();
        assert!(can_focus_pane(
            &session,
            InteractionSource::Keyboard,
            PaneId(1)
        ));
        assert!(can_focus_pane(
            &session,
            InteractionSource::MouseContent,
            PaneId(1)
        ));
        assert!(can_focus_pane(
            &session,
            InteractionSource::MouseLeftSidebar,
            PaneId(1)
        ));
    }

    /// `can_focus_pane` blocks non-floating pane when in floating domain.
    #[test]
    fn can_focus_pane_blocks_non_floating_when_floating() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        // In a floating domain, pane 1 (in scrolling columns) is NOT the active floating pane.
        // So can_focus_pane should block it from all sources.
        assert!(!can_focus_pane(
            &session,
            InteractionSource::Keyboard,
            PaneId(1)
        ));
        assert!(!can_focus_pane(
            &session,
            InteractionSource::MouseContent,
            PaneId(1)
        ));
        assert!(!can_focus_pane(
            &session,
            InteractionSource::MouseLeftSidebar,
            PaneId(1)
        ));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Phase E — Regression tests
    // ═══════════════════════════════════════════════════════════════════════

    // ── E.2: Floating-domain focus blocking regression tests ──

    /// When floating, keyboard FocusPane targeting the active floating pane
    /// is allowed (SourceDependent policy).
    #[test]
    fn floating_focus_allows_active_floating_pane_via_keyboard() {
        let mut session = test_session();
        session.add_pane(Pane::new(PaneId(99), "float-99"), None, true);
        let ws = session.active_workspace_mut().unwrap();
        ws.floating_panes.push(FloatingPane {
            pane: Pane::new(PaneId(99), "float-99"),
            position: Point::new(0.0, 0.0),
            size: Size::new(200.0, 100.0),
            is_active: true,
            original_column_idx: None,
            original_pane_idx: None,
        });
        ws.focus_domain = FocusDomain::Floating;

        // FocusPane targeting the active floating pane should be allowed from Keyboard.
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(WmAction::FocusPane {
                pane_id: PaneId(99),
            }),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "FocusPane on active floating pane should be allowed when floating, got {:?}",
            decision,
        );
    }

    /// When floating, keyboard FocusPane targeting a tiled pane is blocked.
    #[test]
    fn floating_focus_blocks_tiled_pane_via_keyboard() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        // FocusPane targeting tiled pane (ID 1) should be blocked.
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: PaneId(1) }),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "FocusPane on tiled pane should be blocked when floating, got {:?}",
            decision,
        );
    }

    /// When floating, FocusPane intent from MouseContent is blocked.
    #[test]
    fn floating_focus_pane_intent_blocked_via_mouse_content() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let decision = route_interaction_for_session(
            &session,
            InteractionSource::MouseContent,
            InteractionIntent::FocusPane { pane_id: PaneId(1) },
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "FocusPane intent from MouseContent should be blocked when floating, got {:?}",
            decision,
        );
    }

    /// When tiled, sidebar actions are allowed via MouseLeftSidebar.
    #[test]
    fn tiled_sidebar_action_allowed_via_mouse_sidebar() {
        let session = test_session();
        let actions = [
            WmAction::SidebarFocus,
            WmAction::SidebarLeft,
            WmAction::SidebarRight,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::MouseLeftSidebar,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Tiled domain should allow {:?} from MouseLeftSidebar, got {:?}",
                action,
                decision,
            );
        }
    }

    /// When tiled, FocusPane from MouseContent is allowed.
    #[test]
    fn tiled_content_focus_pane_allowed_via_mouse_content() {
        let session = test_session();
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::MouseContent,
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: PaneId(1) }),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow FocusPane from MouseContent, got {:?}",
            decision,
        );
    }

    // ── E.3: Behavior preservation tests ──

    /// When tiled, keyboard navigation actions (FocusLeft/Right/Up/Down) are allowed.
    #[test]
    fn tiled_keyboard_focus_navigation_allowed() {
        let session = test_session();
        let actions = [
            WmAction::FocusLeft,
            WmAction::FocusRight,
            WmAction::FocusUp,
            WmAction::FocusDown,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Tiled domain should allow {:?} via Keyboard, got {:?}",
                action,
                decision,
            );
        }
    }

    /// When tiled, workspace actions are allowed.
    #[test]
    fn tiled_workspace_actions_allowed() {
        let session = test_session();
        let actions = [
            WmAction::WorkspaceNext,
            WmAction::WorkspacePrev,
            WmAction::CreateWorkspace,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Tiled domain should allow {:?} via Keyboard, got {:?}",
                action,
                decision,
            );
        }
    }

    /// When floating, AlwaysAllowed actions (CommandPalette, SpawnCommand) are
    /// blocked from current UI sources. ReloadConfig is NOT in this group — it
    /// is `Global` (always allowed, even when floating) because it has no layout
    /// impact; see `floating_allows_global_reload`.
    #[test]
    fn floating_blocks_always_allowed_from_keyboard() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [
            WmAction::CommandPalette,
            WmAction::SpawnCommand {
                command: String::new(),
                kind: crate::input::SpawnKind::Terminal,
                float: false,
                close_policy: heca_core::runtime::PaneClosePolicy::default(),
            },
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Block),
                "Floating domain should block {:?} from Keyboard (may allow from chrome later), got {:?}",
                action,
                decision,
            );
        }
    }

    /// ReloadConfig is a `Global` action — allowed in every focus domain,
    /// including Floating (it reloads config from disk with no layout impact).
    /// Regression test for the bug where hot-reload silently failed whenever a
    /// floating pane was active (style only applied on full restart).
    #[test]
    fn floating_allows_global_reload() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let decision = route_interaction_for_session(
            &session,
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(WmAction::ReloadConfig),
        );
        assert!(
            matches!(
                decision,
                RouteDecision::Allow(InteractionIntent::ActivateAction(_))
            ),
            "Floating domain should allow ReloadConfig from Keyboard (Global action), got {:?}",
            decision,
        );
    }

    /// When floating, FocusedPaneLocal actions (Float, ClosePane, RenamePane,
    /// scrollback, selection actions) are still allowed.
    #[test]
    fn floating_allows_focused_pane_local_via_keyboard() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [
            WmAction::Float,
            WmAction::ClosePane,
            WmAction::RenamePane,
            // scrollback
            WmAction::ScrollbackPageUp,
            WmAction::ScrollbackPageDown,
            WmAction::ScrollbackToTop,
            WmAction::ScrollbackToBottom,
            WmAction::ExitScrollback,
            // Direct scroll (same policy)
            WmAction::ScrollLineUp,
            WmAction::ScrollLineDown,
            WmAction::ScrollPageUp,
            WmAction::ScrollPageDown,
            WmAction::ScrollToTop,
            WmAction::ScrollToBottom,
            // Selection (host capability, Task 02) — allowed in both domains.
            WmAction::EnterSelectionMode,
            WmAction::SelectionLeft,
            WmAction::SelectionRight,
            WmAction::SelectionUp,
            WmAction::SelectionDown,
            WmAction::ClearSelection,
            WmAction::CopySelection,
            WmAction::PasteClipboard,
        ];
        for action in &actions {
            let decision = route_interaction_for_session(
                &session,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Floating domain should allow {:?} via Keyboard, got {:?}",
                action,
                decision,
            );
        }
    }

    /// When floating, FocusedPaneLocal actions are allowed from all sources.
    #[test]
    fn floating_allows_focused_pane_local_from_all_sources() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let sources = [
            InteractionSource::Keyboard,
            InteractionSource::MouseContent,
            InteractionSource::MouseLeftSidebar,
        ];
        let actions = [
            WmAction::Float,
            WmAction::ClosePane,
            // Selection (host capability, Task 02) — allowed from all sources.
            WmAction::EnterSelectionMode,
            WmAction::SelectionLeft,
            WmAction::SelectionRight,
            WmAction::SelectionUp,
            WmAction::SelectionDown,
            WmAction::ClearSelection,
            WmAction::CopySelection,
            WmAction::PasteClipboard,
        ];

        for source in &sources {
            for action in &actions {
                let decision = route_interaction_for_session(
                    &session,
                    *source,
                    InteractionIntent::ActivateAction(action.clone()),
                );
                assert!(
                    matches!(decision, RouteDecision::Allow(_)),
                    "Floating domain should allow {:?} from {:?}, got {:?}",
                    action,
                    source,
                    decision,
                );
            }
        }
    }

    /// Sidebar navigation intent is blocked when floating.
    #[test]
    fn floating_blocks_sidebar_nav_intent() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let decision = route_interaction_for_session(
            &session,
            InteractionSource::MouseLeftSidebar,
            InteractionIntent::EnterSidebarNav,
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "EnterSidebarNav should be blocked when floating, got {:?}",
            decision,
        );
    }

    /// Sidebar navigation intent is allowed when tiled.
    #[test]
    fn tiled_allows_sidebar_nav_intent() {
        let session = test_session();
        let decision = route_interaction_for_session(
            &session,
            InteractionSource::MouseLeftSidebar,
            InteractionIntent::EnterSidebarNav,
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "EnterSidebarNav should be allowed when tiled, got {:?}",
            decision,
        );
    }

    /// can_focus_pane allows the active floating pane when in floating domain.
    #[test]
    fn can_focus_pane_allows_active_floating_pane() {
        let mut session = test_session();
        // Add a floating pane with ID 99, set active
        let ws = session.active_workspace_mut().unwrap();
        ws.update_working_area(Rectangle::new(
            Point::new(0.0, 0.0),
            Size::new(1280.0, 800.0),
        ));
        ws.floating_panes.push(FloatingPane {
            pane: Pane::new(PaneId(99), "float-99"),
            position: Point::new(50.0, 50.0),
            size: Size::new(800.0, 600.0),
            is_active: true,
            original_column_idx: None,
            original_pane_idx: None,
        });
        ws.focus_domain = FocusDomain::Floating;

        // The active floating pane (ID 99) can be focused from all sources.
        assert!(can_focus_pane(
            &session,
            InteractionSource::Keyboard,
            PaneId(99)
        ));
        assert!(can_focus_pane(
            &session,
            InteractionSource::MouseContent,
            PaneId(99)
        ));
        assert!(can_focus_pane(
            &session,
            InteractionSource::MouseLeftSidebar,
            PaneId(99)
        ));
    }
}
