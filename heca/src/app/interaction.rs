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
use crate::input::WmAction;
use heca_core::layout::FocusDomain;

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
/// TODO(wire-intents): FocusPane, FocusWorkspace, EnterSidebarNav, StartSidebarDrag
/// will be constructed when sidebar/mouse click routing goes through dispatch_action.
#[derive(Debug, Clone)]
pub(crate) enum InteractionIntent {
    /// A keyboard shortcut resolved to a WM action.
    ActivateAction(WmAction),
    /// Focus a specific pane (from sidebar click, content click, or RPC).
    /// TODO(wire-intents): will be constructed when sidebar/mouse routing uses intents
    #[allow(dead_code)] // TODO(wire-intents): wired in Phase B
    FocusPane { pane_id: u64 },
    /// Focus a specific workspace (from sidebar click).
    /// TODO(wire-intents): will be constructed when sidebar/mouse routing uses intents
    #[allow(dead_code)] // TODO(wire-intents): wired in Phase B
    FocusWorkspace { ws_idx: usize },
    /// Enter sidebar navigation mode (from keyboard shortcut or click).
    /// TODO(wire-intents): will be constructed when sidebar/mouse routing uses intents
    #[allow(dead_code)] // TODO(wire-intents): wired in Phase B
    EnterSidebarNav,
    /// Start dragging a sidebar item (no WmAction equivalent).
    /// TODO(wire-intents): will be constructed when sidebar/mouse routing uses intents
    #[allow(dead_code)] // TODO(wire-intents): wired in Phase B
    StartSidebarDrag { #[allow(dead_code)] pane_id: u64 },
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
enum ActionPolicy {
    /// Always allowed regardless of focus domain.
    AlwaysAllowed,
    /// Only meaningful in Tiled domain — blocked when Floating.
    TiledOnly,
    /// Operates on the focused pane regardless of domain (close, rename, float/unfloat).
    FocusedPaneLocal,
    /// Affects workspace structure — blocked when Floating (for now).
    WorkspaceLevel,
    /// Policy depends on the interaction source.
    SourceDependent,
}

/// Classify a WM action into its policy category.
fn action_policy(action: &WmAction) -> ActionPolicy {
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
        | WmAction::ResizeIncrease
        | WmAction::ResizeDecrease
        | WmAction::PaneHeightIncrease
        | WmAction::PaneHeightDecrease
        | WmAction::SwapLeft
        | WmAction::SwapRight
        | WmAction::SwapUp
        | WmAction::SwapDown
        | WmAction::MovePaneLeft
        | WmAction::MovePaneRight
        | WmAction::MoveColumnUp
        | WmAction::MoveColumnDown
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::MovePaneToWorkspace { .. }
        | WmAction::MovePaneToColumn { .. }
        | WmAction::MoveColumnToWorkspace { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::RenameColumn
        | WmAction::DeleteColumn { .. }
        | WmAction::AddPaneToColumn { .. }
        // Sidebar actions: blocked when Floating
        | WmAction::SidebarLeft
        | WmAction::SidebarRight
        | WmAction::SidebarFocus
        | WmAction::SidebarUp
        | WmAction::SidebarDown
        | WmAction::SidebarLeftNav
        | WmAction::SidebarRightNav
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
        // FloatAt: spawns new floating pane — blocked when already floating
        | WmAction::FloatAt { .. } => ActionPolicy::TiledOnly,

        // ── Focused-pane-local: allowed in both domains ──
        WmAction::Float
        | WmAction::ClosePane
        | WmAction::ClosePaneById { .. }
        | WmAction::RenamePane
        | WmAction::RenameTarget { .. } => ActionPolicy::FocusedPaneLocal,

        // ── Workspace-level: blocked when Floating ──
        WmAction::WorkspaceNext
        | WmAction::WorkspacePrev
        | WmAction::FocusWorkspace { .. }
        | WmAction::CreateWorkspace
        | WmAction::RenameWorkspace
        | WmAction::DeleteWorkspace { .. } => ActionPolicy::WorkspaceLevel,

        // ── Always-allowed: work regardless of domain ──
        WmAction::CommandPalette
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. }
        | WmAction::ReloadConfig => ActionPolicy::AlwaysAllowed,

        // ── Source-dependent: may be allowed from some sources ──
        WmAction::FocusPane { .. } => ActionPolicy::SourceDependent,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Route interaction
// ═══════════════════════════════════════════════════════════════════════════

/// Decide whether an interaction is allowed under the current state.
///
/// Delegates to `route_interaction_for_session()` with `state.session`.
/// Applies floating-domain policy: when `FocusDomain::Floating` is active,
/// only `FocusedPaneLocal` actions are allowed from keyboard/mouse sources.
pub(crate) fn route_interaction(
    state: &AppState,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
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
        InteractionIntent::ActivateAction(action) => {
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
        InteractionIntent::StartSidebarDrag { .. } => {
            // Sidebar drag: blocked when floating.
            if is_floating_domain(session) {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(intent)
            }
        }
    }
}

/// Route a WmAction based on the current focus domain and interaction source.
fn route_action(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    action: &WmAction,
) -> RouteDecision {
    let floating = is_floating_domain(session);
    let policy = action_policy(action);

    match policy {
        ActionPolicy::AlwaysAllowed => {
            // AlwaysAllowed actions (CommandPalette, SpawnCommand, ReloadConfig, EnterMode)
            // are blocked when floating from current sources (Keyboard, MouseContent, MouseLeftSidebar).
            // Future chrome sources (MouseTopMenu, MouseStatusBar) may allow these even while floating.
            if floating {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
            }
        }
        ActionPolicy::FocusedPaneLocal => RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone())),
        ActionPolicy::TiledOnly => {
            if floating {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
            }
        }
        ActionPolicy::WorkspaceLevel => {
            if floating {
                RouteDecision::Block
            } else {
                RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
            }
        }
        ActionPolicy::SourceDependent => {
            // FocusPane: allowed if targeting the active floating pane, otherwise blocked.
            if let WmAction::FocusPane { pane_id } = action {
                if floating {
                    // Only allow focus if it targets the active floating pane.
                    let active_floating = session
                        .active_workspace()
                        .and_then(|ws| ws.floating_panes.iter().find(|f| f.is_active))
                        .map(|f| f.pane.id.0);
                    if active_floating == Some(*pane_id) {
                        RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
                    } else {
                        RouteDecision::Block
                    }
                } else {
                    RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
                }
            } else {
                // Other source-dependent actions: defer to source when floating.
                if floating {
                    match source {
                        InteractionSource::Keyboard => RouteDecision::Block,
                        InteractionSource::MouseContent => RouteDecision::Block,
                        InteractionSource::MouseLeftSidebar => RouteDecision::Block,
                    }
                } else {
                    RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
                }
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
///
/// TODO(phase-E): used in regression tests to verify domain-based blocking.
#[allow(dead_code)] // Phase D helper — will be consumed by handlers and tests
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
pub(crate) fn focused_pane_id(state: &AppState) -> Option<u64> {
    state.focused_pane
}

/// Checks whether a specific pane can receive focus from the given source.
///
/// When in floating domain, only the active floating pane can receive focus
/// from mouse/sidebar sources. Keyboard focus changes are blocked entirely
/// (they go through `dispatch_action` which handles policy).
///
/// TODO(phase-E): used in regression tests to verify pane-targeting policy.
#[allow(dead_code)] // Phase D helper — will be consumed by handlers and tests
pub(crate) fn can_focus_pane(
    session: &heca_core::layout::Session,
    source: InteractionSource,
    pane_id: u64,
) -> bool {
    if is_floating_domain(session) {
        // In floating domain, only the active floating pane can receive focus.
        let active_floating = session
            .active_workspace()
            .and_then(|ws| ws.floating_panes.iter().find(|f| f.is_active))
            .map(|f| f.pane.id.0);
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
pub(crate) fn pane_is_floating(session: &heca_core::layout::Session, pane_id: u64) -> bool {
    session
        .active_workspace()
        .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id.0 == pane_id))
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
pub(crate) fn dispatch_action(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &WmAction,
) {
    let intent = InteractionIntent::ActivateAction(action.clone());
    let decision = route_interaction(state, source, intent);

    match decision {
        RouteDecision::Allow(InteractionIntent::ActivateAction(act)) => {
            registry.execute(&act, state);
        }
        RouteDecision::Allow(other_intent) => {
            // Non-action intents (FocusPane, EnterSidebarNav, etc.)
            // are dispatched differently — for now, log and treat as no-op
            // until Phase B wires them.
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] interaction: allowed non-action intent {:?} (dispatch not yet wired)",
                other_intent
            );
        }
        RouteDecision::Block => {
            #[cfg(debug_assertions)]
            eprintln!("[heca] interaction: blocked action {:?}", action);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{LayoutOptions, PaneId, SessionId, Size};
    use heca_core::layout::column::Pane;
    use heca_core::layout::workspace::FloatingPane;
    use heca_core::layout::types::{Point, Rectangle};

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

    /// In Tiled domain, FocusedPaneLocal actions are allowed.
    #[test]
    fn tiled_domain_allows_focused_pane_local() {
        let session = test_session();
        let actions = [WmAction::Float, WmAction::ClosePane, WmAction::RenamePane];
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
            WmAction::FloatAt { pane_id: 0, x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
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

        let actions = [WmAction::Float, WmAction::ClosePane, WmAction::RenamePane];
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
            InteractionIntent::FocusPane { pane_id: 99 },
            InteractionIntent::FocusWorkspace { ws_idx: 0 },
            InteractionIntent::EnterSidebarNav,
            InteractionIntent::StartSidebarDrag { pane_id: 42 },
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
            WmAction::FocusLeft, WmAction::FocusRight, WmAction::FocusUp, WmAction::FocusDown,
            WmAction::NextPane, WmAction::PrevPane, WmAction::FocusToggleLocal, WmAction::FocusToggleGlobal,
            WmAction::SplitHorizontal, WmAction::SplitVertical, WmAction::ZoomColumn,
            WmAction::ResizeIncrease, WmAction::ResizeDecrease,
            WmAction::PaneHeightIncrease, WmAction::PaneHeightDecrease,
            WmAction::SwapLeft, WmAction::SwapRight, WmAction::SwapUp, WmAction::SwapDown,
            WmAction::MovePaneLeft, WmAction::MovePaneRight, WmAction::MoveColumnUp, WmAction::MoveColumnDown,
            WmAction::PaneSelect, WmAction::SwapPane, WmAction::SwapAndFocusPane,
            WmAction::PaneTake, WmAction::PaneTakeAndFocus,
            WmAction::Float, WmAction::ClosePane, WmAction::RenamePane, WmAction::RenameColumn,
            WmAction::SidebarLeft, WmAction::SidebarRight, WmAction::SidebarFocus,
            WmAction::SidebarUp, WmAction::SidebarDown,
            WmAction::SidebarLeftNav, WmAction::SidebarRightNav, WmAction::SidebarExpandToggle,
            WmAction::SidebarCreateWorkspace, WmAction::SidebarCreateColumn,
            WmAction::SidebarSplitInColumn, WmAction::SidebarZoomSelectedColumn,
            WmAction::SidebarDeleteSelected,
            WmAction::CollapseCurrentWorkspace, WmAction::ExpandCurrentWorkspace,
            WmAction::ToggleCurrentWorkspaceCollapsed,
            WmAction::CollapseCurrentColumn, WmAction::ExpandCurrentColumn,
            WmAction::ToggleCurrentColumnCollapsed,
            WmAction::WorkspaceNext, WmAction::WorkspacePrev,
            WmAction::CreateWorkspace, WmAction::RenameWorkspace,
            WmAction::CommandPalette, WmAction::ReloadConfig,
        ];
        for action in &unit_actions {
            let _policy = action_policy(action);
        }

        let param_actions = [
            WmAction::FocusPane { pane_id: 0 },
            WmAction::FocusWorkspace { ws_idx: 0 },
            WmAction::Swap { a_id: 0, b_id: 0 },
            WmAction::Move { pane_id: 0, target_col: 0 },
            WmAction::MovePaneToWorkspace { pane_id: 0, ws_idx: 0 },
            WmAction::MovePaneToColumn { pane_id: 0, ws_idx: 0, col_idx: 0 },
            WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 0, focus: false },
            WmAction::Resize { target: crate::input::ResizeTarget::Column, axis: crate::input::ResizeAxis::X, amount: 0.0 },
            WmAction::ResizeTo { target: crate::input::ResizeTarget::Column, width: 0.0, height: 0.0 },
            WmAction::FloatAt { pane_id: 0, x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
            WmAction::ClosePaneById { pane_id: 0 },
            WmAction::RenameTarget { pane_id: 0, name: String::new() },
            WmAction::SpawnCommand { command: String::new() },
            WmAction::EnterMode { name: String::new() },
            WmAction::AddPaneToColumn { ws_idx: 0, col_idx: 0 },
            WmAction::DeleteColumn { ws_idx: 0, col_idx: 0 },
            WmAction::DeleteWorkspace { ws_idx: 0 },
            WmAction::TakePane { pane_id: 0, focus_after: false },
        ];
        for action in &param_actions {
            let _policy = action_policy(action);
        }

        // Spot-check specific classifications
        assert_eq!(action_policy(&WmAction::FocusLeft), ActionPolicy::TiledOnly);
        assert_eq!(action_policy(&WmAction::Float), ActionPolicy::FocusedPaneLocal);
        assert_eq!(action_policy(&WmAction::ClosePane), ActionPolicy::FocusedPaneLocal);
        assert_eq!(action_policy(&WmAction::CommandPalette), ActionPolicy::AlwaysAllowed);
        assert_eq!(action_policy(&WmAction::WorkspaceNext), ActionPolicy::WorkspaceLevel);
        assert_eq!(action_policy(&WmAction::FocusPane { pane_id: 0 }), ActionPolicy::SourceDependent);
    }

    /// is_floating_domain returns false for default (Tiled) workspace.
    #[test]
    fn is_floating_domain_default_is_tiled() {
        let session = test_session();
        assert!(!is_floating_domain(&session));
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
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: 99 }),
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
        assert!(!pane_is_floating(&session, 1));
    }

    /// `pane_is_floating` returns false for non-existent pane.
    #[test]
    fn pane_is_floating_returns_false_for_nonexistent() {
        let session = test_session();
        assert!(!pane_is_floating(&session, 9999));
    }

    /// `can_focus_pane` allows any pane when in tiled domain.
    #[test]
    fn can_focus_pane_allows_any_in_tiled_domain() {
        let session = test_session();
        assert!(can_focus_pane(&session, InteractionSource::Keyboard, 1));
        assert!(can_focus_pane(&session, InteractionSource::MouseContent, 1));
        assert!(can_focus_pane(&session, InteractionSource::MouseLeftSidebar, 1));
    }

    /// `can_focus_pane` blocks non-floating pane when in floating domain.
    #[test]
    fn can_focus_pane_blocks_non_floating_when_floating() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        // In a floating domain, pane 1 (in scrolling columns) is NOT the active floating pane.
        // So can_focus_pane should block it from all sources.
        assert!(!can_focus_pane(&session, InteractionSource::Keyboard, 1));
        assert!(!can_focus_pane(&session, InteractionSource::MouseContent, 1));
        assert!(!can_focus_pane(&session, InteractionSource::MouseLeftSidebar, 1));
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
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: 99 }),
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
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: 1 }),
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
            InteractionIntent::FocusPane { pane_id: 1 },
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
            InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: 1 }),
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

    /// When floating, AlwaysAllowed actions (CommandPalette, ReloadConfig) are blocked
    /// from current UI sources. This confirms the current design decision.
    #[test]
    fn floating_blocks_always_allowed_from_keyboard() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [
            WmAction::CommandPalette,
            WmAction::ReloadConfig,
            WmAction::SpawnCommand { command: String::new() },
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

    /// When floating, FocusedPaneLocal actions (Float, ClosePane, RenamePane) are still allowed.
    #[test]
    fn floating_allows_focused_pane_local_via_keyboard() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        let actions = [
            WmAction::Float,
            WmAction::ClosePane,
            WmAction::RenamePane,
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
        let actions = [WmAction::Float, WmAction::ClosePane];

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
        assert!(can_focus_pane(&session, InteractionSource::Keyboard, 99));
        assert!(can_focus_pane(&session, InteractionSource::MouseContent, 99));
        assert!(can_focus_pane(&session, InteractionSource::MouseLeftSidebar, 99));
    }
}