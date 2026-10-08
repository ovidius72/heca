//! The vocabulary of an interaction: who asked (`InteractionSource`, `SurfaceKey`), what they asked (`InteractionIntent`), and the router's answer (`RouteDecision`).

use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use heca_core::layout::PaneId;


/// **Which surface an intent was declared on** — the identity
/// [`InteractionSource::Surface`] carries.
///
/// Derived from the surface's own **key**, never handed out. That matters because a surface is
/// moving from the layer registry into the one retained tree (`docs/surface-compositor.md` § 0.8):
/// a `LayerId` stops existing the moment it is a node, while the key it declares on itself survives
/// the move — and is the same identity the picker, the drag registry and a row's `key` already use.
///
/// **Nobody constructs one to get a capability.** The host derives it when it builds a surface's
/// emitter, exactly as it stamped the id before, so a plugin declares a handler on its widget and
/// the framework says where the intent came from (⭐⭐ RULE ZERO).
///
/// A `u64` rather than the string so [`InteractionSource`] stays `Copy` — it is stored on the
/// retained chrome and copied through 170-odd call sites, and making it allocate to answer "who
/// asked" would be a heavy price for an equality check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceKey(u64);

impl SurfaceKey {
    /// The key a surface declares on itself, reduced to the value the policy compares.
    pub(crate) fn of(key: &str) -> Self {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        Self(h.finish())
    }
}

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
    /// The host stamps this when it builds a surface's emitter, so a plugin's surface is judged the
    /// same way without constructing anything: it declares a handler on its widget and the
    /// framework says where the intent came from.
    ///
    /// It carries the surface's [`SurfaceKey`] rather than a registry id, because a surface that
    /// has moved into the one retained tree has no registry id to be named by — only the key it
    /// declares on itself.
    Surface(SurfaceKey),
    // Future sources — not implemented yet:
    // MouseRightSidebar,
    // MouseTopMenu,
    // MouseStatusBar,
}

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
    #[expect(
        dead_code,
        reason = "constructed from mouse/sidebar in future wiring pass"
    )]
    StartSidebarDrag { pane_id: PaneId },
    /// A declarative [`ViewNode`](crate::chrome::ViewNode) intent — the universal
    /// invocation currency for click / KeyHint / RPC / plugin (plan §2.7.2, "everything
    /// is an action"). Carries a `view::Intent { action, args }`; `dispatch_intent`
    /// resolves it via [`dispatch_view_intent`] (name → `WmAction` → policy-routed
    /// dispatch). Constructed by [`realize`](crate::chrome::realize) for actionable nodes.
    View(ViewIntent),
}

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
