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
    /// A mounted **component** asking the host for something from inside its `perform`
    /// (`ProviderCx::dispatch`, F003/P085/T353).
    ///
    /// Its own source, not a borrowed one, because it is genuinely a different actor: the request
    /// did not come from a device, and a component is the one caller the host must be able to judge
    /// separately from the user driving it.
    Provider,
    /// A scripted call over the RPC surface (F003/P086/T372).
    ///
    /// Its own source for the same reason `Provider` is: a script is a different actor from a
    /// device, and it is judged by the same policy as everyone else — a `TiledOnly` action called
    /// while a float owns the domain is blocked, exactly as the keypress would be.
    Rpc,
    /// **A layer's own retained tree**, acting on itself — the exposé's `x` on a card, a plugin
    /// panel's button (F003/P082/T416).
    ///
    /// Its own source because the surface that holds the keyboard is a different actor from the
    /// user typing at the app *behind* it, and [`domain_for`] has to tell them apart. While the
    /// exposé is up, `prefix+j` must not move the focused pane underneath the map — but the map's
    /// own `x` must still delete the card the cursor is on. Both arrive as keys; only the surface
    /// they were declared on separates them.
    ///
    /// The host stamps this when it builds a layer's emitter, so a plugin's layer is judged the
    /// same way without constructing anything: it declares a handler on its widget and the
    /// framework says where the intent came from.
    Surface(crate::chrome::LayerId),
    // Future sources — not implemented yet:
    // MouseRightSidebar,
    // MouseTopMenu,
    // MouseStatusBar,
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
    /// Focus a mounted **container**, then run an action on it — the container half of
    /// [`FocusPaneThenAction`](Self::FocusPaneThenAction), and the way the palette and RPC reach a
    /// component's own verbs (F003/P085/T358).
    ///
    /// A component's selection-dependent actions declare
    /// [`ActionPolicy::ContainerFocused`](ActionPolicy::ContainerFocused), so "delete the row my
    /// cursor is on" is refused unless that dock is the one being driven. That is the rule, not an
    /// obstacle to route around: this intent **satisfies** it rather than bypassing it — it focuses
    /// the container first, exactly as the user would, and the action is then judged by the ordinary
    /// policy in the ordinary domain. Every half is routed on its own.
    ///
    /// `container` is a **mount id** (resolved by `owning_mount` / an explicit `--dock`), so a
    /// component seated twice is acted on in the seating the caller meant.
    FocusContainerThenAction {
        container: String,
        action: ViewIntent,
    },
    /// Focus a specific pane (from sidebar click, content click, or RPC).
    ///
    /// Dispatched to `WmAction::FocusPane` in `dispatch_action`.
    FocusPane { pane_id: PaneId },
    /// Toggle a specific workspace header's collapsed state from the sidebar.
    ToggleWorkspaceCollapsed { ws_idx: usize },
    /// Record where the **exposé's highlight** now is, so the map can reopen there and can return
    /// to it after a trip through another workspace.
    ///
    /// Not an action and not a focus change — moving a highlight around a map is *looking*. It
    /// touches no session state, so it is permitted in every domain including `Overlay`; refusing
    /// it there would make it useless, since the only time it fires is while the map is up.
    ExposeCursor { pane_id: PaneId },
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

/// **What owns the screen right now** — the axis a policy is judged against (F003/P086/T371).
///
/// The other half of the pair: [`ActionPolicy`] says *what kind of act this is*, `Domain` says
/// *what the app is currently doing*. Both are needed, and only the policy half was modelled:
/// `FocusDomain` is `Tiled | Floating`, read off the session, so the six policy values could only
/// ever answer "is a pane floating?". A focused container lives in `chrome_state` and an overlay in
/// `state.layers`, so neither was visible to policy at all — they were handled by ad-hoc checks
/// beside it (a blunt "block everything while a modal is up") or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Domain {
    /// A pane has the keyboard — the ordinary case.
    Tiled,
    /// A floating pane is active.
    Floating,
    /// A **dock** has the keyboard.
    Container,
    /// Something **covers the tiled area**: a modal, or a plugin overlay that declared it obscures
    /// the panes. Acting on panes you cannot see is the thing this exists to stop.
    Overlay,
}

/// Which domain the app is in for an interaction from `source`.
///
/// **Coverage first, then the keyboard, then the session.** Decided explicitly rather than left to
/// match order (F003/P086/T371):
///
/// - an overlay that covers the tiled area wins over everything — whatever else is true, the panes
///   are not what the user is looking at;
/// - a container holding the keyboard decides for **keyboard-driven** interactions (a key, a
///   component's own `perform`, or a script), even with a floating pane active: the user is driving
///   the dock. A mouse click lands *on* something, so it is judged by what it landed on, not by
///   where the keyboard happens to be — and a script lands on nothing, which is precisely why it
///   belongs with the keys rather than with the clicks (F003/P085/T358). Without this, a component's
///   `ContainerFocused` verb was unreachable over RPC *even with its dock focused*, so the door T371
///   closed had no key;
/// - otherwise the session's own `Tiled | Floating`.
pub(crate) fn domain_for(state: &AppState, source: InteractionSource) -> Domain {
    if base_context_is_dormant(
        state.layers.top_modal_id(),
        crate::chrome::content_covered(state),
        source,
    ) {
        return Domain::Overlay;
    }
    let keyboard_driven = matches!(
        source,
        InteractionSource::Keyboard | InteractionSource::Provider | InteractionSource::Rpc
    );
    // **A modal layer holds the keyboard, so no dock does.** `Domain::Container` means "a dock is
    // being driven", and it is what permits a component's own cursor verbs
    // (`workspaces.delete_selected`, the `j`/`k` nav). While a menu, the palette or the exposé is
    // up, the keys belong to *it* — so a key it had no use for must not fall through and drive the
    // dock underneath it. Antonio, 2026-08-10: right-clicking a sidebar row opened its menu and
    // `j`/`k` went on moving the pane cursor behind it.
    //
    // The layer used to swallow every key it did not want, by hand, which is the same rule written
    // in the wrong place: it also ate `q` and `Esc`, which are catalogued actions the host resolves
    // (F004/P084/T400). Here the layer claims only the keyboard, and `ActionPolicy` decides the
    // rest — `Global` actions still run, `ContainerFocused` ones do not.
    let modal_holds_keyboard = state.layers.top_modal_id().is_some();
    if keyboard_driven
        && !modal_holds_keyboard
        && state.chrome_state.focused_container().is_some()
    {
        return Domain::Container;
    }
    session_domain(&state.session)
}

/// **Is the base context — panes, sidebar, floats — dormant?** The coarse half of
/// `docs/surface-compositor.md` §2, and the whole of what F003/P082/T416 changed.
///
/// It used to be a single call to `chrome::content_covered` — a fact about **what is painted
/// over** — used to decide **which context is live**. The two are orthogonal, and the exposé is
/// exactly where they disagree: the map declares `covers_content: false` because you can still see
/// the panes through it, and that geometric truth was also, accidentally, saying "the base context
/// is still live". So `prefix+j` moved the focused pane behind the map, `prefix+p` opened the
/// palette over it, and `prefix+/` picked sidebar rows the user could not see (Antonio,
/// 2026-08-12). §6's invariant names the shape of that bug: if you are special-casing a surface,
/// the surface is mis-modelled.
///
/// So the coarse question is asked coarsely. `modal` on a layer already answers it — it means
/// "this layer takes the keyboard" (`layers.rs`) — and coverage goes back to meaning only what it
/// says: with nothing holding the keyboard, something opaque over the tiled area still means the
/// panes are not what the user is looking at.
///
/// **The one exception is the active surface acting on itself.** The map's `x`, `r` and `d` are its
/// own declarations, dispatched by its own tree, and they arrive as keys exactly like `prefix+j`
/// does — only [`InteractionSource::Surface`] separates them. They fall through and are judged like
/// anyone else's, by the action's declared policy against the session's domain. That is not an
/// allow-list: a plugin's layer is treated identically for whatever actions it declares, which is
/// the point (Antonio: *"each overlay might use its own actions and keybindings so we risk blocking
/// future actions"*).
fn base_context_is_dormant(
    active_context: Option<crate::chrome::LayerId>,
    content_covered: bool,
    source: InteractionSource,
) -> bool {
    match active_context {
        Some(active) => source != InteractionSource::Surface(active),
        None => content_covered,
    }
}

/// The session's own half of the domain — what [`FocusDomain`] already says.
pub(crate) fn session_domain(session: &heca_core::layout::Session) -> Domain {
    match is_floating_domain(session) {
        true => Domain::Floating,
        false => Domain::Tiled,
    }
}

/// Policy classification for a WM action.
///
/// This determines how the action behaves under different [`Domain`]s. Callers should use
/// `dispatch_action()` or `route_interaction()` and never check policy directly — it is `pub`
/// only because a component **declares** one on every action it registers
/// (`ActionMeta::policy`), and a plugin outside this crate must be able to name what it is
/// required to declare (F003/P086/T371 step 6; the plugin-facing crate itself is F003/P017/T9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        WmAction::CommandPalette { .. }
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. } => ActionPolicy::AlwaysAllowed,

        // ── Global: true app-level action, allowed even when Floating ──
        // ReloadConfig reloads config from disk — no tiled/floating layout impact,
        // so it must stay reachable while a floating pane is active (hot-reload).
        WmAction::ReloadConfig => ActionPolicy::Global,
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
        WmAction::CursorTo { .. } => ActionPolicy::ContainerFocused,
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
/// **One layer, two inputs**: the interaction's [`ActionPolicy`] (what kind of act it is) judged
/// against the current [`Domain`] (what owns the screen). Everything the router used to special-case
/// beside the policy system is a domain now (F003/P086/T371):
///
/// - a modal used to be a blunt "block everything" check right here; it is an overlay that **covers
///   the tiled area**, so `Domain::Overlay` blocks the same things — and a *non-modal* plugin
///   overlay that covers the panes gets the same protection, which the old check could not give it;
/// - a focused container had **no gate at all** — its actions were merely hard to reach by key —
///   and is now `Domain::Container`, the only domain `ActionPolicy::ContainerFocused` permits.
///
/// Overlay *control* (`SubmitOverlay` / `CloseOverlay`) never reaches here — it is intercepted in
/// [`dispatch_intent`] before routing — and an overlay's own Esc / Enter / nav reach the widget
/// through the widget-keymap path, not the WM action system.
pub(crate) fn route_interaction(
    state: &AppState,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    route_in_domain(&state.session, domain_for(state, source), source, intent)
}

/// The core policy function — **pure**, so it stays unit-testable without an `AppState` (which
/// cannot be built without a window). [`route_interaction`] computes the [`Domain`] and hands it
/// the truth rather than re-deriving it here.
///
/// `session` is still needed beside the domain: `SourceDependent` asks *which* pane is the active
/// floating one, which is a fact about the session, not about the domain.
///
/// # What each domain permits
///
/// | policy | `Tiled` | `Floating` | `Container` | `Overlay` |
/// |---|---|---|---|---|
/// | `Global` | ✓ | ✓ | ✓ | ✓ |
/// | `AlwaysAllowed` | ✓ | ✗ | ✓ | ✗ |
/// | `FocusedPaneLocal` | ✓ | ✓ | ✓ | ✗ |
/// | `TiledOnly` | ✓ | ✗ | ✓ | ✗ |
/// | `WorkspaceLevel` | ✓ | ✗ | ✓ | ✗ |
/// | `ContainerFocused` | ✗ | ✗ | ✓ | ✗ |
/// | `SourceDependent` | ✓ | the active floating pane only | ✓ | ✗ |
///
/// `Container` permits what `Tiled` does **plus** `ContainerFocused`: a dock holding the keyboard
/// does not stop `prefix+Enter` from splitting the pane you last worked in, which is the rule the
/// phase settled. `Overlay` permits only `Global`, so `reload_config` keeps working and nothing
/// touches panes the user cannot see.
pub(crate) fn route_in_domain(
    session: &heca_core::layout::Session,
    domain: Domain,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    // The three carrier intents below are the mouse's own vocabulary — focus this pane, fold this
    // workspace, start this drag. None of them is meaningful while a floating pane owns the domain
    // or while something covers the panes, and each has always said so; `blocked_domain` is that
    // one condition written once instead of three times.
    let blocked_domain = matches!(domain, Domain::Floating | Domain::Overlay);
    match &intent {
        InteractionIntent::ActivateAction(action) => route_action(session, domain, source, action),
        // Defensive: `dispatch_intent` expands this into FocusPane + the action before
        // routing, so the router should not normally see it. If it does, route by the
        // inner action's policy (the focus half is always benign).
        InteractionIntent::FocusPaneThenAction { action, .. } => {
            route_action(session, domain, source, action)
        }
        // Likewise expanded before routing. Defensively it is a `View` intent: the name resolves to
        // its real policy when `dispatch_view_intent` looks it up.
        InteractionIntent::FocusContainerThenAction { .. } => RouteDecision::Allow(intent),
        // Pure UI bookkeeping: it changes nothing anyone could be protected from, and the only
        // time it fires is while a map covers the panes — so refusing it in `Overlay` would refuse
        // it always.
        InteractionIntent::ExposeCursor { .. } => RouteDecision::Allow(intent),
        // FocusPane from mouse content/sidebar: only the active floating pane can receive focus in
        // the floating domain, and nothing behind an overlay can.
        InteractionIntent::FocusPane { .. }
        | InteractionIntent::ToggleWorkspaceCollapsed { .. }
        | InteractionIntent::StartSidebarDrag { .. } => match blocked_domain {
            true => RouteDecision::Block,
            false => RouteDecision::Allow(intent),
        },
        // A View intent is a pass-through here: it carries only an action *name*, so its
        // real focus-domain policy is applied when `dispatch_view_intent` resolves it to a
        // `WmAction` and re-dispatches through `dispatch_action` (which routes it).
        InteractionIntent::View(_) => RouteDecision::Allow(intent),
    }
}

/// Route a WmAction based on the current focus domain and interaction source.
fn route_action(
    session: &heca_core::layout::Session,
    domain: Domain,
    source: InteractionSource,
    action: &WmAction,
) -> RouteDecision {
    if policy_allows(session, domain, source, action_policy(action), Some(action)) {
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
    domain: Domain,
    source: InteractionSource,
    policy: ActionPolicy,
    action: Option<&WmAction>,
) -> bool {
    // Nothing but a true global act reaches past something covering the panes — acting on what the
    // user cannot see is the whole reason this domain exists (F003/P086/T371). It replaces the
    // blunt "a modal blocks everything" check the router did before policy was consulted at all,
    // and unlike that one it also covers a non-modal overlay that declared it obscures the panes.
    if domain == Domain::Overlay {
        return policy == ActionPolicy::Global;
    }
    let floating = domain == Domain::Floating;

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
        // A component's own selection-dependent verb: only while a dock is being driven. This is
        // what makes the palette and RPC paths safe without the component declaring anything about
        // where it can be reached from.
        ActionPolicy::ContainerFocused => domain == Domain::Container,
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
                    // A component gets no more reach than the user driving it: if the same request
                    // would be blocked from a key while a pane is floating, asking for it from
                    // inside `perform` must be blocked too.
                    InteractionSource::Provider => false,
                    // Nor does a script (F003/P086/T372).
                    InteractionSource::Rpc => false,
                    // Nor a surface acting on itself: a layer is a place a declaration was made,
                    // not a licence to reach past the floating domain (F003/P082/T416).
                    InteractionSource::Surface(_) => false,
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
            // A component asking to focus a pane is the sidebar's "activate this row" in another
            // shape — allowed in the tiled domain like every other source, as is a script's. A
            // layer's own tree is the same act again: choosing a card in the exposé.
            InteractionSource::Provider
            | InteractionSource::Rpc
            | InteractionSource::Surface(_) => true,
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
        // Bare — a bound key — means the front-most visible modal layer. A named host layer (the
        // exposé) has no completion to resolve, so it is simply hidden; an overlay with one is
        // resolved as a dismissal, which is what pops it and runs its completion.
        let overlay = match overlay {
            Some(id) => id,
            None => match state.layers.top_modal_id() {
                Some(id) => crate::chrome::OverlayId(id),
                None => return,
            },
        };
        crate::chrome::resolve_overlay(state, registry, overlay, crate::chrome::ModalResult::Dismissed);
        return;
    }

    // The container half of the same composite. Its outcome is dropped here — a click and a menu
    // pick have nowhere to report one — while RPC calls the same function for the answer.
    if let InteractionIntent::FocusContainerThenAction { container, action } = &intent {
        let (container, action) = (container.clone(), action.clone());
        focus_container_then_action(state, registry, source, &container, &action);
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
        // Both composites are expanded before routing, so neither reaches here in practice;
        // handled defensively as focus-then-act rather than silently dropped.
        RouteDecision::Allow(InteractionIntent::FocusContainerThenAction { container, action }) => {
            focus_container_then_action(state, registry, source, &container, &action);
        }
        RouteDecision::Allow(InteractionIntent::FocusPaneThenAction { pane_id, action }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
            registry.execute(&action, state);
        }
        RouteDecision::Allow(InteractionIntent::FocusPane { pane_id }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
        }
        RouteDecision::Allow(InteractionIntent::ExposeCursor { pane_id }) => {
            crate::chrome::record_expose_cursor(state, pane_id);
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

/// Focus `container`, then dispatch `action` at it — the one implementation behind
/// [`InteractionIntent::FocusContainerThenAction`], shared by the palette and RPC
/// (F003/P085/T358).
///
/// **The focus half is skipped when that container already holds the keyboard**, and this is not an
/// optimization: `focus_dock` aimed at the focused dock is deliberately a *toggle* (it is the way
/// back out, F003/P085/T352), so focusing unconditionally would release the keyboard and leave the
/// action to be refused by the very policy this exists to satisfy.
///
/// Returns what became of the **action** — the focus half is a means, not the answer a caller asked
/// for. A container that is not mounted is [`IntentOutcome::NotRunnable`]: nothing is focused and
/// nothing runs, rather than the action falling through to whatever else declares that name.
pub(crate) fn focus_container_then_action(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    container: &str,
    action: &ViewIntent,
) -> IntentOutcome {
    if state.chrome_host.provider(container).is_none() {
        #[cfg(debug_assertions)]
        eprintln!("[heca] interaction: no container mounted as '{container}'");
        return IntentOutcome::NotRunnable;
    }
    if state.chrome_state.focused_container().as_deref() != Some(container) {
        dispatch_action(
            state,
            registry,
            source,
            &WmAction::FocusDock {
                dock: Some(container.to_string()),
            },
        );
    }
    dispatch_view_intent(state, registry, source, action)
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
pub(crate) fn dispatch_view_intent(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    intent: &ViewIntent,
) -> IntentOutcome {
    // 0. Judge the args against what the action DECLARES it takes, and say what is wrong. Without
    //    this the two failures below are indistinguishable and both silent: a misspelled required
    //    argument makes `build_action` return `None` (so the intent looks like an unknown action),
    //    and a misspelled optional one is simply dropped, leaving the action to run with a default
    //    nobody asked for.
    let mut args = intent_args_as_strings(intent);
    // **The seating is an address, not an argument.** A row declares its gesture inside one mounted
    // container and names it (`SEAT_ARG`) so nothing has to resolve the call back to an instance;
    // that is the host's business, so it is taken off before the args are judged against what the
    // action declares — otherwise every addressed call would report an argument the action does not
    // take. `route_to_owner` takes it off again before `perform`, and `build_action` below never
    // sees it either.
    args.remove(crate::providers::SEAT_ARG);
    report_arg_problems(&state.action_catalog, &intent.action, &args);

    // 1. Built-in. Parameterized variants are built from the intent's args (`build_action`, the same
    //    constructor a config binding uses); unit variants come straight from the name.
    if let Some(action) = builtin_of(&intent.action, &args) {
        dispatch_action(state, registry, source, &action);
        return IntentOutcome::Ran;
    }

    // A built-in the caller could not build is an **arity** failure, and saying so is the whole
    // point: without this it fell through to the name-keyed branch below, where the catalog does
    // hold its metadata, no dynamic handler exists, and the answer came back `NotRunnable` —
    // "component not mounted?" about one of the app's own compiled-in actions. `heca action
    // add_pane_to_column` said that; the palette listing it said nothing at all (F003/P085/T358).
    if state.action_catalog.is_builtin(&intent.action) {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: '{}' is a built-in whose required arguments were not supplied",
            intent.action
        );
        return IntentOutcome::MissingArgs;
    }

    // 2. **A widget on screen declares it** (F003/P082/T427). Between the built-ins and the
    //    provider catalog, because a surface's own verb is the more specific thing the name means
    //    while that surface is up — the same nearest-declaration rule keys and menus follow.
    //
    //    This is the seam a **layer** has and used not to: a dock declares its actions through
    //    `Provider::actions`, while an overlay could only bind verbs the app had already compiled
    //    in. It is why the exposé's picker had to borrow the built-in `hint_pick`, and why a plugin
    //    could contribute targets to heca's picker but never open one of its own.
    //
    //    Reachability *is* the gate here, and deliberately so: a widget-declared action is found
    //    only by walking the **visible** trees, so an unmounted surface's verb resolves to nothing
    //    exactly as an unmounted provider's does. It needs no policy of its own because it cannot
    //    be reached when its surface is not on screen.
    if crate::chrome::fire_widget_action(state, &intent.action) {
        return IntentOutcome::Ran;
    }

    // 3. Name-keyed (provider/plugin), routed by its DECLARED policy — the same `policy_allows` the
    //    built-in path reaches through `route_action`, so a plugin action is judged by identical
    //    rules.
    let Some(policy) = state.action_catalog.policy(&intent.action) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: view intent '{}' did not resolve to a known action",
            intent.action
        );
        return IntentOutcome::Unknown;
    };

    // The same gate the built-in path takes through `route_action` — a component's or plugin's
    // action is judged by identical rules, on the same domain (F003/P086/T371). The `top_modal`
    // check that used to sit here is gone: a modal covers the tiled area, which `Domain::Overlay`
    // already refuses.
    if !policy_allows(
        &state.session,
        domain_for(state, source),
        source,
        policy,
        None,
    ) {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: blocked dynamic action '{}' from {source:?}",
            intent.action
        );
        return IntentOutcome::Blocked;
    }
    if !registry.execute_dynamic(&intent.action, state, intent) {
        // Declared but host-unrunnable (`Dispatch::Declarative`): its owner lives across the plugin
        // boundary and forwarding lands with the WASM bridge (plugin-08). Never a crash.
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: action '{}' is declared but has no host handler",
            intent.action
        );
        return IntentOutcome::NotRunnable;
    }
    IntentOutcome::Ran
}

/// **The `WmAction` a view-intent name means**, built from its args exactly as a `config.toml`
/// binding is.
///
/// Extracted so that *running* an intent and *judging* one resolve it the same way. Two copies of
/// this line would be two answers to "what does this name mean", and the one nobody exercises is
/// the one that drifts.
fn builtin_of(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    crate::input::build_action(name, args).or_else(|| crate::input::action_from_name(name))
}

/// **Would this intent be allowed to run right now?** — asked *before* a picker spends a letter on
/// it (F003/P082/T432).
///
/// A picker that offers letters which do nothing is a broken picker. With a floating pane active,
/// `prefix+/` lettered every pane and every sidebar row naming one, and pressing a letter did
/// nothing at all, because `ActionPolicy` correctly refuses a `FocusPane` that does not target the
/// active float. This is that refusal, asked one moment earlier.
///
/// It is deliberately the **same three arms** [`dispatch_view_intent`] resolves, in the same order,
/// and it shares [`builtin_of`] with it so the two cannot disagree about what a name means:
///
/// 1. a **built-in** → judged by [`route_interaction`], the one gate every surface goes through;
/// 2. a **name-keyed** action → judged by its declared policy, the same [`policy_allows`] call the
///    dispatcher makes;
/// 3. anything else → **allowed**. A verb declared by a widget on screen has no policy of its own
///    (reachability is its gate — it cannot be found when its surface is not visible), and a name
///    nothing knows is not this function's to refuse. `true` here means *"nothing to ask"*, never
///    *"permitted"*: withholding letters from everything the policy cannot see would be a worse
///    picker than the one this fixes.
pub(crate) fn view_intent_allowed(
    state: &AppState,
    source: InteractionSource,
    intent: &ViewIntent,
) -> bool {
    let mut args = intent_args_as_strings(intent);
    args.remove(crate::providers::SEAT_ARG);
    if let Some(action) = builtin_of(&intent.action, &args) {
        return matches!(
            route_interaction(state, source, InteractionIntent::ActivateAction(action)),
            RouteDecision::Allow(_)
        );
    }
    match state.action_catalog.policy(&intent.action) {
        Some(policy) => policy_allows(
            &state.session,
            domain_for(state, source),
            source,
            policy,
            None,
        ),
        None => true,
    }
}

/// What became of a dispatched intent — the answer a **scripted** caller needs (F003/P086/T372).
///
/// A click can afford to fail silently; a script cannot be told "ok" when nothing happened. The
/// three failures are genuinely different: a name nothing knows, a name the domain refuses right
/// now, and an action whose owner is not mounted (or lives across the plugin boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntentOutcome {
    /// Dispatched — a built-in ran, or a name-keyed handler did.
    Ran,
    /// No such action, in the built-ins or the catalog.
    Unknown,
    /// Known, but its policy does not permit it in the current domain.
    Blocked,
    /// A **built-in whose required arguments were not supplied** — `close_pane_by_id` with no
    /// `pane_id`, `add_pane_to_column` with no column. Its own answer, because the caller's mistake
    /// is fixable and none of the other three say what is wrong: the name is real, the domain has
    /// no opinion, and the action is perfectly runnable with arguments (F003/P085/T358).
    MissingArgs,
    /// Declared, but nothing here can run it: its component is not mounted, or it is a plugin's to
    /// run across a boundary that does not exist yet.
    NotRunnable,
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
        ];
        for action in &actions {
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
            WmAction::SidebarLeft,
            WmAction::WorkspaceNext,
            WmAction::CommandPalette { mode: None, query: None },
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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

    /// **The chain a `prefix+/` candidate is judged by, end to end** (F003/P082/T432).
    ///
    /// Antonio, driving 2026-08-18: with a floating pane active, `prefix+/` lettered every pane and
    /// every sidebar row naming one, and pressing a letter did **nothing**. This is why — and now
    /// it is asked one moment earlier, so the letter is never offered.
    ///
    /// The test walks the real links: the intent a pane declares → the `WmAction` it resolves to →
    /// the routing decision. `view_intent_allowed` is the same three steps with an `AppState` to
    /// supply the domain, which a test cannot build (it needs a window), so the pure half is held
    /// here and the pane's half of the declaration is held by
    /// `chrome::pane::shell::tests::the_pane_says_what_picking_it_would_do`.
    #[test]
    fn a_pick_that_would_be_refused_resolves_to_a_refused_action() {
        let mut session = test_session();
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;

        // What `PaneShell` declares — the same name and argument, built the same way a config
        // binding is.
        let args = std::collections::HashMap::from([("pane_id".to_string(), "99".to_string())]);
        let action = builtin_of("focus_pane", &args).expect("focus_pane is a built-in");
        assert_eq!(action, WmAction::FocusPane { pane_id: PaneId(99) });

        let decision = route_in_domain(
            &session,
            session_domain(&session),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "a pane that is not the active float cannot be focused, so its letter would do nothing"
        );
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
            InteractionIntent::StartSidebarDrag {
                pane_id: PaneId(42),
            },
        ];
        for intent in &intents {
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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

    /// **The user's case** (2026-07-29): a plugin opens a non-modal overlay over the scrolling area,
    /// and `prefix+Enter` must not add a pane behind it. No plugin declares anything about
    /// `split_horizontal` — it declares that its overlay covers the panes, and every `TiledOnly`
    /// act falls away with no further rule (F003/P086/T371).
    #[test]
    fn an_overlay_covering_the_panes_blocks_acting_on_them() {
        let session = test_session();
        for action in [
            WmAction::SplitHorizontal,
            WmAction::ZoomColumn,
            WmAction::ClosePane,
            WmAction::FocusLeft,
            WmAction::CreateWorkspace,
        ] {
            let decision = route_in_domain(
                &session,
                Domain::Overlay,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Block),
                "{action:?} must not reach a pane the user cannot see, got {decision:?}",
            );
        }
        // …and a true global act still gets through, or a covered screen would also mean no
        // `reload_config`.
        assert!(matches!(
            route_in_domain(
                &session,
                Domain::Overlay,
                InteractionSource::Keyboard,
                InteractionIntent::ActivateAction(WmAction::ReloadConfig),
            ),
            RouteDecision::Allow(_),
        ));
    }

    /// A layer id for the tests. Ids are opaque and this module only needs two that differ.
    fn layer(n: u64) -> crate::chrome::LayerId {
        crate::chrome::LayerId::for_test(n)
    }

    /// **The exposé's defect, as a rule.** The map declares `covers_content: false` — you can see
    /// the panes through it, and that is geometrically true — so the old gate said the base context
    /// was still live and `prefix+j` drove the session behind it (Antonio, 2026-08-12). What makes
    /// a context active is that it took the keyboard, not what it painted over.
    #[test]
    fn a_surface_that_took_the_keyboard_makes_the_base_context_dormant_even_if_it_covers_nothing() {
        assert!(base_context_is_dormant(
            Some(layer(1)),
            false, // covers nothing — a map of the panes is not a lid over them
            InteractionSource::Keyboard,
        ));
    }

    /// The exception that keeps T327's deletes working, and the one a plugin's layer relies on:
    /// the surface holding the keyboard is not "the app being driven behind it". `x` on a card is
    /// the map's own declaration and is judged by the action's policy, like anyone else's.
    #[test]
    fn the_active_surface_acting_on_itself_is_not_refused() {
        assert!(!base_context_is_dormant(
            Some(layer(1)),
            false,
            InteractionSource::Surface(layer(1)),
        ));
    }

    /// …and it is the *active* surface, not any surface. A layer that no longer holds the keyboard
    /// — one beneath the dialog that opened over it — gets no reach past it.
    #[test]
    fn a_surface_underneath_the_active_one_gets_no_reach() {
        assert!(base_context_is_dormant(
            Some(layer(2)),
            false,
            InteractionSource::Surface(layer(1)),
        ));
    }

    /// With nothing holding the keyboard, coverage still means what it says: something opaque over
    /// the tiled area means the panes are not what the user is looking at.
    #[test]
    fn coverage_still_decides_when_no_surface_holds_the_keyboard() {
        assert!(base_context_is_dormant(None, true, InteractionSource::Keyboard));
        assert!(!base_context_is_dormant(None, false, InteractionSource::Keyboard));
    }

    /// A component's cursor verb is reachable **only while its dock is being driven** — which is
    /// what closes the palette/RPC hole those actions had (F003/P086/T371).
    #[test]
    fn a_container_focused_action_needs_a_focused_container() {
        let session = test_session();
        assert!(policy_allows(
            &session,
            Domain::Container,
            InteractionSource::Keyboard,
            ActionPolicy::ContainerFocused,
            None,
        ));
        for domain in [Domain::Tiled, Domain::Floating, Domain::Overlay] {
            assert!(
                !policy_allows(
                    &session,
                    domain,
                    InteractionSource::Keyboard,
                    ActionPolicy::ContainerFocused,
                    None,
                ),
                "{domain:?} is not a dock being driven",
            );
        }
    }

    /// **A script is judged like a key, not like a click** (F003/P085/T358).
    ///
    /// A click lands *on* something and is judged by what it landed on; a script lands on nothing,
    /// so where the keyboard is *is* the context. Without this, `heca action workspaces.delete_row`
    /// was refused even with the workspaces dock focused — the policy T371 introduced had no door at
    /// all from RPC, which is what `FocusContainerThenAction` exists to open.
    #[test]
    fn a_script_is_judged_where_the_keyboard_is() {
        let session = test_session();
        for source in [
            InteractionSource::Rpc,
            InteractionSource::Keyboard,
            InteractionSource::Provider,
        ] {
            assert!(
                policy_allows(
                    &session,
                    Domain::Container,
                    source,
                    ActionPolicy::ContainerFocused,
                    None,
                ),
                "{source:?} drives the focused dock",
            );
        }
        // And it buys a script nothing else: with no dock focused it is refused exactly as a
        // keypress is, which is why focusing first is the composite's job and not a special case.
        assert!(!policy_allows(
            &session,
            Domain::Tiled,
            InteractionSource::Rpc,
            ActionPolicy::ContainerFocused,
            None,
        ));
    }

    /// The composite is expanded before routing, so the router only ever sees it defensively — and
    /// passes it through, because the action half carries a *name* whose real policy is read when
    /// `dispatch_view_intent` resolves it.
    #[test]
    fn the_focus_container_composite_passes_through_the_router() {
        let session = test_session();
        for domain in [Domain::Tiled, Domain::Container, Domain::Overlay] {
            assert!(
                matches!(
                    route_in_domain(
                        &session,
                        domain,
                        InteractionSource::Keyboard,
                        InteractionIntent::FocusContainerThenAction {
                            container: "workspaces".to_string(),
                            action: ViewIntent::new("workspaces.delete_row"),
                        },
                    ),
                    RouteDecision::Allow(InteractionIntent::FocusContainerThenAction { .. }),
                ),
                "{domain:?}: the name is judged on resolution, not here",
            );
        }
    }

    /// A dock holding the keyboard does not stop the app's own tiled actions — the phase's rule
    /// that `prefix+…` keeps working while a container is focused, expressed as a domain.
    #[test]
    fn a_focused_container_still_permits_the_tiled_actions() {
        let session = test_session();
        for policy in [
            ActionPolicy::TiledOnly,
            ActionPolicy::WorkspaceLevel,
            ActionPolicy::AlwaysAllowed,
            ActionPolicy::FocusedPaneLocal,
            ActionPolicy::Global,
        ] {
            assert!(
                policy_allows(
                    &session,
                    Domain::Container,
                    InteractionSource::Keyboard,
                    policy,
                    None,
                ),
                "{policy:?} should still run while a dock has the keyboard",
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
            WmAction::CommandPalette { mode: None, query: None },
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
        // Surface-agnostic: it asks whatever owns the screen for its menu, and bubbling decides
        // whose that is (F003/P082/T416).
        assert_eq!(
            action_policy(&WmAction::OpenContextMenu),
            ActionPolicy::Global
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
            action_policy(&WmAction::CommandPalette { mode: None, query: None }),
            ActionPolicy::AlwaysAllowed
        );
        assert_eq!(action_policy(&WmAction::ReloadConfig), ActionPolicy::Global);
        // **Taking** chrome focus is tiled-only; **releasing** it is always allowed. A focused
        // container's own `activate`/`hint` move pane focus and are reached through the first, so it
        // must not open while a float owns the domain — but a way out that can be blocked is not a
        // way out (F003/P085/T356).
        assert_eq!(
            action_policy(&WmAction::FocusDock { dock: None }),
            ActionPolicy::TiledOnly
        );
        assert_eq!(
            action_policy(&WmAction::FocusDock {
                dock: Some("workspaces".into())
            }),
            ActionPolicy::TiledOnly
        );
        assert_eq!(action_policy(&WmAction::UnfocusDock), ActionPolicy::Global);
        // The horizontal four reach a chrome container's scroll area only, so they follow chrome
        // focus rather than the pane's tiled/floating domain.
        for action in [
            WmAction::ScrollPageLeft,
            WmAction::ScrollPageRight,
            WmAction::ScrollToLeftEdge,
            WmAction::ScrollToRightEdge,
        ] {
            assert_eq!(action_policy(&action), ActionPolicy::Global, "{action:?}");
        }
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
            Domain::Tiled,
            InteractionSource::Keyboard,
            ActionPolicy::TiledOnly,
            None
        ));
        // Floating: blocked.
        session.active_workspace_mut().unwrap().focus_domain = FocusDomain::Floating;
        assert!(!policy_allows(
            &session,
            session_domain(&session),
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
            session_domain(&session),
            InteractionSource::Keyboard,
            ActionPolicy::Global,
            None
        ));
        assert!(
            !policy_allows(
                &session,
                session_domain(&session),
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
            session_domain(&session),
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
        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
            WmAction::SidebarLeft,
        ];
        for action in &actions {
            // Test via MouseLeftSidebar source (same result as Keyboard, but testing the source explicitly)
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
        let decision = route_in_domain(
            &session,
            session_domain(&session),
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

        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
            WmAction::SidebarLeft,
            WmAction::SidebarRight,
        ];
        for action in &actions {
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
            WmAction::CommandPalette { mode: None, query: None },
            WmAction::SpawnCommand {
                command: String::new(),
                kind: crate::input::SpawnKind::Terminal,
                float: false,
                close_policy: heca_core::runtime::PaneClosePolicy::default(),
            },
        ];
        for action in &actions {
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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

        let decision = route_in_domain(
            &session,
            session_domain(&session),
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
            let decision = route_in_domain(
                &session,
                session_domain(&session),
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
                let decision = route_in_domain(
                    &session,
                    session_domain(&session),
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
    /// Sidebar navigation intent is allowed when tiled.
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
