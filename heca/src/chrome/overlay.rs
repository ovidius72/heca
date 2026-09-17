//! `OverlayHost` — the host-owned overlay stack (plan §2.7.1 / §2.7.2), built **on** the
//! [`LayerRegistry`](super::LayerRegistry) rather than as a parallel stack.
//!
//! A caller (a handler, or later a plugin/RPC) submits a [`ModalSpec`] and a completion; the
//! host [`realize`](super::realize)s the spec's `body` [`ViewNode`], injects a real
//! [`Button`](heca_grid_ui::Button) per [`ModalAction`] — each carrying the overlay's id via
//! [`WmAction::SubmitOverlay`] — wraps them in a [`Dialog`](heca_grid_ui::Dialog), and pushes
//! it as a `Modal`-band layer. Because the buttons are real components they are hint targets
//! and focus-traversable for free. Confirmation/dismissal come back as overlay-control actions
//! (`SubmitOverlay` / `CloseOverlay`) resolved by [`resolve`] — the single point that pops the
//! layer and runs the caller's completion. RPC drives the exact same actions by id.
//!
//! Seam module: `open_modal` is consumed by the confirm-dialog migration (and later plugins);
//! carries `#![allow(dead_code)]` like the sibling chrome seam modules until then.
#![allow(dead_code)]

use std::collections::HashMap;
use std::rc::Rc;

use heca_grid_ui::reactive::{create_effect, SignalGet, SignalUpdate};
use heca_grid_ui::widgets::{ContextMenu, Menu, MenuAnchor};
use heca_grid_ui::{Button, ButtonVariant, Component, ComponentExt as _, Dialog, Point};

use heca_view::{PropMap, ViewNode, WidgetKind};
use super::{ChromeIntentEmitter, FormBindings, LayerId, LayerKind};
use crate::host::App;
use crate::providers::ChromeCtx;
use crate::actions::ActionRegistry;
use crate::app::interaction::{InteractionIntent, InteractionSource};
use crate::app_state::AppState;
use crate::input::WmAction;

/// Opaque, stable id for an open overlay — the same value as its backing
/// [`LayerId`](super::LayerId). Public because [`WmAction`] carries it (an action targets a
/// specific overlay by id); the inner layer id stays crate-private.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverlayId(pub(crate) LayerId);

/// One bottom action button of a modal. The author supplies only id/label/danger; the
/// [`OverlayHost`] wires the rest through the same centralized path as every chrome button —
/// a KeyHint target + a tooltip resolved from the action, never a hand-picked shortcut string.
/// `id` doubles as the action name the tooltip resolves its shortcut from (`action_tooltip`).
#[derive(Clone, Debug)]
pub struct ModalAction {
    /// Comes back in [`ModalResult::Action`]; also the action name for the tooltip's shortcut.
    pub id: String,
    pub label: String,
    /// Tint with the danger hue (destructive action).
    pub danger: bool,
    /// If `Some(field)`, this button is **disabled while the named form field is empty** (trimmed)
    /// — used to block submission until a required [`Input`](heca_grid_ui::Input) has content (e.g.
    /// a rename dialog's OK). The host binds the button's disabled state to the field's live value.
    pub disable_when_empty: Option<String>,
}

impl ModalAction {
    /// A plain action button.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            danger: false,
            disable_when_empty: None,
        }
    }
    /// Tint with the danger hue (destructive primary action).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Disable this button while the named form field is empty (blocks blank submission).
    pub fn disabled_when_empty(mut self, field: impl Into<String>) -> Self {
        self.disable_when_empty = Some(field.into());
        self
    }
}

/// A modal specification: a title, a declarative body tree, and the bottom action buttons.
pub struct ModalSpec {
    pub title: String,
    /// The dialog body — a full [`ViewNode`] tree (§2.6.2), so a modal can hold a table / form
    /// / list, not just text. [`ModalSpec::message`] wraps a single `Label`.
    pub body: ViewNode,
    pub actions: Vec<ModalAction>,
    /// Tint the panel/primary action as destructive.
    pub danger: bool,
    /// `false` = forced decision (Esc / scrim swallowed) — mirrors `Dialog::dismissible`.
    pub dismissible: bool,
}

impl ModalSpec {
    /// A simple message modal: `title` over a one-line body `message`, no actions yet (add
    /// them with [`action`](ModalSpec::action)).
    pub fn message(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: ViewNode::new(WidgetKind::Label).text(message),
            actions: Vec::new(),
            danger: false,
            dismissible: true,
        }
    }
    /// Append an action button.
    pub fn action(mut self, action: ModalAction) -> Self {
        self.actions.push(action);
        self
    }
    /// Mark destructive (tints the primary action).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Force an explicit choice — Esc / scrim are swallowed without dismissing.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }
}

/// The resolved outcome of a modal, handed to the caller's completion.
#[derive(Clone, Debug, PartialEq)]
pub enum ModalResult {
    /// A button (or RPC) chose action `id`; `data` carries the modal body's named value fields,
    /// each read at submit time — `Input` → `Text`, `Toggle`/`Checkbox` → `Bool`, `Select` → the
    /// chosen option's value (`Text`). Empty when the body has no named field (e.g. a plain confirm).
    Action { id: String, data: PropMap },
    /// Esc / scrim / `CloseOverlay` — no action chosen.
    Dismissed,
}

/// A caller's completion, run once when the overlay resolves. It receives the live
/// [`AppState`] + [`ActionRegistry`] (so it can dispatch a follow-up action) and the result.
type OverlayCompletion = Box<dyn FnOnce(&mut AppState, &ActionRegistry, ModalResult)>;

/// The host-owned overlay stack: pending completions keyed by overlay id. The overlay's
/// *visual* stack lives in the [`LayerRegistry`](super::LayerRegistry); this holds only the
/// result callbacks. Kept on [`AppState`].
#[derive(Default)]
pub struct OverlayHost {
    completions: HashMap<OverlayId, OverlayCompletion>,
    /// Per-overlay form bindings — the named value widgets in the modal body, read into
    /// [`ModalResult::Action`]'s `data` when the overlay is submitted.
    forms: HashMap<OverlayId, FormBindings>,
}

impl OverlayHost {
    /// Register what runs when overlay `id` resolves.
    ///
    /// The map itself stays private: a completion is only ever *installed* beside the layer that
    /// will resolve it, and only ever *run* by [`resolve`], which removes it. This is the door for
    /// an overlay built outside this module (the command palette, F003/P085/T358).
    pub(crate) fn on_resolve(
        &mut self,
        id: OverlayId,
        completion: impl FnOnce(&mut AppState, &ActionRegistry, ModalResult) + 'static,
    ) {
        self.completions.insert(id, Box::new(completion));
    }
}

/// The front-most open **modal** overlay — the one capturing input — if any. The input path
/// routes keyboard/pointer to its layer root (a self-contained [`Dialog`](heca_grid_ui::Dialog))
/// and swallows everything else while it's up.
pub(crate) fn top_modal(state: &AppState) -> Option<OverlayId> {
    state.layers.top_modal_id(&state.window_root).map(OverlayId)
}

/// Is the tiled area covered by an overlay? The one input `Domain::Overlay` needs
/// (F003/P086/T371) — see [`DynamicLayer::lock`](crate::chrome::layers::DynamicLayer).
pub(crate) fn content_covered(state: &AppState) -> bool {
    state.layers.content_covered(&state.window_root)
}

/// Open a modal: realize its body + inject id-carrying action buttons, push it as a
/// `Modal`-band layer, and register `completion` to run when it resolves. Returns the
/// [`OverlayId`] (RPC keeps it to drive `SubmitOverlay`/`CloseOverlay`).
pub(crate) fn open_modal(
    state: &mut AppState,
    spec: ModalSpec,
    completion: impl FnOnce(&mut AppState, &ActionRegistry, ModalResult) + 'static,
) -> OverlayId {
    // Reserve the id first so the action buttons can carry it (they're built before the layer
    // is inserted).
    let id = OverlayId(state.layers.reserve_id());

    // The chrome intent sink for this layer's own tree: a button's `on_click` posts an
    // `AppEvent::ChromeIntent` stamped with the layer it was declared in, dispatched by the event
    // loop. This used to claim `MouseContent`, which was untrue of a button reached by keyboard and
    // said nothing about *which* surface acted (F003/P082/T416).
    let emit = super::layer_emitter(&state.event_proxy, super::surface_key_of(None, id.0));

    let mut forms = FormBindings::default();
    let root = build_modal_root(
        &spec,
        id,
        // The theme this tree is built with: a `PropValue::Color` naming a token resolves against
        // it now. A theme reload rebuilds every overlay, so the token follows (F003/P017/T7).
        &super::chrome_gui_theme(state),
        &emit,
        &state.action_shortcuts,
        &mut forms,
    );
    // **Nothing about the surface is said here.** A `Dialog` locks what is behind it because it is
    // a dialog, and an open layer holds focus, which IS how it takes the keyboard. Both travel with
    // the widget, so this path cannot disagree with a dialog raised any other way.
    let parent = state.layers.current();
    state.layers.insert(
        id.0,
        parent,
        LayerKind::OnDemand,
        root,
        &mut state.window_root,
    );
    state.overlays.completions.insert(id, Box::new(completion));
    state.overlays.forms.insert(id, forms);
    state.needs_redraw = true;
    id
}

/// Open a layer whose content is a **description** — the plugin / config / RPC path
/// (F003/P082/T339).
///
/// The counterpart of [`open_modal`] for an author that cannot hand over a native tree. The
/// `ViewNode` is realized through the **one** bridge, exactly as a modal's body is, and the layer
/// keeps the node beside the realized tree so a theme reload or a plugin update can re-realize from
/// the description rather than from whatever the tree has become.
///
/// `parent` and `lock` are the caller's. `parent` is **what opened this** — pass
/// `state.layers.current()` for a panel raised from wherever the user is, so it sits above that
/// surface and goes with it; pass `None` for a surface that belongs to the base context. A plugin
/// panel over the scrolling area is `lock: true`.
///
/// **Whether it takes the keyboard is not passed**, because it is not a decision anyone makes here:
/// a surface that wants keys holds focus, so a described overlay that opens takes them by opening,
/// exactly as a native one does. **No occluder is passed either** — the hint walk reads it from the
/// realized tree's laid-out bounds, which is the invariant this path must not break.
pub(crate) fn open_view_layer(
    state: &mut AppState,
    name: Option<String>,
    parent: Option<LayerId>,
    kind: LayerKind,
    lock: bool,
    node: ViewNode,
) -> LayerId {
    // Reserved before the tree is built, because the tree's intent sink names the layer it lives in
    // — a plugin's panel is judged by *which* surface acted, exactly as the exposé is.
    let id = state.layers.reserve_id();
    let emit = super::layer_emitter(&state.event_proxy, super::surface_key_of(None, id));
    // Same boundary as `build_modal_root`: `realize` speaks the model's own `Intent` and knows
    // nothing of `InteractionIntent`, so the carrier is put on here.
    let view_emit: super::IntentEmitter = {
        let emit = emit.clone();
        Rc::new(move |intent| emit.fire(InteractionIntent::View(intent)))
    };
    let theme = super::chrome_gui_theme(state);
    let mut forms = FormBindings::default();
    // The identity rule's declarative half, said once per description rather than per realize —
    // this node is realized again on every theme reload (F003/P082/T444).
    super::identity::report_unkeyed_description("view layer", &node);
    let mut realized = super::realize(&node, &theme, &view_emit, &mut forms);
    // **The surface declares what it obscures, on itself.** A described overlay says it the same
    // way a native one does, so the two authoring paths produce the same tree (F003/P097/T499).
    // A described overlay declares coverage the same way a native one does. Whether it takes
    // the keyboard is read from the tree, exactly as for a native one.
    realized.base_mut().lock = lock;
    let id = state.layers.add_view(
        id,
        name,
        parent,
        kind,
        node,
        realized,
        &mut state.window_root,
    );
    state.needs_redraw = true;
    id
}

/// Read the current values of an overlay's named body fields into a [`PropMap`] — the `data`
/// handed back in [`ModalResult::Action`]. Empty if the overlay has no form (e.g. a plain
/// confirm) or is already gone.
pub(crate) fn collect_form(state: &AppState, overlay: OverlayId) -> PropMap {
    state
        .overlays
        .forms
        .get(&overlay)
        .map(FormBindings::collect)
        .unwrap_or_default()
}

// `DropdownItem` lives in `heca-view` now (F003/P097/T501): a menu entry is pure data carrying an
// `Intent`, and a **described** tree must be able to declare one. Re-exported here so the app-side
// name a hundred call sites already use keeps working — one type, not a second one beside it.
pub use heca_view::DropdownItem;

/// A cursor-anchored dropdown / context menu spec — the pointer / `OpenContextMenu` counterpart to
/// [`ModalSpec`]. A data description a native handler **or** a plugin submits to [`open_dropdown`].
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
/// panel is small: *a modal is an overlay with coverage* (F003/P086/T371). Without that, the prefix
/// sequence that deliberately falls through the overlay key path (so `prefix+/` can still pick an
/// entry) reaches the router and runs — `prefix+x` with a menu open raised the close-pane confirm
/// (found by the user, 2026-07-30).
fn insert_menu_layer(state: &mut AppState, id: OverlayId, panel: ContextMenu) {
    let parent = state.layers.current();
    state.layers.insert(
        id.0,
        parent,
        LayerKind::OnDemand,
        Box::new(panel),
        &mut state.window_root,
    );
}

/// **Present a menu a widget declared** (F004/P084/T395).
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
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: Some(id) });
    let after = emit.clone();
    let closing = close.clone();
    let dismiss = emit.clone();
    let panel = anchor
        .open(ctx)
        .after_select(move || after.fire(closing.clone()))
        .on_dismiss(move || dismiss.fire(close.clone()));

    insert_menu_layer(state, id, panel);
    state.needs_redraw = true;
    id
}

/// Append every mounted provider's rows for the menu's [`name`](Menu::name).
///
/// A menu without a name is closed: it built its own rows and nothing else may add to it.
/// Where the menu was opened, when a pointer opened it. `None` for a keyboard-opened menu, which
/// has no cell under it — which is exactly why "Open link" never appears on one.
fn anchor_point(anchor: &MenuAnchor) -> Option<Point> {
    match anchor {
        MenuAnchor::At(p) => Some(*p),
        _ => None,
    }
}

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
    let target = super::context_menu::target_from_subject(state, subject, at);
    let ctx = ChromeCtx::new(App::new(&state.chrome_state));
    let plugin = super::context_menu::plugin_providers_for(&state.chrome_host, &ctx, &path);
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
        super::context_menu::menu_from_items("", "", "", items, &state.action_catalog, emit);
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
    // same menu came to have two shapes on screen (Antonio, 2026-08-07).
    let items = super::context_menu::menu_from_items(
        "",
        "",
        "",
        spec.items,
        &state.action_catalog,
        &emit,
    );
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: Some(id) });
    let emit_dismiss = emit.clone();
    let dismiss_close = close.clone();
    let emit_after = emit.clone();
    let menu = ContextMenu::new("dropdown")
        .child(items)
        .anchor(spec.anchor)
        .centered(spec.centered)
        .on_dismiss(move || emit_dismiss.fire(dismiss_close.clone()))
        // **A chosen entry takes the layer down too, not just a dismissal.** An entry dispatches
        // its own `Intent` now (one builder for every menu), so nothing else resolves this overlay
        // — it used to be `SubmitOverlay`, intercepted by the completion below. Without this the
        // panel hid itself while the layer stayed registered: still modal, still holding the
        // keyboard, so every keybinding was dead until `Escape` (Antonio, 2026-08-07 —
        // "`prefix+>` then float/unfloat makes it unstable, keybindings don't work").
        .after_select(move || emit_after.fire(close.clone()))
        .default_open(true);

    // A menu **captures input and demands a choice**, so it covers for policy purposes even though
    // its panel is small: *a modal is an overlay with coverage* (F003/P086/T371). Its own entries
    // are unaffected — they dispatch their `Intent` on `after_select`, after the layer is down.
    //
    // Without this, the prefix sequence that deliberately falls through the overlay key path
    // (`app/events.rs`, so `prefix+/` can still pick an entry) reaches the router and runs:
    // `prefix+x` with a menu open raised the close-pane confirm, which the blanket `top_modal` rule
    // this replaced had prevented (found by the user, 2026-07-30).
    insert_menu_layer(state, id, menu);

    state.needs_redraw = true;
    id
}

/// Build the realized `Dialog` tree for a modal. Each action becomes a real `Button` wired the
/// SAME centralized way as every chrome button (AGENTS.md "Chrome buttons → action, tooltip,
/// KeyHint — do NOT hand-roll"): its click and its `prefix+/` pick both carry `SubmitOverlay`, and
/// the button is wrapped in [`action_tooltip`](super::action_tooltip) so its tip + shortcut come
/// from the action, never a hand-picked string. The `Dialog` itself owns focus/nav/activation.
fn build_modal_root(
    spec: &ModalSpec,
    id: OverlayId,
    theme: &heca_grid_ui::Theme,
    emit: &ChromeIntentEmitter,
    shortcuts: &super::ActionShortcuts,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    // `realize` speaks the model's own `Intent` and knows nothing of `InteractionIntent`
    // (F003/P017/T009). The carrier is put on here, at the boundary, by a wrapping closure — one
    // sink now, for the click and the `prefix+/` pick alike.
    let body = {
        let view_emit: super::IntentEmitter = {
            let emit = emit.clone();
            Rc::new(move |intent| emit.fire(InteractionIntent::View(intent)))
        };
        super::identity::report_unkeyed_description("modal body", &spec.body);
        super::realize(&spec.body, theme, &view_emit, forms)
    };
    let mut dialog = Dialog::new(spec.title.clone()).body(body);
    for action in &spec.actions {
        let variant = if action.danger {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Secondary
        };
        let carrier = InteractionIntent::ActivateAction(WmAction::SubmitOverlay {
            overlay: id,
            action: action.id.clone(),
        });
        let emit = emit.clone();
        let fire = move || emit.fire(carrier.clone());
        let hint = fire.clone();
        let button = Button::new(action.label.clone())
            .variant(variant)
            .on_click(fire);
        // Reactive validation: disable this button while a required form field is empty (blocks
        // blank submission). Binds the button's `disabled` signal to the field's live value.
        if let Some(field) = &action.disable_when_empty
            && let Some(sig) = forms.text_signal(field)
        {
            let disabled = button.base().disabled;
            create_effect(move |_| disabled.set(sig.get().trim().is_empty()));
        }
        // Tooltip + live shortcut from the action id — the one centralized path. The pick
        // declaration goes on the button itself, which is what `on_hint` is for (AGENTS §
        // 5a): a `KeyHint` wrapper used to carry it, costing the button a second pick target.
        dialog = dialog.action(super::action_tooltip(
            button.on_hint(hint),
            &action.id,
            &action.label,
            shortcuts,
        ));
    }
    // Esc / scrim dismissal flows through the same emitter as the buttons: a `CloseOverlay`
    // for this overlay, resolved to `ModalResult::Dismissed` in `dispatch_intent`.
    let emit_dismiss = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: Some(id) });
    Box::new(
        dialog
            .dismissible(spec.dismissible)
            .on_dismiss(move || emit_dismiss.fire(close.clone()))
            .default_open(true),
    )
}

/// Resolve an open overlay: pop its layer + action metadata and run its completion with
/// `result`. The single confirm/dismiss point — a button press, a KeyHint pick, and RPC all
/// funnel here via `SubmitOverlay`/`CloseOverlay` (dispatched in `dispatch_intent`). No-op if
/// the overlay is already gone (double-resolve safe).
pub(crate) fn resolve(
    state: &mut AppState,
    registry: &ActionRegistry,
    overlay: OverlayId,
    result: ModalResult,
) {
    let completion = state.overlays.completions.remove(&overlay);
    state.overlays.forms.remove(&overlay);
    state.layers.remove(&mut state.window_root, overlay.0);
    state.needs_redraw = true;
    // **No mode is restored** (F003/P086/T365). A container's keyboard focus is not a mode, an
    // overlay never takes it away, and it is simply still there when the overlay closes — so there
    // is nothing to put back and no origin to record.

    if let Some(comp) = completion {
        comp(state, registry, result);
    }
}

#[cfg(test)]
mod tests {
    use heca_view::Intent;
    use super::*;
    use heca_view::PropValue;

    fn noop_emit() -> ChromeIntentEmitter {
        ChromeIntentEmitter::of(InteractionSource::MouseContent, |_, _| {})
    }

    /// **A contributed row runs its own action, exactly as a row the declarer wrote does.**
    /// ⚠️ Ran red against its own bug.
    ///
    /// Rows contributed to somebody else's menu used to be built by hand here and wired to
    /// `SubmitOverlay`, which resolves the overlay and hands the chosen id to a *completion*. A
    /// menu presented from a widget's own declaration has no completion — nobody registers one —
    /// so a contributed row opened fine and then did **nothing at all** when chosen, and nothing
    /// failed anywhere.
    ///
    /// It went unnoticed while contributions were the rare case. The moment a pane's whole menu
    /// arrived this way, every entry in it was dead. This pins that
    /// the merge goes through the one conversion, so the two kinds of row are wired the same way.
    #[test]
    fn a_contributed_row_carries_the_action_it_runs() {
        let fired = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit = ChromeIntentEmitter::of(InteractionSource::MouseContent, move |_, intent| {
            if let InteractionIntent::View(i) = intent {
                sink.borrow_mut().push(i.action.clone());
            }
        });

        let menu = crate::chrome::context_menu::menu_from_items(
            "",
            "",
            "",
            vec![heca_view::DropdownItem::with_intent(
                "close",
                "Close pane",
                Intent::new("close_pane_by_id").arg("pane_id", PropValue::Int(4)),
            )],
            &crate::actions::ActionCatalog::default(),
            &emit,
        );

        let rows = menu.into_items();
        assert_eq!(rows.len(), 1, "one entry in, one row out");
        rows[0].activate();
        assert_eq!(
            fired.borrow().as_slice(),
            &["close_pane_by_id".to_string()],
            "choosing the row runs the action the entry declared — not an overlay-resolve that \
             lands on a completion nobody registered",
        );
    }

    #[test]
    fn dropdown_item_builders() {
        // The plain constructor: the entry's id IS the action it runs.
        let close = DropdownItem::new("close", "Close pane").danger(true);
        assert_eq!(close.id, "close");
        assert_eq!(close.label, "Close pane");
        assert_eq!(close.intent.action, "close", "id doubles as the action");
        assert!(close.danger && close.enabled, "danger set, enabled by default");

        let disabled = DropdownItem::new("dup", "Duplicate").enabled(false);
        assert!(!disabled.enabled && !disabled.danger);
    }

    /// An entry may run something other than its id — the id keeps the icon/label identity while
    /// the intent carries the real action + args. This is what lets a plugin entry dispatch a
    /// plugin action, which has no `WmAction` variant at all.
    #[test]
    fn dropdown_item_can_run_an_action_other_than_its_id() {
        let item = DropdownItem::with_intent(
            "close",
            "Delete pane",
            Intent::new("close_pane_by_id").arg("pane_id", PropValue::Int(7)),
        );
        assert_eq!(item.id, "close", "identity (icon) stays `close`");
        assert_eq!(item.intent.action, "close_pane_by_id", "behaviour differs");
        assert_eq!(item.intent.args.get("pane_id"), Some(&PropValue::Int(7)));
    }

    #[test]
    fn message_spec_wraps_body_in_a_label() {
        let spec = ModalSpec::message("Delete pane?", "This cannot be undone.");
        assert_eq!(spec.body.kind, WidgetKind::Label);
        assert_eq!(
            spec.body.props.get("text").and_then(PropValue::as_text),
            Some("This cannot be undone."),
        );
        assert!(spec.dismissible && !spec.danger && spec.actions.is_empty());
    }

    /// Every modal action button declares what a `prefix+/` pick does to it, and it is the same
    /// `SubmitOverlay` its click carries — so the picker reaches a dialog's buttons with nothing
    /// registered anywhere.
    #[test]
    fn each_action_button_declares_the_submit_its_click_would_fire() {
        let spec = ModalSpec::message("Delete pane?", "Gone forever.")
            .action(ModalAction::new("cancel", "Cancel"))
            .action(ModalAction::new("confirm", "Delete").danger(true))
            .dismissible(false);
        let id = OverlayId(super::super::LayerRegistry::default().reserve_id());
        let shortcuts = super::super::ActionShortcuts::default();
        let fired: std::rc::Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
        let emit: ChromeIntentEmitter = {
            let fired = fired.clone();
            ChromeIntentEmitter::of(InteractionSource::MouseContent, move |_, intent| {
                fired.borrow_mut().push(intent)
            })
        };
        let mut root = build_modal_root(&spec, id, &heca_grid_ui::Theme::default(), &emit, &shortcuts, &mut FormBindings::default());

        let targets = heca_grid_ui::collect_hints(root.as_ref());
        assert_eq!(targets.len(), 2, "two actions → two pick targets");
        for (offset, action_id) in [(0, "cancel"), (1, "confirm")] {
            assert!(heca_grid_ui::fire_hint(root.as_mut(), &targets[offset].0));
            let intent = fired.borrow().last().cloned().unwrap();
            assert!(
                matches!(
                    &intent,
                    InteractionIntent::ActivateAction(WmAction::SubmitOverlay { overlay, action })
                        if *overlay == id && action == action_id
                ),
                "target {offset} should submit '{action_id}' to this overlay, got {intent:?}",
            );
        }

        // Structure: the Dialog root composes a base Overlay (the blocking layer) whose
        // single child is the panel holding [title, body, action-row(2)].
        let overlay = &root.base().children[0];
        let panel = &overlay.base().children[0];
        assert_eq!(panel.base().children.len(), 3, "title + body + action row");
        assert_eq!(panel.base().children[2].base().children.len(), 2, "two buttons");
    }

    /// **A modal's action button is one pick target, not two** (docs/hint-architecture.md § 5a).
    ///
    /// `on_hint` is on `ComponentExt`, so the `Button` already answers for itself — wrapping it in
    /// a `KeyHint` used to add a second pick target on top of the button's own actionability, the
    /// same bug the exposé's pane card had until 2026-09-15. This pins `build_modal_root` to the
    /// fix: one action button, one target — and that the target IS the button, not a wrapper
    /// standing in front of it.
    ///
    /// The count alone can't catch a reintroduced wrapper: `collect_hints`'s own "one letter per
    /// thing, not per layer" rule already collapses a transparent `KeyHint` and the single
    /// actionable child it holds into one target (`heca-grid-ui/src/hint/collect.rs`), so a
    /// wrapped button and a bare one both report exactly one. What differs is *which node* survives
    /// — the wrapper's declaration outranks the button's mere actionability, so a reintroduced
    /// `KeyHint` would make the surviving target the transparent wrapper, not the button.
    #[test]
    fn a_modal_button_is_one_pick_target_not_two() {
        let spec = ModalSpec::message("Delete pane?", "Gone forever.")
            .action(ModalAction::new("confirm", "Delete").danger(true))
            .dismissible(false);
        let id = OverlayId(super::super::LayerRegistry::default().reserve_id());
        let shortcuts = super::super::ActionShortcuts::default();
        let emit = noop_emit();
        let root = build_modal_root(
            &spec,
            id,
            &heca_grid_ui::Theme::default(),
            &emit,
            &shortcuts,
            &mut FormBindings::default(),
        );

        let targets = heca_grid_ui::collect_hints(root.as_ref());
        assert_eq!(
            targets.len(),
            1,
            "one action button must be exactly one pick target, not the button plus its wrapper",
        );

        let mut node: &dyn Component = root.as_ref();
        for &step in &targets[0].0 {
            node = node.base().children[step].as_ref();
        }
        assert!(
            !node.base().transparent,
            "the pick target must be the button itself, not a transparent KeyHint wrapped \
             around it",
        );
    }
}
