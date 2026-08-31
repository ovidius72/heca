//! Chrome metrics and central application constants.
//!
//! UI dimensions, timing defaults, layout proportions, and render parameters
//! that were previously scattered as magic numbers across the codebase.

pub(crate) mod theme;
pub(crate) use theme::{
    alpha_u8, apply_pane_frame, chrome_colors, chrome_gui_theme, chrome_status,
    left_sidebar_shell_background_color, right_sidebar_shell_background_color,
};
pub(crate) mod signals;
pub(crate) use signals::{sync_chrome_signals, sync_chrome_state, ChromeSignals};
pub(crate) mod pane;
pub(crate) use pane::{clear_panes, sync_panes, RetainedPane};
pub(crate) mod pane_header;
pub(crate) use pane_header::{
    action_tooltip, clear_pane_headers, home_relative_path, pane_info_view, sync_pane_headers,
    sync_pane_viewport_widgets, truncate_path_left, ActionShortcuts, PaneInfoSignals,
    RetainedPaneHeader, RetainedPaneViewportWidgets, CARD_META_FONT_SCALE,
};
pub(crate) mod drag;
pub(crate) use drag::{ChromeDragItem, DragItemRegistry};
pub(crate) mod hint;
// The hint half of the chrome — declarations, the visibility rule, and who is lettering what.
// Re-exported so call sites keep naming `crate::chrome::…` while the code lives where it belongs.
pub(crate) use hint::{
    active_hint_targets, clear_hint_letters, fire_hint, fire_widget_action,
    target_identity,
    HintTarget,
};
mod contribution;
pub(crate) mod context_menu;
mod events;
mod expose;
pub(crate) use expose::register as register_expose;

mod focus;
mod dispatch;
mod scene;
#[allow(unused_imports)]
use scene::{
    build_region_content, build_sidebar_shell, pass_box_down, search_bar_tree, search_field_slot,
    sidebar_toggle_button, with_share, ChromeFrame,
};
#[cfg(test)]
use scene::chrome_scene;
pub(crate) use scene::{
    build_chrome_root, paint_bell_flash, paint_chrome_root, paint_drag_overlay, paint_link_hints,
    paint_search,
};
pub(crate) use dispatch::{
    chrome_dispatch_button_press, chrome_dispatch_button_release, chrome_dispatch_cancelled,
    chrome_dispatch_move, chrome_dispatch_press, chrome_dispatch_release, chrome_dispatch_wheel,
    chrome_dispatch_widget, dispatch_modal_pointer, dispatch_pane_header_move,
    dispatch_pane_header_press, dispatch_pane_header_release, dispatch_pane_header_wheel,
    dispatch_pane_viewport_move, dispatch_pane_viewport_press, dispatch_pane_viewport_release,
    drain_pending_menus, open_declared_menu_for_focus,
};
mod layers_glue;
pub(crate) use layers_glue::{layout_layers, paint_layers, rebuild_named_layer};
mod notification_layer;
pub(crate) use notification_layer::{
    mount_notification_stack, surface_key as notification_surface_key,
};
mod host;
/// The identity rule's reporting half (F003/P082/T444) — see the module docs.
mod identity;
mod layers;
mod overlay;
mod palette;
mod state;
// Registry API surface consumed by the next migration steps (ShowLayer/HideLayer, the
// confirm dialog as a layer, plugins) — some names not yet referenced in-binary.
#[allow(unused_imports)]
pub(crate) use expose::record_expose_cursor;
pub(crate) use layers::{surface_key_of, LayerId, LayerKind, LayerRegistry};
// Declarative UI model (plugin-task-ui-1); consumed by `realize` (ui-3) + Modal body (ui-4).
// It lives in the `heca-view` crate since F003/P017/T009 — a plugin depends on that crate, and it
// cannot depend on this binary. Re-exported here so the app keeps one path to the vocabulary.
#[allow(unused_imports)]
pub(crate) use heca_view::{
    Intent, PropMap, PropValue, ViewAlign, ViewNode, ViewSize, ViewVariant, WidgetKind,
};
// The mapper (plugin-task-ui-3): `ViewNode` → retained grid-ui `Component`. It lives in
// `heca-view-realize` since F003/P017/T009 — below this crate, so anything that can build widgets
// can render a described tree, the showcase included. The app supplies the two seams it takes:
// `ViewHintTargets` for pick registration, a wrapping closure for the click sink.
#[allow(unused_imports)]
pub(crate) use heca_view_realize::{realize, FormBindings, IntentEmitter};
// Host-owned overlay stack (plugin-task-ui-4, §2.7.1/§2.7.2), built on `LayerRegistry`.
// `OverlayId` is pub (carried by `WmAction`); the rest is crate-internal.
pub use overlay::OverlayId;
#[allow(unused_imports)]
pub(crate) use overlay::{
    collect_form as collect_overlay_form, open_dropdown, open_modal, resolve as resolve_overlay,
    content_covered, top_modal, DropdownItem, DropdownSpec, ModalAction, ModalResult, ModalSpec,
    OverlayHost,
};
// Context-menu resolution: ContextPath + ContextTarget + ContextMenuRegistry + the unified
// `open_context_menu_for`. Built-in providers seeded at startup; a provider attaches its own
// entries via `Provider::context_menus` (context-menu-5).
#[allow(unused_imports)]
pub(crate) use context_menu::{
    open_context_menu_for, resolve_active_context, ContextMenuProvider, ContextMenuRegistry,
    ContextPath, ContextTarget,
};
pub use context_menu::MenuBuild;
// The command palette: every registered action, searchable, dispatched through the one door
// (F003/P085/T358).
pub(crate) use palette::open_command_palette;
pub use contribution::{ContextMenuContribution, Contribution, RegionSet};
pub use events::{ChromeEvent, ChromeEventBus, ChromeSubscription, RegionId, SidebarSelection};
// Chrome keyboard focus: which dock the keyboard is aimed at (F003/P011/T020).
pub(crate) use focus::{dock_candidates, navigable_dock, placement_for, region_on_screen};
pub use host::ChromeHost;
pub use state::{SharedChromeState, WorkspacesContainerState};
// Contribution/placement API surface for the render + provider phases (plugin-03).
// These are public seam types not yet consumed by name in-binary — same rationale
// as the `#![allow(dead_code)]` carried by the modules that define them.
// `MoveError` is `ChromeHost::move_container`'s error; handlers report it via
// `Display` today and the mouse/DnD path names it in plugin-03.
#[allow(unused_imports)]
pub use contribution::{
    BuildBody, BuildCx, ContainerContribution, ContainerId, OverlaySpec, PanelContribution,
    StatusSegment, ToolbarGroup, WidgetModel,
};
#[allow(unused_imports)]
pub use host::{MountedContribution, MoveError, RegionHost};

use heca_core::layout::ColumnWidth;
use heca_core::layout::types::{Point, Rectangle, Size};
use std::time::Duration;

// ── Chrome metrics ──

/// Default tab bar height in logical pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 32.0;
/// Default status bar height in logical pixels.
pub const DEFAULT_STATUS_BAR_HEIGHT: f32 = 24.0;
/// Default expanded sidebar width in logical pixels.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;

// ── Timing ──

/// Prefix mode auto-exit timeout (ms). After this time with no key, prefix mode cancels.
pub const PREFIX_TIMEOUT: Duration = Duration::from_millis(500);
/// Target frame interval (~60 FPS).
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

// ── Layout defaults ──

/// Default width proportion for newly created columns.
pub(crate) const DEFAULT_COLUMN_PROPORTION: f64 = 0.5;
/// Helper to get the default ColumnWidth for new columns.
pub const fn default_column_width() -> ColumnWidth {
    ColumnWidth::Proportion(DEFAULT_COLUMN_PROPORTION)
}

// ── Pane name overlay ──

/// Font size factor for the pane name overlay (fraction of min(width, height)).
pub const PANE_NAME_SIZE_FACTOR: f32 = 0.25;
/// Minimum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MIN: f32 = 24.0;
/// Maximum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MAX: f32 = 72.0;

// ── Chrome text ──

/// Default font size for chrome text (tab bar, status bar).
pub const CHROME_TEXT_SIZE: f32 = 14.0;

// ── Mouse ──

/// Distance from content area edge that triggers edge scrolling (logical pixels).
pub const EDGE_SCROLL_TRIGGER: f32 = 80.0;

/// Layout configuration for chrome elements around the pane area.
#[derive(Clone, Copy, Debug)]
pub struct ChromeConfig {
    pub tab_bar_height: f32,
    pub status_bar_height: f32,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub sidebar_gap: f32,
}

impl ChromeConfig {
    /// Compute the rectangle available for pane content, given a window size.
    /// Chrome occupies the outer edges; panes get the center.
    pub fn content_rect(&self, window_width: f32, window_height: f32) -> Rectangle {
        let sidebar_gap = self.sidebar_gap.max(0.0);
        let x = self.left_sidebar_width
            + if self.left_sidebar_width > 0.0 {
                sidebar_gap
            } else {
                0.0
            };
        let y = self.tab_bar_height;
        let right_reserved = self.right_sidebar_width
            + if self.right_sidebar_width > 0.0 {
                sidebar_gap
            } else {
                0.0
            };
        let w = (window_width - x - right_reserved.min((window_width - x).max(0.0))).max(0.0);
        let h = (window_height - self.tab_bar_height - self.status_bar_height).max(0.0);
        Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(w as f64, h as f64),
        )
    }
}

// ── Grid-UI chrome scene builder ──────────────────────────────────────────────

use crate::providers::workspaces::{PaneEntry, WorkspaceTree};
use heca_config::programs::{ProgramIcon, ProgramsConfig};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::drag::{DragPhase, DragSurfaceId};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, signal};
use heca_grid_ui::style::{Align, Justify, Length, Spacing, WidgetSize};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    BadgeButton, Flex, FocusScope, Glyph, HintPlacement, Icon, IconButton, KeyHint, Label, Pane,
    ScrollBar, Separator, Surface, Tag,
    Tooltip,
    TooltipSide,
};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
use std::rc::Rc;

/// Translate a freshly-laid-out widget subtree (positioned from the origin by
/// [`LayoutEngine::compute`]) to an absolute `(dx, dy)`. Mirrors the helper in
/// `terminal_render` so the retained header can be placed at its pane.
pub(crate) fn translate_tree(c: &mut dyn Component, dx: f64, dy: f64) {
    let b = c.base().bounds;
    c.base_mut().bounds = Rectangle::new(Point::new(b.loc.x + dx, b.loc.y + dy), b.size);
    for child in c.base_mut().children.iter_mut() {
        translate_tree(child.as_mut(), dx, dy);
    }
}


pub(crate) fn runtime_snapshot(state: &WorkspacesContainerState, pane_id: PaneId) -> Option<PaneRuntime> {
    state.pane_runtime(pane_id)
}

// Kept short so the branch + git counts fit the sidebar card width without
// overflowing (the row isn't width-clipped). The full branch is on hover.
const SIDEBAR_GIT_BRANCH_MAX_CHARS: usize = 22;

/// Truncate a branch for the sidebar, keeping the **tail** (the meaningful end,
/// e.g. `…security-upgrade`) rather than the boilerplate `feature/` prefix.
pub(crate) fn truncate_sidebar_git_branch(branch: &str) -> String {
    truncate_path_left(branch, SIDEBAR_GIT_BRANCH_MAX_CHARS)
}

/// How a chrome widget reports a user action back to the app: it emits an
/// [`InteractionIntent`](crate::app::interaction::InteractionIntent), never a direct
/// state mutation. Handed to container providers through
/// [`ChromeCtx::emit_intent`](crate::providers::ChromeCtx::emit_intent).
#[derive(Clone)]
pub(crate) struct ChromeIntentEmitter {
    source: crate::app::interaction::InteractionSource,
    sink: Rc<dyn Fn(crate::app::interaction::InteractionSource, crate::app::interaction::InteractionIntent)>,
}

impl ChromeIntentEmitter {
    /// A sink that stamps everything it posts with `source`.
    pub(crate) fn new(
        event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
        source: crate::app::interaction::InteractionSource,
    ) -> Self {
        let event_proxy = event_proxy.clone();
        Self {
            source,
            sink: Rc::new(move |source, intent| {
                let _ = event_proxy
                    .send_event(crate::app::events::AppEvent::ChromeIntent { source, intent });
            }),
        }
    }

    /// A sink of your own — what a test uses to record what a component emitted.
    #[cfg(test)]
    pub(crate) fn of(
        source: crate::app::interaction::InteractionSource,
        sink: impl Fn(crate::app::interaction::InteractionSource, crate::app::interaction::InteractionIntent)
            + 'static,
    ) -> Self {
        Self { source, sink: Rc::new(sink) }
    }

    /// **Where intents from this tree come from.**
    ///
    /// It is readable, and that is the point: asking *"would this be allowed?"* and *running* it
    /// have to ask the identical question, and they cannot while one of them invents an answer.
    /// The `prefix+/` picker asked as `Keyboard` while the chrome tree dispatches as
    /// `MouseLeftSidebar`, so the two disagreed about the domain — the picker offered letters in
    /// `Container` that execution then refused in `Floating` (Antonio, driving 2026-08-21).
    pub(crate) fn source(&self) -> crate::app::interaction::InteractionSource {
        self.source
    }

    /// Post an intent, stamped with this sink's source.
    pub(crate) fn fire(&self, intent: crate::app::interaction::InteractionIntent) {
        (self.sink)(self.source, intent)
    }
}

/// **The intent sink for one layer's own retained tree** — the exposé's cards, a modal's buttons, a
/// plugin panel's widgets (F003/P082/T416).
///
/// Every intent it posts is stamped [`InteractionSource::Surface`] with that layer's id, which is
/// how [`domain_for`](crate::app::interaction::domain_for) tells "the surface holding the keyboard
/// acted on itself" from "the user typed at the app behind it". The two arrive as the same event
/// and only this stamp separates them.
///
/// One function rather than a closure per call site: there were three, and each had picked a
/// *different* lie about where its intents came from — the exposé claimed `Keyboard`, the modal and
/// the described-layer path claimed `MouseContent`. Three copies of one rule is a missing API
/// (AGENTS ⭐⭐ Rule Zero), and here the copies had already drifted.
///
/// Take the id from [`LayerRegistry::reserve_id`](layers::LayerRegistry::reserve_id) when the tree
/// has to be built before the layer is inserted, which is the usual case.
pub(crate) fn layer_emitter(
    event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    key: crate::app::interaction::SurfaceKey,
) -> ChromeIntentEmitter {
    ChromeIntentEmitter::new(
        event_proxy,
        crate::app::interaction::InteractionSource::Surface(key),
    )
}

/// Transparent wrapper that marks only its own bounds dirty when the host bumps
/// `request`. This lets retained chrome updates damage the specific card/marker/label
/// instead of the entire chrome root.
pub(crate) struct RepaintWatch {
    base: heca_grid_ui::Base,
    request: Signal<u64>,
    seen: u64,
}

impl RepaintWatch {
    pub(crate) fn new(child: impl Component + 'static) -> (Self, Signal<u64>) {
        let mut base = heca_grid_ui::Base::new();
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.style.layout.direction = heca_grid_ui::Direction::Column;
        base.children.push(Box::new(child));
        let request = signal(0_u64);
        (
            Self {
                base,
                request,
                seen: 0,
            },
            request,
        )
    }
}

impl Component for RepaintWatch {
    fn base(&self) -> &heca_grid_ui::Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut heca_grid_ui::Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            // `paint_child` and not `child.paint`: it owns the hidden check *and* draws the child's
            // hint letter, so a custom container gets both by going through the framework
            // (F003/P082/T431).
            heca_grid_ui::paint_child(child.as_ref(), cx);
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let next = self.request.get_untracked();
        if next != self.seen {
            self.seen = next;
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}


/// A **retained** chrome tree + the signature of the state that produced it. The
/// tree is rebuilt only when [`chrome_signature`] changes; otherwise it is just
/// re-laid-out and painted each frame. This keeps the widget signals alive across
/// frames (no per-frame signal churn) and gives a live tree to dispatch events into
/// (F4.2). The collapsed sidebar rail is still hand-drawn in `render.rs`.
pub(crate) struct RetainedChrome {
    pub(crate) sig: u64,
    /// Handles to the tree's **value** signals (selection + status), so they update
    /// in place via [`sync_chrome_signals`] instead of forcing a rebuild.
    pub(crate) signals: ChromeSignals,
    /// Maps each draggable/droppable widget's opaque [`DragItemId`] back to *what it
    /// is* (pane / column / workspace). Populated during [`build_chrome_root`] and
    /// queried by [`sidebar_drag_source`]/[`sidebar_drop_target`].
    pub(crate) drag_items: DragItemRegistry,
    /// **The [`InteractionSource`](crate::app::interaction::InteractionSource) every intent from
    /// this tree is dispatched with** — taken from the emitter that built it, never restated.
    ///
    /// It is here so that *asking whether a gesture would be allowed* and *running it* ask the
    /// identical question. The `prefix+/` picker judged its candidates as `Keyboard` while this
    /// tree dispatches as `MouseLeftSidebar`; the two resolved to different domains, so the picker
    /// offered letters in `Container` that execution then refused in `Floating` — a letter that did
    /// nothing (Antonio, driving 2026-08-21). Guessing the source in the filter was the bug; there
    /// is one authority and this is a copy of it, made at construction.
    pub(crate) intent_source: crate::app::interaction::InteractionSource,
}

/// **A fresh, empty window root** — what [`AppState::window_root`](crate::app_state::AppState)
/// starts as, before any chrome has been built or any surface placed.
///
/// It fills the window and imposes nothing else: the chrome subtree sizes itself in real pixels and
/// a surface placed beside it takes itself out of the flow, so this level changes no geometry. It
/// exists to **outlive** the chrome, not to lay anything out.
pub(crate) fn new_window_root() -> Flex {
    Flex::column()
        .width(Length::Pct(1.0))
        .height(Length::Pct(1.0))
}

/// **The chrome subtree's identity in the window root.** Its slot is found by this, never by
/// position — a surface may be placed before the first chrome is ever built (the toast stack is,
/// at startup), and a positional "child 0" would then seat the chrome *over* it.
pub(crate) const CHROME_KEY: &str = "heca.chrome";

/// **Seat a freshly built chrome subtree in the window root**, keeping every surface beside it.
///
/// Doing it this way rather than replacing the retained tree is the whole point of the extra level.
/// The chrome is rebuilt on a resize, a sidebar toggle and a theme reload — and dropped outright by
/// `reload_config` — all of which happen while an overlay is open. None of them may take it with
/// them.
///
/// It goes **first**, so every surface placed beside it paints and hit-tests above it: child order
/// is z-order in one tree, which is what replaces the layer stack's separate sort
/// (`docs/surface-compositor.md` § 0.6).
pub(crate) fn seat_chrome(root: &mut Flex, chrome: Flex) {
    let chrome = Box::new(chrome.key(CHROME_KEY));
    let children = &mut root.base_mut().children;
    match children
        .iter()
        .position(|c| c.base().key.as_deref() == Some(CHROME_KEY))
    {
        Some(at) => children[at] = chrome,
        None => children.insert(0, chrome),
    }
}

/// **Place a surface in the window root** — the whole of "how do I put something on screen"
/// (`docs/surface-compositor.md` § 0.3).
///
/// It is a child, like any widget: the one walk lays it out, paints it, delivers its pointer events
/// and collects its hint letters, with nothing registered and no dispatch function added for it.
///
/// Positioned out of the flow at the full viewport, so it takes no space from the chrome beside it
/// and places its own content within itself — which is what `at_rect` means and why layers needed
/// no new layout capability to become children.
///
/// Re-placing under the same `key` **replaces** that surface, so a rebuild is a swap rather than a
/// second copy accumulating behind the first.
pub(crate) fn place_surface(root: &mut Flex, key: &str, surface: Box<dyn Component>) {
    let mut surface = surface;
    surface.base_mut().key = Some(key.to_string());
    surface.base_mut().style.layout.placement = Some(heca_grid_ui::style::Placement {
        left: Length::Pct(0.0),
        top: Length::Pct(0.0),
        width: Length::Pct(1.0),
        height: Length::Pct(1.0),
    });
    let children = &mut root.base_mut().children;
    match children
        .iter()
        .position(|c| c.base().key.as_deref() == Some(key))
    {
        Some(at) => children[at] = surface,
        None => children.push(surface),
    }
}

/// `pane:<id>` — **a pane's identity**, declared by the pane itself.
///
/// Whoever owns the thing declares its identity; anything else showing it is a view. A pane owns
/// `pane:7`; the sidebar row and the exposé card that show that pane are second views of it. This
/// lives here rather than in a provider so the pane and every view of it read the SAME string
/// instead of keeping two copies in step (it was defined twice before F011/P094/T451).
///
/// It is never a position and never a counter: a `PaneId` survives every tree rebuild, which is
/// what lets a hint letter stay with the same pane between openings of the picker.
pub(crate) fn pane_key(pane: heca_core::layout::PaneId) -> String {
    format!("pane:{}", pane.0)
}

/// **Fire a named gesture**: the closure that emits `intent` through this surface's chrome sink.
///
/// This is `realize`'s `emit(intent)` for native code — deliberately the same one line, so the two
/// authoring paths produce the same wiring:
///
/// ```ignore
/// // declarative (heca-view-realize):     native (here):
/// row.on_press(intent)                    row.on_activate(fires(mount, intent, emit))
/// row.on_hint(intent)                     row.on_hint(picks(mount, intent, emit))
/// ```
///
/// **Why a name and not a closure.** A closure is reachable from exactly one place: the gesture
/// that captured it. An [`Intent`] is an action id plus arguments, so the same declaration answers
/// a click, a menu entry, a keybinding and an RPC call, is routed by the action's own policy, and
/// passes the destructive-confirm gate — none of which a closure can be. It is also the only form
/// that crosses a plugin boundary, so a native row and a WASM row declare their gestures the same
/// way.
///
/// **`mount` is not ceremony — it is which element this is.** A widget is built inside one mounted
/// container and the gesture it declares carries that seating
/// ([`SEAT_ARG`](crate::providers::SEAT_ARG)), so the same container seated twice has two rows that
/// each answer for themselves. It is a parameter rather than something a caller may add, because a
/// rule a caller has to remember is a rule that gets forgotten: without it the host had to resolve
/// the call back to an instance by guessing (focused seat, else last focused, else the first one
/// that declares the name), and a right-sidebar row was performed by the left-sidebar copy. Every
/// call site already holds its mount — it is what `BuildCx::container_id` hands a component.
///
/// It replaced `named_press`, which handed back **one** intent for both the click and the
/// `prefix+/` pick. Those are different gestures — a click on a sidebar row means *go there and
/// leave*, a pick means *look at that one* — and serving both from one declaration is what made
/// `prefix+/` walk out of the sidebar (F004/P084/T399).
pub(crate) fn fires(
    mount: &str,
    intent: Intent,
    emit: &ChromeIntentEmitter,
) -> impl Fn() + 'static {
    let seated = seated(mount, intent);
    let emit = emit.clone();
    move || {
        emit.fire(crate::app::interaction::InteractionIntent::View(
            seated.clone(),
        ))
    }
}

/// **What a `prefix+/` pick does** — [`fires`], plus the declaration of *what it is*.
///
/// The same closure, carried in a [`Hint`](heca_grid_ui::Hint) that keeps the [`Intent`] beside it.
/// A closure alone is opaque, and a host cannot ask its policy about an opaque thing — so a pick
/// whose action the domain would refuse was still offered a letter, and pressing it did nothing
/// (F003/P082/T432). With the intent in hand, `chrome::active_hint_targets` drops the candidate
/// before the letter is spent.
///
/// Reach for it wherever a pick has a name. A pick that genuinely has none is still a plain closure
/// — it simply cannot be judged, and is offered as it always was.
pub(crate) fn picks(mount: &str, intent: Intent, emit: &ChromeIntentEmitter) -> heca_grid_ui::Hint {
    let seated = seated(mount, intent);
    let run = seated.clone();
    let emit = emit.clone();
    heca_grid_ui::Hint::of(seated, move || {
        emit.fire(crate::app::interaction::InteractionIntent::View(run.clone()))
    })
}

/// Stamp the seating onto an intent — **which element this is**, so the same container seated twice
/// has two rows that each answer for themselves. One definition, so a click and a pick of the same
/// row cannot be addressed differently.
fn seated(mount: &str, mut intent: Intent) -> Intent {
    intent.args.insert(
        crate::providers::SEAT_ARG.to_string(),
        heca_view::PropValue::Text(mount.to_string()),
    );
    intent
}



/// The pane a press at `pos` (logical window coords) would start dragging, found by
/// hit-testing the **retained** chrome tree's real laid-out bounds (F4.5) — replaces
/// the legacy fixed-row `sidebar_hit_test`. `None` off any pane card.
pub(crate) fn sidebar_drag_source(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<ChromeDragItem> {
    let tree = state.chrome_tree.as_ref()?;
    let id =
        heca_grid_ui::drag::source_at(&state.window_root, Point::new(pos.0 as f64, pos.1 as f64))?;
    tree.drag_items.get(id).cloned()
}

/// The deepest sidebar item (pane → column → workspace) under `pos`, regardless of
/// drag semantics — used to anchor the right-click context menu on whatever the
/// cursor is over. Unlike [`sidebar_drag_source`] this accepts every registered
/// item (workspaces are drop-only, so they never appear as a drag source but must
/// still be right-clickable). Resolves against the **expanded** grid sidebar's
/// retained tree; the hand-drawn collapsed rail is not covered (it moves onto grid
/// widgets in the collapsed-rail migration, `app-task-21`).
pub(crate) fn sidebar_item_at(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<ChromeDragItem> {
    let tree = state.chrome_tree.as_ref()?;
    let hit = heca_grid_ui::drag::resolve_at_filtered(
        &state.window_root,
        Point::new(pos.0 as f64, pos.1 as f64),
        &|_| true,
    )?;
    tree.drag_items.get(hit.id).cloned()
}

/// The kind of thing being dragged — passed **explicitly** by the caller so drop
/// resolution never depends on the live drag payload, which is already wiped to
/// `Idle` by the time the release handler runs (`mouse.rs` `mem::replace`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragSourceKind {
    /// A pane is being dragged.
    Pane,
    /// A column is being dragged.
    Column,
}

/// Which drop-target kinds a given drag source may land on (F4.5 scope C). A pane
/// drag targets panes **and workspaces** — a workspace is only the resolved target
/// when the cursor is over its header/empty area (a pane card under the cursor is the
/// deeper hit and wins), which is the one way to move a pane into an *empty* workspace
/// (empty columns can't exist, so columns need no pane-drop target). A column drag
/// targets columns + workspaces — **never** the nested pane cards, or the deepest hit
/// would always be a pane and a column could never be dropped on another column.
fn target_accepted_by(source: DragSourceKind, item: &ChromeDragItem) -> bool {
    match source {
        DragSourceKind::Pane => {
            matches!(
                item,
                ChromeDragItem::Pane(_) | ChromeDragItem::Workspace { .. }
            )
        }
        DragSourceKind::Column => {
            matches!(
                item,
                ChromeDragItem::Column { .. } | ChromeDragItem::Workspace { .. }
            )
        }
    }
}

/// Resolve the drop a drag of `source` kind would land on at `pos`, filtered to the
/// target kinds it accepts (see [`target_accepted_by`]). `None` when no acceptable
/// target is under the cursor.
fn resolve_sidebar_drop(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
    source: DragSourceKind,
) -> Option<(ChromeDragItem, heca_grid_ui::drag::DropHit)> {
    let tree = state.chrome_tree.as_ref()?;
    let accept = |id| {
        tree.drag_items
            .get(id)
            .is_some_and(|it| target_accepted_by(source, it))
    };
    let hit = heca_grid_ui::drag::resolve_at_filtered(
        &state.window_root,
        Point::new(pos.0 as f64, pos.1 as f64),
        &accept,
    )?;
    let item = tree.drag_items.get(hit.id).cloned()?;
    Some((item, hit))
}

/// The drop target + [`DropSide`](heca_grid_ui::drag::DropSide) a drag of `source`
/// kind at `pos` lands on, source-aware (see [`resolve_sidebar_drop`]). `None` off
/// any acceptable item.
pub(crate) fn sidebar_drop_target(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
    source: DragSourceKind,
) -> Option<(ChromeDragItem, heca_grid_ui::drag::DropSide)> {
    resolve_sidebar_drop(state, pos, source).map(|(item, hit)| (item, hit.side))
}

/// Hash of everything the chrome tree displays (window size, theme, status text,
/// sidebar content). When it changes, the retained tree is rebuilt; otherwise the
/// existing tree is reused (re-laid-out + painted only).
pub(crate) fn chrome_signature(state: &crate::app_state::AppState, chrome: ChromeConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hsh = std::collections::hash_map::DefaultHasher::new();
    let phys = state.window.inner_size();
    phys.width.hash(&mut hsh);
    phys.height.hash(&mut hsh);
    state.scale_factor.to_bits().hash(&mut hsh);
    // App-wide font zoom scales the chrome font, so a change must rebuild the tree.
    state.app_font_zoom.to_bits().hash(&mut hsh);
    chrome.left_sidebar_width.to_bits().hash(&mut hsh);
    chrome.right_sidebar_width.to_bits().hash(&mut hsh);
    chrome.sidebar_gap.to_bits().hash(&mut hsh);
    state.theme.name.hash(&mut hsh);
    for c in [
        state.theme.accent,
        state.theme.foreground,
        state.theme.border,
    ] {
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    state.theme.border_radius.to_bits().hash(&mut hsh);
    state.appearance.chrome_opacity().to_bits().hash(&mut hsh);
    state.appearance.opacity().to_bits().hash(&mut hsh);
    // The sidebar frame STYLE is a build-time structural choice (it picks the Pane
    // frame), so a config reload that changes it must rebuild the retained tree.
    // (The border WIDTH is read at paint via `chrome_gui_theme`, so it live-reloads
    // without a rebuild.)
    state.appearance.effective_sidebar_border_style().hash(&mut hsh);
    // The sidebar border WIDTH/RADIUS and background are baked into the retained
    // tree at build time (per-widget Pane overrides + the shell fill), so a config
    // reload that changes them must rebuild the tree.
    state
        .appearance
        .effective_sidebar_border_width(&state.theme)
        .to_bits()
        .hash(&mut hsh);
    state
        .appearance
        .effective_sidebar_border_radius(&state.theme)
        .to_bits()
        .hash(&mut hsh);
    {
        let c = left_sidebar_shell_background_color(state);
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
        let c = right_sidebar_shell_background_color(state);
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    // The active workspace (which gets the accent wash + count badge) is structural
    // enough to rebuild on a workspace SWITCH — but pane-to-pane focus *within* a
    // workspace must NOT rebuild: pane/column `active` + the status text are bound
    // signals (`sync_chrome_signals`), deliberately excluded from this signature.
    state.session.active_workspace_idx.hash(&mut hsh);
    for ws in &state.chrome_state.workspaces.tree().workspaces {
        ws.ws_idx.hash(&mut hsh);
        ws.name.hash(&mut hsh);
        ws.collapsed.hash(&mut hsh);
        for c in &ws.columns {
            c.col_idx.hash(&mut hsh);
            c.collapsed.hash(&mut hsh);
            for p in &c.panes {
                p.pane_id.0.hash(&mut hsh);
                p.name.hash(&mut hsh);
                state
                    .chrome_state
                    .workspaces
                    .with_pane_runtime(p.pane_id, |runtime| {
                        runtime.and_then(|rt| rt.git.get_untracked()).is_some()
                    })
                    .hash(&mut hsh);
            }
            u8::MAX.hash(&mut hsh); // column separator in the hash stream
        }
        for p in &ws.floating_panes {
            p.pane_id.0.hash(&mut hsh);
            p.name.hash(&mut hsh);
            state
                .chrome_state
                .workspaces
                .with_pane_runtime(p.pane_id, |runtime| {
                    runtime.and_then(|rt| rt.git.get_untracked()).is_some()
                })
                .hash(&mut hsh);
        }
        u64::MAX.hash(&mut hsh); // workspace separator
    }
    hsh.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The two sidebar toggles keep their letters when you toggle a sidebar** (F003/P082/T444).
    ///
    /// Both buttons flip their glyph with the sidebar's state — left shows `ArrowLineLeft` while
    /// expanded and `ArrowLineRight` while collapsed, right is the mirror. With no `key` their
    /// identity is the glyph's name, so collapsing the left sidebar made *both* buttons derive
    /// `arrow_line_right`: one wore the bare name and the other `arrow_line_right[1]`, decided by
    /// document order. The left button took the name the right one had, and their remembered
    /// `prefix+/` letters swapped with it.
    ///
    /// Antonio, driving, 2026-08-19: *"toggling the left sidebar the letter are a and s, toggling
    /// again they get inverted s and a"*.
    ///
    /// Keyed by the action they run, both identities are stable through every combination of
    /// states — which is the whole point of a key: it comes from the data, not from the picture.
    #[test]
    fn the_sidebar_toggles_keep_their_identity_when_a_sidebar_is_toggled() {
        use heca_grid_ui::nav::identity_of;

        let shortcuts = ActionShortcuts::default();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let emit: ChromeIntentEmitter = ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, |_, _| {});

        // The bar as it is built: the two toggles side by side, in document order.
        let bar = |left_open: bool, right_open: bool| {
            let left_glyph = if left_open { Glyph::ArrowLineLeft } else { Glyph::ArrowLineRight };
            let right_glyph = if right_open { Glyph::ArrowLineRight } else { Glyph::ArrowLineLeft };
            Flex::row()
                .child(sidebar_toggle_button(
                    left_glyph,
                    crate::input::WmAction::SidebarLeft,
                    "sidebar_left",
                    &shortcuts,
                    &catalog,
                    emit.clone(),
                    Color::rgb(255, 255, 255),
                ))
                .child(sidebar_toggle_button(
                    right_glyph,
                    crate::input::WmAction::SidebarRight,
                    "sidebar_right",
                    &shortcuts,
                    &catalog,
                    emit.clone(),
                    Color::rgb(255, 255, 255),
                ))
        };

        // **Ask the collector where the targets are** rather than assuming a depth: the picker
        // identifies the node that declared the hint, and that is the node whose key has to be
        // stable. Assuming the button's own path is what made an earlier version of this test pass
        // while the app still swapped the letters.
        let ids = |left_open: bool, right_open: bool| {
            let mut tree = bar(left_open, right_open);
            heca_grid_ui::LayoutEngine::new()
                .compute(&mut tree, heca_core::layout::Size::new(400.0, 40.0));
            let paths: Vec<Vec<usize>> = heca_grid_ui::collect_hints(&tree)
                .into_iter()
                .map(|(path, _bounds)| path)
                .collect();
            assert_eq!(paths.len(), 2, "one pick target per toggle");
            (
                identity_of(&tree, &paths[0]),
                identity_of(&tree, &paths[1]),
            )
        };

        let (l_open, r_open) = ids(true, true);
        assert_eq!(l_open.as_deref(), Some("sidebar_left"));
        assert_eq!(r_open.as_deref(), Some("sidebar_right"));

        // The state that used to collide: left collapsed shows the same arrow the right one does.
        assert_eq!(
            ids(false, true),
            (l_open.clone(), r_open.clone()),
            "collapsing the left sidebar must not rename either button",
        );
        assert_eq!(ids(true, false), (l_open.clone(), r_open.clone()));
        assert_eq!(ids(false, false), (l_open, r_open));
    }
    use heca_config::programs::ProgramsConfig;
    use heca_core::layout::{LayoutOptions, Session, SessionId};
    use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
    use std::path::PathBuf;

    /// A rule sits between containers, and takes no share of the height.
    ///
    /// It has to be a leaf with its natural height, or it would be handed a share of its own and the
    /// containers would each lose height to a 1px line.
    #[test]
    fn a_rule_separates_containers_without_taking_a_share() {
        use heca_grid_ui::LayoutEngine;
        use heca_core::layout::Size as CoreSize;

        let body = || -> WidgetModel { Box::new(Flex::column().height(Length::Px(40.0))) };
        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        // Two containers with a rule between them, as `build_region_content` assembles them.
        stack.base_mut().children.push(with_share(body(), 1.0));
        stack = stack.child(Separator::horizontal());
        stack.base_mut().children.push(with_share(body(), 1.0));

        let mut root = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut root, CoreSize::new(300.0, 600.0));

        let kids = &root.base().children[0].base().children;
        assert_eq!(kids.len(), 3, "container, rule, container");
        assert_eq!(kids[1].base().style.layout.flex_grow, 0.0, "the rule takes no share");
        assert!(
            kids[1].base().bounds.size.h < 10.0,
            "the rule keeps its own thin height: {:?}",
            kids[1].base().bounds.size.h,
        );
        // Within a pixel: an odd leftover after the rule and the gaps has to land somewhere, so
        // equal shares of an odd number of pixels differ by one. Measured 292 / 1 / 291.
        assert!(
            (kids[0].base().bounds.size.h - kids[2].base().bounds.size.h).abs() <= 1.0,
            "and the containers still share equally around it: {:?}",
            kids.iter().map(|k| k.base().bounds.size.h).collect::<Vec<_>>(),
        );
    }

    /// Shares divide the region even when the content is taller than it.
    ///
    /// This is the part that looked done and was not. `flex_grow` distributes only *positive* free
    /// space, and a container's content is routinely taller than a sidebar — so two containers
    /// measured 1214px each inside a 600px body, overflowed the frame, and divided nothing. The fix
    /// is a zero base size plus permission to shrink (CSS `flex: 1 1 0`; this vocabulary has no
    /// `flex_basis`), which makes the free space the whole region.
    ///
    /// The numbers are asserted rather than the flags, because the flags were "right" while the
    /// layout was wrong.
    #[test]
    fn shares_divide_the_region_even_with_content_taller_than_it() {
        use heca_grid_ui::LayoutEngine;
        use heca_core::layout::Size as CoreSize;

        // A body far shorter than the content it holds, as a sidebar is.
        let tall = || -> WidgetModel {
            let mut inner = Flex::column();
            for _ in 0..20 {
                inner = inner.child(Flex::column().height(Length::Px(60.0)));
            }
            Box::new(heca_grid_ui::ScrollRegion::new().child(inner))
        };

        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        for _ in 0..2 {
            stack.base_mut().children.push(with_share(tall(), 1.0));
        }
        let mut body = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

        let stack = &body.base().children[0];
        assert!(
            stack.base().bounds.size.h <= 600.0,
            "the stack fits the region instead of overflowing it: {:?}",
            stack.base().bounds.size.h,
        );
        let heights: Vec<f64> = stack
            .base()
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        assert_eq!(heights.len(), 2);
        assert!(
            (heights[0] - heights[1]).abs() < 1.0,
            "equal shares are equal: {heights:?}",
        );
        assert!(
            heights[0] > 250.0 && heights[0] < 300.0,
            "each takes about half the 600px region, less the gap: {heights:?}",
        );
    }

    /// A container's declared share reaches the widget, and saying nothing means an equal share.
    ///
    /// The share is a flex grow factor, so it is the region's **main axis** — height in a sidebar,
    /// width in a bar — and one number covers both. What this pins is the wiring: the number on the
    /// contribution has to land on the body that gets laid out, and it would be silently dropped if
    /// anything rebuilt or rewrapped the body afterwards (F003/P011/T021).
    ///
    /// Whether two containers then *look* right side by side is layout, and the user judges that in
    /// the app — a test asserting taffy divides 200px into 100 and 100 would be testing taffy.
    #[test]
    fn a_containers_declared_share_reaches_its_body() {
        let body = || -> WidgetModel { Box::new(Flex::column()) };

        // The trait default, which is what a provider that says nothing gets.
        assert_eq!(
            crate::providers::Provider::grow(&crate::providers::WorkspacesContainerProvider::new()),
            1.0,
            "saying nothing means one equal share",
        );

        assert_eq!(with_share(body(), 1.0).base().style.layout.flex_grow, 1.0);
        assert_eq!(
            with_share(body(), 2.0).base().style.layout.flex_grow,
            2.0,
            "twice the share of a 1.0 beside it",
        );
        assert_eq!(
            with_share(body(), 0.0).base().style.layout.flex_grow,
            0.0,
            "content-sized: no share of the leftover",
        );
    }

    /// Wrapping a container in the focus ring + pick keycap must not disturb the shares.
    ///
    /// The wrappers are transparent, so the share has to be applied to the **outermost** node: on the
    /// body it would leave the wrapper content-sized, and two containers would divide nothing —
    /// exactly the failure F003/P011/T021 measured (1214px each inside a 600px body). Pixels are
    /// asserted, not flags, because the flags were right while the layout was broken.
    #[test]
    fn the_focus_wrappers_do_not_disturb_the_shares() {
        use heca_core::layout::Size as CoreSize;
        use heca_grid_ui::LayoutEngine;
        use heca_grid_ui::widgets::{FocusScope, KeyHint};

        // A body far taller than the region it is given, as a real dock is.
        let tall = || -> WidgetModel {
            let mut inner = Flex::column();
            for _ in 0..20 {
                inner = inner.child(Flex::column().height(Length::Px(60.0)));
            }
            Box::new(heca_grid_ui::ScrollRegion::new().grow(1.0).child(inner))
        };
        // The wrapping `focus_and_pick` applies, without needing a render context for it.
        let wrapped = || -> WidgetModel {
            let mut body = tall();
            pass_box_down(body.as_mut());
            let mut picked = KeyHint::new_boxed(body);
            pass_box_down(&mut picked);
            Box::new(FocusScope::new(picked).focus(signal(true)))
        };

        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        for _ in 0..2 {
            stack.base_mut().children.push(with_share(wrapped(), 1.0));
        }
        let mut body = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

        let stack = &body.base().children[0];
        let heights: Vec<f64> = stack
            .base()
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        assert_eq!(heights.len(), 2);
        assert!(
            (heights[0] - heights[1]).abs() < 1.0,
            "equal shares are still equal through the wrappers: {heights:?}",
        );
        assert!(
            heights[0] > 250.0 && heights[0] < 300.0,
            "each still takes about half the 600px region, less the gap: {heights:?}",
        );
        // And the wrappers pass the height straight down — a ring around a box half the size of the
        // box would be worse than no ring.
        let ring = &stack.base().children[0];
        let hinted = &ring.base().children[0];
        let dock = &hinted.base().children[0];
        assert_eq!(
            (ring.base().bounds.size.h, dock.base().bounds.size.h),
            (heights[0], heights[0]),
            "ring, keycap wrapper and dock all measure the same box",
        );
    }

    /// A container that asked for **no** share keeps its content height through the wrappers.
    ///
    /// `grow = 0.0` means "size me to my content", and a zero base size inside a content-sized parent
    /// would collapse the whole thing to nothing — which is why the pass-down is conditional.
    #[test]
    fn a_content_sized_container_keeps_its_height_through_the_wrappers() {
        use heca_core::layout::Size as CoreSize;
        use heca_grid_ui::LayoutEngine;
        use heca_grid_ui::widgets::{FocusScope, KeyHint};

        let body: WidgetModel = Box::new(Flex::column().height(Length::Px(120.0)));
        // What `focus_and_pick` does with `share = 0.0`: wrap, and touch no layout.
        let wrapped: WidgetModel =
            Box::new(FocusScope::new(KeyHint::new_boxed(body)).focus(signal(false)));
        let mut region = Flex::column()
            .height(Length::Px(600.0))
            .child_boxed(with_share(wrapped, 0.0));
        LayoutEngine::new().compute(&mut region, CoreSize::new(300.0, 600.0));

        let ring = &region.base().children[0];
        assert_eq!(
            ring.base().bounds.size.h, 120.0,
            "content-sized means the content's height, not zero and not the region's",
        );
    }

    /// **A placement is lettered by its own mount id** — `workspaces` and `workspaces.2` are two
    /// targets, not one. Asserted through the door every target now uses rather than a host-side
    /// projection helper, which is what this replaced (F003/P082/T427).
    #[test]
    fn a_second_placement_of_a_container_is_its_own_letter_target() {
        let mode = crate::app_state::InputMode::DockPick {
            candidates: vec![
                ('a', "workspaces".to_string()),
                ('b', "workspaces.2".to_string()),
            ],
        };
        let wanted = crate::chrome::hint::wanted_for_tests(&mode);
        assert_eq!(
            wanted,
            vec![
                (
                    crate::chrome::hint::Offer::ByKey("workspaces".to_string()),
                    'a',
                ),
                (
                    crate::chrome::hint::Offer::ByKey("workspaces.2".to_string()),
                    'b',
                ),
            ],
        );
    }

    #[test]
    fn test_content_rect_full() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.loc.x, 200.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 880.0);
        assert_eq!(r.size.h, 744.0);
    }

    #[test]
    fn test_content_rect_clamping_when_sidebars_exceed_window() {
        // Both sidebars together exceed the window width.
        // Left sidebar is NOT clamped for x-position, but IS clamped for width calculation.
        // Right sidebar is clamped to remaining space after left sidebar.
        // Width must never go negative.
        let c = ChromeConfig {
            tab_bar_height: 20.0,
            status_bar_height: 10.0,
            left_sidebar_width: 300.0,
            right_sidebar_width: 300.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(500.0, 600.0);
        // x = 300, y = 20
        // left clamped: min(300, 500) = 300
        // right clamped: min(300, 500-300) = min(300, 200) = 200
        // w = 500 - 300 - 200 = 0  (not negative)
        // h = 600 - 20 - 10 = 570
        assert_eq!(r.loc.x, 300.0);
        assert_eq!(r.loc.y, 20.0);
        assert_eq!(r.size.w, 0.0);
        assert_eq!(r.size.h, 570.0);
    }

    #[test]
    fn test_content_rect_no_sidebars() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 0.0,
            right_sidebar_width: 0.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(1024.0, 768.0);
        assert_eq!(r.loc.x, 0.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 1024.0);
        assert_eq!(r.size.h, 712.0);
    }

    #[test]
    fn test_content_rect_reserves_sidebar_gap_between_sidebars_and_content() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
            sidebar_gap: 12.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.loc.x, 212.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 856.0);
        assert_eq!(r.size.h, 744.0);
    }

    #[test]
    fn app_theme_to_gui_theme_preserves_loaded_palette_tokens() {
        let theme = heca_config::theme::load("mocha");
        let font_config = heca_config::font::FontConfig::default();
        let gui = super::theme::app_theme_to_gui_theme(&theme, &font_config);

        assert_eq!(gui.colors.name, theme.name);
        assert_eq!(gui.colors.background, theme.background);
        assert_eq!(gui.colors.surface, theme.surface);
        assert_eq!(gui.colors.muted, theme.muted);
        assert_eq!(gui.colors.border, theme.border);
        assert_eq!(gui.colors.accent, theme.accent);
        assert_eq!(gui.colors.glow, theme.glow);
        assert_eq!(gui.colors.danger, theme.danger);
        assert_eq!(gui.colors.success, theme.success);
        assert_eq!(gui.colors.warning, theme.warning);
        assert_eq!(gui.font_family, font_config.family.ui_normal());
        assert_eq!(gui.font_size, font_config.size.ui);
        assert_eq!(gui.colors.border_radius, theme.border_radius);
        assert_eq!(gui.colors.border_width, theme.border_width);
        assert_eq!(gui.colors.glow_size, heca_grid_ui::theme::GlowLevel::None);
        assert_eq!(gui.colors.intensity, heca_grid_ui::theme::Intensity::Off);
        assert!(gui.colors.show_focus_border);
    }

    #[test]
    fn chrome_background_and_surface_colors_come_from_theme_tokens() {
        let tron = heca_config::theme::load("grid_tron");
        let latte = heca_config::theme::load("latte");

        assert_eq!(
            super::theme::top_bottom_pane_background_color(&tron),
            tron.effective_top_bottom_pane_background()
        );
        assert_eq!(super::theme::chrome_surface_color(&tron), tron.surface);
        assert_eq!(
            super::theme::chrome_surface_color(&latte),
            latte.surface
        );
        assert_ne!(
            latte.effective_left_sidebar_background(),
            super::theme::chrome_surface_color(&latte)
        );
    }

    #[test]
    fn sidebar_shell_background_stays_distinct_from_bar_tint_when_transparent() {
        let theme = heca_config::theme::load("latte");
        let appearance = heca_config::appearance::AppearanceConfig {
            transparency: 10,
            ..Default::default()
        };

        let left_bg = theme.effective_left_sidebar_background();
        assert_ne!(
            super::theme::chrome_bar_color_for(&theme, &appearance),
            super::theme::sidebar_shell_background_color_for(left_bg, &appearance)
        );
        assert_eq!(
            super::theme::sidebar_shell_background_color_for(left_bg, &appearance).r,
            theme.effective_left_sidebar_background().r
        );
    }

    #[test]
    fn chrome_scene_emits_status_text() {
        use heca_grid_ui::{Color, DrawCommand};
        let theme = GuiTheme::default();
        // No sidebar (collapsed) — just the status bar should produce text.
        let scene = super::chrome_scene(
            &super::ChromeFrame {
                w: 800.0,
                h: 600.0,
                tab_bar_height: 32.0,
                status_bar_height: 24.0,
                status: "2 panes | foo | NORMAL",
                side_bg: theme.colors.background,
                fg: Color::new(200, 200, 200, 255),
            },
            &theme,
            None,
            None,
        );
        assert!(!scene.is_empty(), "scene should not be empty");
        assert!(
            scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(..))),
            "scene should contain at least one Text draw command",
        );
    }

    /// A one-workspace / one-column / one-pane tree — the smallest projection that still
    /// has every level.
    fn one_pane_tree(pane_id: u64) -> crate::providers::workspaces::WorkspaceTree {
        use crate::app_state::SidebarItemState;
        use crate::providers::workspaces::{ColumnEntry, PaneEntry, WorkspaceTree, WorkspaceEntry};

        let mut tree = WorkspaceTree::new();
        tree.workspaces.push(WorkspaceEntry {
            ws_idx: 0,
            ws_id: heca_core::layout::WorkspaceId(0),
            name: "ws1".into(),
            custom_name: None,
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![ColumnEntry {
                col_idx: 0,
                col_id: heca_core::layout::ColumnId(0),
                collapsed: false,
                panes: vec![PaneEntry {
                    pane_id: heca_core::layout::PaneId(pane_id),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });
        tree
    }

    /// Build a region's body the way the app does since the provider cutover: through the
    /// `ChromeHost`, by asking the mounted provider for its contribution and calling its
    /// `build` seam. These tests deliberately do **not** reach into the container's own
    /// builder — that would test a path the app no longer takes.
    fn region_body(
        tree: &crate::providers::workspaces::WorkspaceTree,
        theme: &GuiTheme,
        chrome: &SharedChromeState,
        signals: &mut super::ChromeSignals,
        drag: &mut super::DragItemRegistry,
    ) -> Option<super::WidgetModel> {
        let mut host = super::ChromeHost::new(chrome.events());
        host.register(Box::new(crate::providers::WorkspacesContainerProvider::new()));
        let emit: super::ChromeIntentEmitter = ChromeIntentEmitter::of(crate::app::interaction::InteractionSource::Keyboard, |_, _| {});
        // The component reads its model from its own state, so the fixture puts it there rather
        // than handing it to the context (F003/P086/T367).
        *chrome.workspaces.tree_mut() = tree.clone();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = crate::providers::ChromeCtx::for_build(
            crate::host::App::new(chrome),
            theme,
            &emit,
            &catalog,
        );
        super::build_region_content(&host, super::RegionId::LeftSidebar, &ctx, signals, drag)
    }

    #[test]
    fn sidebar_shell_hosts_the_provider_mounted_container() {
        use heca_grid_ui::Component;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        chrome
            .workspaces
            .set_active_pane(Some(heca_core::layout::PaneId(1)));
        // The shell wraps a bracketed Pane that holds just the container the region's
        // provider built (the collapse toggle moved to the top bar, sidebar-fu-14); the
        // container hosts a dock per workspace (so the tree's text is visible).
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        assert!(
            content.is_some(),
            "the workspaces provider is mounted in the left region, so it must contribute a body",
        );
        let shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
        );
        assert_eq!(
            shell.base().children.len(),
            1,
            "sidebar shell wraps a single full-height background surface",
        );
        let surface = &shell.base().children[0];
        assert_eq!(
            surface.base().children.len(),
            1,
            "the background surface wraps a single bracketed Pane",
        );
        let pane = &surface.base().children[0];
        assert_eq!(
            pane.base().children.len(),
            1,
            "the shell Pane holds just the mounted container's body (the collapse toggle \
             moved to the top bar, sidebar-fu-14)",
        );
        let container = &pane.base().children[0];
        assert!(
            !container.base().children.is_empty(),
            "the WorkspacesContainer must host a dock per workspace",
        );
    }

    #[test]
    fn bordered_sidebar_paints_border_color_at_configured_width() {
        // Regression: with `sidebar_border_style = "bordered"`, the shell must paint
        // a visible frame in the (global) border color at the configured
        // `sidebar_border_width`. Previously the bordered sidebar drew nothing /
        // ignored the color because its width was theme-locked.
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);

        // A distinct border color so we can prove it reached the painted frame.
        let mut theme = GuiTheme::default();
        theme.colors.border = Color::new(0x40, 0xe0, 0xff, 0xff);
        let border_w = 4.0_f32;

        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bordered,
            border_w,
            12.0,
            content,
        );

        let scene = super::paint_chrome_root(&mut shell, 280.0, 600.0, &theme);
        let found = scene.iter().any(|c| match c {
            DrawCommand::Rect(r) => r
                .border
                .is_some_and(|b| b.color == theme.colors.border && (b.width - border_w).abs() < 0.01),
            _ => false,
        });
        assert!(
            found,
            "bordered sidebar must paint a {border_w}px frame in the configured border color",
        );
    }

    /// The focused dock is **outlined**, and an unfocused one is not (F003/P011/T020).
    ///
    /// Focus with nothing to show for it tells the user nothing, so this asserts the drawn ring
    /// rather than a flag — and asserts that it follows the store *without a rebuild*, because the
    /// host flips a signal rather than rebuilding the chrome tree.
    ///
    /// Whether the ring reads well at that width and colour is a visual judgement, and the user makes
    /// it in the app. What is pinned here is that it is drawn at all, on the right dock.
    #[test]
    fn the_focused_dock_is_outlined_and_follows_the_store() {
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
        );

        let ring = theme.colors.effective_focus_ring();
        let outlined = |shell: &mut Flex| {
            super::paint_chrome_root(shell, 280.0, 600.0, &theme)
                .iter()
                .any(|c| match c {
                    DrawCommand::Rect(r) => r.border.is_some_and(|b| b.color == ring),
                    _ => false,
                })
        };

        assert!(
            !outlined(&mut shell),
            "no dock has focus yet, so nothing is outlined",
        );
        chrome.set_focused_container(Some("workspaces".into()));
        assert!(
            outlined(&mut shell),
            "the focused dock is outlined in the theme's focus colour — same tree, one signal",
        );
        chrome.set_focused_container(Some("something-else".into()));
        assert!(
            !outlined(&mut shell),
            "and focus elsewhere takes the outline away",
        );
    }

    /// A dock's pick letter reaches the tree: the host registers the signal, and setting it stamps a
    /// keycap over that dock. The letters themselves come from the pick candidates
    /// (`dock_keycap`), projected each frame by `sync_chrome_signals`.
    #[test]
    fn a_docks_pick_letter_is_stamped_over_it() {
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut signals = super::ChromeSignals::default();
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut signals,
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
        );

        // **The dock is lettered by the identity it declares**, through the one door every other
        // target uses — `offer_hint_by_key`, matching its `scope_key`. No per-widget signal list,
        // and nothing projected onto it every frame (F003/P082/T427).
        let letter_drawn = |shell: &mut Flex| {
            super::paint_chrome_root(shell, 280.0, 600.0, &theme)
                .iter()
                .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "a"))
        };
        assert!(!letter_drawn(&mut shell), "no pick open, no keycap");
        assert!(
            heca_grid_ui::offer_hint_by_key(&shell, "workspaces", Some("a".to_string())),
            "the mounted container answers to its own mount id",
        );
        assert!(
            letter_drawn(&mut shell),
            "the dock's letter is stamped over it while the pick is open",
        );
        heca_grid_ui::offer_hint_by_key(&shell, "workspaces", None);
        assert!(!letter_drawn(&mut shell), "and withdrawn when the pick ends");
    }

    #[test]
    fn drag_registry_captures_pane_column_and_workspace() {
        let tree = one_pane_tree(7);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut drag = super::DragItemRegistry::default();
        // Drag items are registered while the region's mounted container builds its body,
        // so go through the host path and inspect what landed in the registry.
        let _ = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut drag,
        );

        let items = drag.items();
        // The drag framework decides kind by the side-map, not by raw ids — so a pane,
        // its column, and its workspace must each be registered (F4.5 step 2 scope C).
        assert!(items.contains(&super::ChromeDragItem::Pane(heca_core::layout::PaneId(7))));
        assert!(items.contains(&super::ChromeDragItem::Column { ws: 0, col: 0 }));
        assert!(items.contains(&super::ChromeDragItem::Workspace { ws: 0 }));
    }

    #[test]
    fn sync_pane_runtime_state_projects_session_runtime_into_store() {
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        {
            let ws = session
                .active_workspace_mut()
                .expect("session should create an initial workspace");
            ws.add_pane(
                heca_core::layout::Pane::new(PaneId(10), "editor"),
                None,
                true,
                ColumnWidth::Proportion(0.5),
                heca_core::layout::ColumnId(10),
            );
            ws.floating_panes
                .push(heca_core::layout::workspace::FloatingPane {
                    pane: heca_core::layout::Pane::new(PaneId(20), "git"),
                    position: Point::new(50.0, 50.0),
                    size: Size::new(400.0, 300.0),
                    is_active: true,
                    original_column_idx: None,
                    original_pane_idx: None,
                });
            ws.find_pane_mut(PaneId(10)).expect("tiled pane").runtime = PaneRuntime {
                program: Some("nvim".into()),
                status: ProcessStatus::Running,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: Some(0),
                git: Some(GitInfo {
                    branch: Some("main".into()),
                    ahead: 1,
                    behind: 0,
                    added: 2,
                    modified: 3,
                    deleted: 4,
                    dirty: true,
                }),
                kind: ContentKind::Terminal,
            };
            ws.find_pane_mut(PaneId(20)).expect("floating pane").runtime = PaneRuntime {
                program: Some("lazygit".into()),
                status: ProcessStatus::Idle,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: None,
                git: None,
                kind: ContentKind::Terminal,
            };
        }

        super::signals::sync_pane_runtime_state(&session, &chrome.workspaces);

        let tiled = chrome.workspaces.with_pane_runtime(PaneId(10), |runtime| {
            runtime.expect("tiled runtime").snapshot()
        });
        let floating = chrome.workspaces.with_pane_runtime(PaneId(20), |runtime| {
            runtime.expect("floating runtime").snapshot()
        });
        assert_eq!(tiled.program.as_deref(), Some("nvim"));
        assert_eq!(tiled.status, ProcessStatus::Running);
        assert_eq!(tiled.cwd, Some(PathBuf::from("/tmp/project")));
        assert_eq!(
            tiled.git,
            Some(GitInfo {
                branch: Some("main".into()),
                ahead: 1,
                behind: 0,
                added: 2,
                modified: 3,
                deleted: 4,
                dirty: true,
            })
        );
        assert_eq!(floating.program.as_deref(), Some("lazygit"));
        assert_eq!(floating.status, ProcessStatus::Idle);
    }

    #[test]
    fn sync_pane_runtime_state_prunes_removed_panes() {
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        {
            let ws = session
                .active_workspace_mut()
                .expect("session should create an initial workspace");
            ws.add_pane(
                heca_core::layout::Pane::new(PaneId(10), "editor"),
                None,
                true,
                ColumnWidth::Proportion(0.5),
                heca_core::layout::ColumnId(10),
            );
        }

        super::signals::sync_pane_runtime_state(&session, &chrome.workspaces);
        assert!(
            chrome
                .workspaces
                .with_pane_runtime(PaneId(10), |runtime| runtime.is_some())
        );

        session
            .active_workspace_mut()
            .expect("session should keep its workspace")
            .scrolling
            .columns
            .clear();
        super::signals::sync_pane_runtime_state(&session, &chrome.workspaces);

        assert!(
            !chrome
                .workspaces
                .with_pane_runtime(PaneId(10), |runtime| runtime.is_some())
        );
    }

    #[test]
    fn pane_info_view_resolves_program_status_and_git_segments() {
        let programs = ProgramsConfig::default();
        let runtime = PaneRuntime {
            program: Some("v".into()),
            status: ProcessStatus::Running,
            cwd: None,
            exit_code: None,
            git: Some(GitInfo {
                branch: Some("main".into()),
                ahead: 0,
                behind: 0,
                added: 2,
                modified: 3,
                deleted: 1,
                dirty: true,
            }),
            kind: ContentKind::Terminal,
        };

        // No custom name → title is the program name, no process hint (the title IS the process).
        let view = pane_info_view(&programs, "shell", None, Some(&runtime), true);
        assert_eq!(view.title, "Neovim");
        assert_eq!(view.process_hint, None);

        // Custom name + setting on → title is the custom name, hint carries the program name.
        let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), true);
        assert_eq!(view.title, "Editor");
        assert_eq!(view.process_hint.as_deref(), Some("Neovim"));
        // Custom name + setting off → no hint.
        let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), false);
        assert_eq!(view.title, "Editor");
        assert_eq!(view.process_hint, None);

        let view = pane_info_view(&programs, "shell", None, Some(&runtime), false);

        assert_eq!(view.icon, Glyph::FileCode);
        assert_eq!(view.title, "Neovim");
        assert_eq!(view.status, ProcessStatus::Running);
        assert_eq!(view.git_branch.as_deref(), Some("main"));
        assert_eq!(view.git_added.as_deref(), Some("+2"));
        assert_eq!(view.git_modified.as_deref(), Some("~3"));
        assert_eq!(view.git_deleted.as_deref(), Some("-1"));
    }

    #[test]
    fn pane_info_view_uses_shell_fallbacks_without_git() {
        let programs = ProgramsConfig::default();
        let runtime = PaneRuntime {
            program: Some("zsh".into()),
            status: ProcessStatus::Idle,
            ..PaneRuntime::default()
        };

        let view = pane_info_view(&programs, "pane", None, Some(&runtime), false);

        assert_eq!(view.icon, Glyph::Terminal);
        assert_eq!(view.title, "zsh");
        assert_eq!(view.status, ProcessStatus::Idle);
        assert_eq!(view.git_branch, None);
        assert_eq!(view.git_added, None);
        assert_eq!(view.git_modified, None);
        assert_eq!(view.git_deleted, None);
    }

    #[test]
    fn pane_name_segment_renders_the_panes_own_name() {
        use heca_config::appearance::PaneSegment;
        let programs = ProgramsConfig::default();
        let theme = GuiTheme::default();
        let runtime = PaneRuntime {
            program: Some("v".into()),
            status: ProcessStatus::Running,
            ..PaneRuntime::default()
        };

        // Renamed pane → the `pane_name` segment produces a bar (the custom name wins over
        // the program name, unlike `app_name` which always tracks the process).
        let bar = pane_header::build_pane_info_bar(
            &programs,
            "shell",
            Some("Editor"),
            Some(&runtime),
            &[PaneSegment::PaneName],
            &theme,
            400.0,
            13.0,
        );
        assert!(bar.is_some());

        // Un-renamed pane → still produces a bar (falls back to the program name, never empty).
        let bar = pane_header::build_pane_info_bar(
            &programs,
            "shell",
            None,
            Some(&runtime),
            &[PaneSegment::PaneName],
            &theme,
            400.0,
            13.0,
        );
        assert!(bar.is_some());
    }

    #[test]
    fn pane_action_spec_maps_kinds_to_actions() {
        use crate::input::WmAction;
        use heca_config::appearance::PaneAction;
        let pid = PaneId(7);
        let catalog = crate::actions::ActionCatalog::with_builtins();

        // Pane-parameterized actions carry the pane/column and don't need focus. Icons
        // resolve from the action catalog (close = FolderSimpleMinus; add-pane = the
        // add_pane_to_column identity → FolderSimplePlus).
        let (g, a, _, focus) = super::pane_header::pane_action_spec(&catalog, PaneAction::Close, pid, 2, 3);
        assert_eq!(g, Glyph::FolderSimpleMinus);
        assert_eq!(a, WmAction::ClosePaneById { pane_id: pid });
        assert!(!focus);

        let (g, a, _, focus) = super::pane_header::pane_action_spec(&catalog, PaneAction::Split, pid, 2, 3);
        assert_eq!(g, Glyph::FolderSimplePlus);
        assert_eq!(
            a,
            WmAction::AddPaneToColumn {
                ws_idx: 2,
                col_idx: 3
            }
        );
        assert!(!focus);

        // Active-targeted actions use the requested icons + need focus-first.
        let (g, a, _, focus) = super::pane_header::pane_action_spec(&catalog, PaneAction::Zoom, pid, 0, 0);
        assert_eq!(g, Glyph::FrameCorners);
        assert_eq!(a, WmAction::ZoomColumn);
        assert!(focus);

        let (g, a, _, focus) = super::pane_header::pane_action_spec(&catalog, PaneAction::Float, pid, 0, 0);
        assert_eq!(g, Glyph::Cards);
        assert_eq!(a, WmAction::Float);
        assert!(focus);
    }

    #[test]
    fn floating_pane_keeps_only_float_and_close() {
        use heca_config::appearance::PaneAction;
        let catalog = crate::actions::ActionCatalog::with_builtins();
        // Driven by the action policy: float/close are focused-pane-local (kept),
        // split/zoom/move are tiled-only (hidden when floating).
        assert!(super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::Float));
        assert!(super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::Close));
        assert!(!super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::Split));
        assert!(!super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::Zoom));
        assert!(!super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::MoveLeft));
        assert!(!super::pane_header::pane_action_visible_when_floating(&catalog, PaneAction::MoveRight));
    }

    #[test]
    fn pane_header_key_changes_on_content_and_width() {
        use heca_config::appearance::{PaneAction, PaneSegment};
        let programs = ProgramsConfig::default();
        let segments = [PaneSegment::AppName, PaneSegment::GitBranch];
        let actions = [PaneAction::Split, PaneAction::Close];
        let runtime = PaneRuntime {
            program: Some("zsh".into()),
            status: ProcessStatus::Idle,
            git: Some(GitInfo {
                branch: Some("main".into()),
                ..GitInfo::default()
            }),
            ..PaneRuntime::default()
        };
        let hints = ActionShortcuts::default();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        #[allow(clippy::too_many_arguments)]
        fn content<'a>(
            programs: &'a ProgramsConfig,
            segments: &'a [heca_config::appearance::PaneSegment],
            actions: &'a [heca_config::appearance::PaneAction],
            rt: &'a PaneRuntime,
            hints: &'a ActionShortcuts,
            catalog: &'a crate::actions::ActionCatalog,
            col_idx: usize,
        ) -> super::pane_header::PaneHeaderContent<'a> {
            super::pane_header::PaneHeaderContent {
                programs,
                fallback_name: "shell",
                custom_name: None,
                runtime: Some(rt),
                segments,
                actions,
                ws_idx: 0,
                col_idx,
                zoomed: false,
                floating: false,
                shortcuts: hints,
                catalog,
            }
        }
        let base = super::pane_header::pane_header_key(
            &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
            15.0,
            300.0,
        );
        // Same inputs ⇒ same key (no needless rebuild).
        assert_eq!(
            base,
            super::pane_header::pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                300.0
            )
        );
        // A different column ⇒ different key (re-bakes the split action's col_idx).
        assert_ne!(
            base,
            super::pane_header::pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 1),
                15.0,
                300.0
            )
        );
        // A different branch ⇒ different key (rebuild).
        let mut other = runtime.clone();
        other.git = Some(GitInfo {
            branch: Some("dev".into()),
            ..GitInfo::default()
        });
        assert_ne!(
            base,
            super::pane_header::pane_header_key(
                &content(&programs, &segments, &actions, &other, &hints, &catalog, 0),
                15.0,
                300.0
            )
        );
        // A large width change ⇒ different key (re-truncate); tiny jitter ⇒ same bucket.
        assert_ne!(
            base,
            super::pane_header::pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                120.0
            )
        );
        assert_eq!(
            base,
            super::pane_header::pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                295.0
            )
        );
    }

}

#[cfg(test)]
mod search_bar_tests {
    use super::*;

    /// A pane deliberately far from the window origin: a bar placed relative to the
    /// window instead of the pane lands nowhere near this rect.
    fn pane() -> Rectangle {
        Rectangle::new(Point::new(600.0, 100.0), Size::new(640.0, 1400.0))
    }

    fn viewport() -> Size {
        Size::new(1900.0, 1600.0)
    }

    /// Lay the bar out for a query field of `field` size and return the bar's own
    /// bounds — the row holding the field slot and the counter.
    fn laid_out_bar_bounds(field: Size, count: Option<String>) -> Rectangle {
        let theme = GuiTheme::default();
        let mut root = search_bar_tree(field, count, pane(), &theme);
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut root, viewport());
        // root (pane box) -> row
        root.base().children[0].base().bounds
    }

    /// A representative measured field size.
    fn field(w: f64) -> Size {
        Size::new(w, 22.0)
    }

    /// **Regression guard.** The bar is positioned purely by margins on its box, so
    /// it must land inside the pane it belongs to. This caught the engine silently
    /// dropping a root's margin, which drew the bar over the *sidebar* — a whole pane
    /// away from the terminal it described.
    #[test]
    fn the_search_bar_lands_inside_its_pane() {
        let b = laid_out_bar_bounds(field(120.0), Some("29/36".into()));
        let p = pane();
        assert!(
            b.loc.x >= p.loc.x
                && b.loc.y >= p.loc.y
                && b.loc.x + b.size.w <= p.loc.x + p.size.w
                && b.loc.y + b.size.h <= p.loc.y + p.size.h,
            "bar at {:?} escaped its pane {p:?}",
            b
        );
    }

    /// It is anchored to the bottom-right specifically, not merely somewhere inside.
    #[test]
    fn the_search_bar_hugs_the_bottom_right_corner() {
        let b = laid_out_bar_bounds(field(120.0), Some("29/36".into()));
        let p = pane();
        let right_gap = (p.loc.x + p.size.w) - (b.loc.x + b.size.w);
        let bottom_gap = (p.loc.y + p.size.h) - (b.loc.y + b.size.h);
        assert!(
            right_gap < p.size.w / 2.0 && bottom_gap < p.size.h / 2.0,
            "expected bottom-right; gaps were right={right_gap} bottom={bottom_gap}"
        );
    }

    /// The bar is sized by the field the engine measured, not by arithmetic — the old
    /// version clamped width to a hand-picked 120..480 range computed from a hardcoded
    /// glyph advance ratio.
    #[test]
    fn a_wider_field_makes_a_wider_bar() {
        let narrow = laid_out_bar_bounds(field(80.0), None).size.w;
        let wide = laid_out_bar_bounds(field(300.0), None).size.w;
        assert!(wide > narrow, "wide bar {wide} should exceed narrow {narrow}");
    }

    /// **The field must land in the slot the tree reserved**, or the caret and the
    /// text would draw somewhere other than the bar the user sees.
    #[test]
    fn the_reserved_slot_matches_the_field_size() {
        let theme = GuiTheme::default();
        let f = field(140.0);
        let mut root = search_bar_tree(f, Some("1/3".into()), pane(), &theme);
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut root, viewport());
        let slot = search_field_slot(&root).expect("the tree always reserves a slot");
        assert_eq!((slot.size.w, slot.size.h), (f.w, f.h));
    }
}
