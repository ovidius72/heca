//! Putting a menu on screen as a layer: one a widget declared, or one the host builds from
//! entries, with the rows other components contribute merged in.

use heca_grid_ui::widgets::{ContextMenu, Menu, MenuAnchor};
use heca_grid_ui::Point;

use super::{menu_from_items, plugin_providers_for, target_from_subject};
use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
use crate::chrome::{ChromeIntentEmitter, LayerKind, OverlayId};
use crate::providers::ChromeCtx;

// `DropdownItem` lives in `heca-view` now: a menu entry is pure data carrying an
// `Intent`, and a **described** tree must be able to declare one. Re-exported here so the app-side
// name a hundred call sites already use keeps working — one type, not a second one beside it.
pub use heca_view::DropdownItem;

/// A cursor-anchored dropdown / context menu spec — the pointer / `OpenContextMenu` counterpart to
/// [`ModalSpec`](crate::chrome::ModalSpec). A data description a native handler **or** a plugin submits to [`open_dropdown`].
pub struct DropdownSpec {
    pub anchor: Point,
    pub items: Vec<DropdownItem>,
    /// Attribution for the dispatched action (which surface opened the menu).
    pub source: InteractionSource,
    /// When true the menu is **centered on `anchor`** (anchor = desired center) instead of
    /// placed down-right of it. Set by the keyboard/RPC-open path; the mouse path leaves it
    /// `false` (anchor = click point).
    pub centered: bool,
}

impl DropdownSpec {
    /// Mark the anchor as the desired **panel center** (keyboard/RPC-opened menus) rather than
    /// the top-left. The mouse-open path keeps the default `false`.
    // A keyboard- or RPC-opened menu will set it; neither opens a menu through this path yet.
    #[allow(dead_code)]
    pub fn centered(mut self, on: bool) -> Self {
        self.centered = on;
        self
    }
}

/// Put a built menu on screen as an Overlay-band **modal** layer.
///
/// The one place a menu becomes a layer, shared by both ways one is opened: the host building it
/// from a registry ([`open_dropdown`]) and a **widget declaring its own** ([`present_menu`]). A
/// second insert site is how two menus end up covering differently.
///
/// A menu **captures input and demands a choice**, so it covers for policy purposes even though its
/// panel is small: *a modal is an overlay with coverage*. Without that, the prefix
/// sequence that deliberately falls through the overlay key path (so `prefix+/` can still pick an
/// entry) reaches the router and runs — `prefix+x` with a menu open raised the close-pane confirm.
fn insert_menu_layer(state: &mut AppState, id: OverlayId, panel: ContextMenu) {
    let parent = state.layers.current();
    state.layers.insert(
        id.0,
        parent,
        LayerKind::OnDemand,
        heca_grid_ui::WidgetModel::new(panel),
        &mut state.window_root,
    );
}

/// **Present a menu a widget declared**.
///
/// The whole of the host's job in the declared path: the widget built the menu and the framework
/// chose the anchor, so this only mounts it and wires the two things a widget cannot reach — the
/// layer it lives in, and the close that follows a choice.
///
/// Plugin entries still merge: a menu that gave itself a [`name`](Menu::name) is offered to the
/// mounted providers for that name, so "Open in Docker" can still appear on a row a different
/// component declared. A menu that named itself nothing is simply itself.
pub(crate) fn present_menu(
    state: &mut AppState,
    ctx: ContextMenu,
    anchor: MenuAnchor,
    subject: Option<String>,
) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = InteractionSource::MouseContent;
    let emit = ChromeIntentEmitter::new(&state.event_proxy, source);

    // Rows other components added to this menu — only if it named itself.
    let mut ctx = ctx;
    ctx.set_menu(merge_contributions(
        state,
        ctx.menu().clone(),
        &emit,
        subject.as_deref(),
        anchor_point(&anchor),
    ));

    // Choosing an entry runs its own closure; taking the layer down afterwards is the host's, so an
    // item stays a plain closure that knows nothing about overlays. The anchor was chosen by
    // whatever triggered the menu — the cursor for a right-click, the widget for the keyboard.
    let panel = anchor
        .open(ctx)
        .after_select(id.closer(&emit))
        .on_dismiss(id.closer(&emit));

    insert_menu_layer(state, id, panel);
    state.needs_redraw = true;
    id
}

/// Where the menu was opened, when a pointer opened it. `None` for a keyboard-opened menu, which
/// has no cell under it — which is exactly why "Open link" never appears on one.
fn anchor_point(anchor: &MenuAnchor) -> Option<Point> {
    match anchor {
        MenuAnchor::At(p) => Some(*p),
        _ => None,
    }
}

/// Append every mounted provider's rows for the menu's [`name`](Menu::name).
///
/// A menu without a name is closed: it built its own rows and nothing else may add to it.
fn merge_contributions(
    state: &mut AppState,
    menu: Menu,
    emit: &ChromeIntentEmitter,
    subject: Option<&str>,
    at: Option<Point>,
) -> Menu {
    let Some(path) = menu.declared_name().map(str::to_string) else {
        return menu;
    };
    // **A menu is opened about something, and the thing says what it is.** The declaring widget's
    // own identity travels with the menu, so a provider building entries for this menu is told what
    // it is building them for — rather than the host hit-testing to work it out, which is how a
    // pane's whole menu ended up hand-written in the mouse handler.
    //
    // A menu whose declarer publishes no identity is a contribution like any other: entries that
    // act on app state rather than on a particular thing.
    let target = target_from_subject(state, subject, at);
    let ctx = ChromeCtx::new(&state.chrome_state);
    let plugin = plugin_providers_for(&state.chrome_host, &ctx, &path);
    let items = state
        .context_menu_registry
        .items_for(&ctx, &path, &target, plugin);
    // **The same conversion the declarer's own entries went through** — one door, not a second
    // one for contributed rows — one conversion, every authoring path.
    //
    // What stood here built its rows by hand and wired each to `SubmitOverlay`, which resolves the
    // overlay and hands the chosen id to a *completion* — and a menu presented from a widget's own
    // declaration has no completion, because nobody registers one. So contributed rows opened
    // fine and then did **nothing at all** when chosen, with nothing failing anywhere. It went
    // unnoticed while contributions were the rare case; the moment a pane's whole menu arrived
    // this way, every entry in it was dead.
    //
    // Going through the one conversion means a contributed row runs **its own intent**, dispatched
    // by name through the central gate — the same policy and destructive-confirm a keypress gets —
    // exactly as a row the declarer wrote does.
    let contributed =
        menu_from_items("", "", "", items, &state.action_catalog, emit);
    let mut menu = menu;
    for row in contributed.into_items() {
        menu = menu.child(row);
    }
    menu
}

/// Open a context menu: build a [`ContextMenu`] from the spec (entries emit `SubmitOverlay`, dismiss
/// emits `CloseOverlay`, icons from the action registry, host-assigned quick-pick letters), push it
/// as an Overlay-band **modal** layer (so `top_modal` routes input + `paint_layers` paints it), and
/// register a completion that dispatches the chosen item's action through the central confirm gate.
pub(crate) fn open_dropdown(state: &mut AppState, spec: DropdownSpec) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = spec.source;

    let emit = ChromeIntentEmitter::new(&state.event_proxy, source);

    // **The same builder every declared menu uses.** A dropdown has no declaring widget — the host
    // builds the rows and anchors it — but *how a menu is built* must not depend on that, or the
    // two drift: they already had, one with quick-pick keycaps and one without, which is how the
    // same menu came to have two shapes on screen.
    let items =
        menu_from_items("", "", "", spec.items, &state.action_catalog, &emit);
    let menu = ContextMenu::new("dropdown")
        .child(items)
        .anchor(spec.anchor)
        .centered(spec.centered)
        .on_dismiss(id.closer(&emit))
        // **A chosen entry takes the layer down too, not just a dismissal.** An entry dispatches
        // its own `Intent` now (one builder for every menu), so nothing else resolves this overlay
        // — it used to be `SubmitOverlay`, intercepted by the completion below. Without this the
        // panel hid itself while the layer stayed registered: still modal, still holding the
        // keyboard, so every keybinding was dead until `Escape`.
        .after_select(id.closer(&emit))
        .default_open(true);

    // A menu **captures input and demands a choice**, so it covers for policy purposes even though
    // its panel is small: *a modal is an overlay with coverage*. Its own entries
    // are unaffected — they dispatch their `Intent` on `after_select`, after the layer is down.
    //
    // Without this, the prefix sequence that deliberately falls through the overlay key path
    // (`app/events.rs`, so `prefix+/` can still pick an entry) reaches the router and runs:
    // `prefix+x` with a menu open raised the close-pane confirm, which the blanket `top_modal` rule
    // this replaced had prevented.
    insert_menu_layer(state, id, menu);

    state.needs_redraw = true;
    id
}
