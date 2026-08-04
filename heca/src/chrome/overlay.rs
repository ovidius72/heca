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
use heca_grid_ui::widgets::{ContextMenu, MenuEntry};
use heca_grid_ui::{Button, ButtonVariant, Component, Dialog, HintExt, Point};

use heca_view::{PropMap, ViewNode, WidgetKind};
use super::{ChromeIntentEmitter, FormBindings, LayerBand, LayerId, LayerKind};
use crate::actions::ActionRegistry;
use crate::app::events::AppEvent;
use crate::app::interaction::{dispatch_intent, InteractionIntent, InteractionSource};
use heca_view::Intent;
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
    state.layers.top_modal_id().map(OverlayId)
}

/// Is the tiled area covered by an overlay? The one input `Domain::Overlay` needs
/// (F003/P086/T371) — see [`DynamicLayer::covers_content`](crate::chrome::layers::DynamicLayer).
pub(crate) fn content_covered(state: &AppState) -> bool {
    state.layers.content_covered()
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

    // The chrome intent sink — same shape as the retained chrome tree's emitter (mod.rs): a
    // button's `on_click` posts an `AppEvent::ChromeIntent`, dispatched by the event loop.
    let event_proxy = state.event_proxy.clone();
    let emit: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent,
        });
    });

    let mut forms = FormBindings::default();
    let root = build_modal_root(
        &spec,
        id,
        // The theme this tree is built with: a `PropValue::Color` naming a token resolves against
        // it now. A theme reload rebuilds every overlay, so the token follows (F003/P017/T7).
        &super::chrome_gui_theme(state),
        &emit,
        &mut state.hint_targets,
        &state.action_shortcuts,
        &mut forms,
    );
    // A modal **covers the tiled area** by definition: it scrims the app and demands a decision,
    // so nothing may act on the panes behind it (F003/P086/T371). That is the same protection the
    // router's old blanket "a modal blocks everything" gave, said as a property of the overlay.
    state.layers.insert(
        id.0,
        LayerBand::Modal,
        LayerKind::OnDemand,
        true,
        true,
        root,
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
/// `band`, `modal` and `covers_content` are the caller's: a plugin panel over the scrolling area is
/// `Overlay` + `covers_content: true` + not modal, a rich dialog is `Modal` + both. **No occluder is
/// passed** — `active_hint_targets` reads it from the realized tree's laid-out bounds, which is the
/// invariant this path must not break.
pub(crate) fn open_view_layer(
    state: &mut AppState,
    band: LayerBand,
    kind: LayerKind,
    modal: bool,
    covers_content: bool,
    node: ViewNode,
) -> LayerId {
    let event_proxy = state.event_proxy.clone();
    let emit: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent,
        });
    });
    // Same boundary as `build_modal_root`: `realize` speaks the model's own `Intent` and knows
    // nothing of `InteractionIntent`, so the carrier is put on here.
    let view_emit: super::IntentEmitter = {
        let emit = emit.clone();
        Rc::new(move |intent| emit(InteractionIntent::View(intent)))
    };
    let theme = super::chrome_gui_theme(state);
    let mut forms = FormBindings::default();
    let realized = {
        let mut targets = super::ViewHintTargets(&mut state.hint_targets);
        super::realize(&node, &theme, &view_emit, &mut targets, &mut forms)
    };
    let id = state
        .layers
        .add_view(band, kind, modal, covers_content, node, realized);
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

/// One entry of a dropdown / context menu. The author supplies id/label/action; the host resolves
/// the icon from the action registry (`ActionCatalog::icon`) and wires the intent + quick-pick —
/// the same centralized path as [`ModalAction`], with **no hand-picked glyph and no `prefix+X`
/// label** (the leader doesn't work while the menu is open; a host-assigned single-letter quick-pick
/// that *does* work replaces it).
#[derive(Clone)]
pub struct DropdownItem {
    /// Stable id returned in [`ModalResult::Action`]; also the **catalog name** the icon and label
    /// resolve from — the entry's visual identity (e.g. `"close"`).
    ///
    /// It is deliberately **not** the same thing as what the entry runs: a sidebar "Close pane"
    /// entry has id `close` (so it shows the close icon) but dispatches `close_pane_by_id` with the
    /// row's pane. Identity and behaviour are separate fields.
    pub id: String,
    pub label: String,
    /// What the entry dispatches when chosen: an [`Intent`] — an action **name + args** — routed
    /// through the one dispatch door, so the interaction policy and the confirm gate apply exactly
    /// as they would for a keypress.
    ///
    /// An `Intent` rather than a `WmAction` because `WmAction` is a **closed enum**: a plugin cannot
    /// add a variant, so a menu entry carrying one could only ever run actions heca already has —
    /// which is precisely what blocked plugin-contributed menus (context-menu-5). A name resolves to
    /// a built-in *or* to a plugin's own registered action, indifferently.
    pub intent: Intent,
    pub danger: bool,
    pub enabled: bool,
}

impl DropdownItem {
    /// An enabled, non-destructive entry whose id is also the action it runs (the common case: the
    /// entry's catalog identity and its behaviour coincide, e.g. `zoom_column`).
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let intent = Intent::new(id.clone());
        Self { id, label: label.into(), intent, danger: false, enabled: true }
    }

    /// An entry whose behaviour differs from its visual identity — the id keeps the icon/label
    /// (`close`), while the intent carries the action actually run, with its args
    /// (`close_pane_by_id` + `pane_id`).
    pub fn with_intent(id: impl Into<String>, label: impl Into<String>, intent: Intent) -> Self {
        Self { id: id.into(), label: label.into(), intent, danger: false, enabled: true }
    }
    /// Tint destructive (red) — the confirm gate still applies on dispatch.
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Enable/disable (a disabled entry is dimmed + unselectable).
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

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

/// Open a context menu: build a [`ContextMenu`] from the spec (entries emit `SubmitOverlay`, dismiss
/// emits `CloseOverlay`, icons from the action registry, host-assigned quick-pick letters), push it
/// as an Overlay-band **modal** layer (so `top_modal` routes input + `paint_layers` paints it), and
/// register a completion that dispatches the chosen item's action through the central confirm gate.
pub(crate) fn open_dropdown(state: &mut AppState, spec: DropdownSpec) -> OverlayId {
    let id = OverlayId(state.layers.reserve_id());
    let source = spec.source;

    let event_proxy = state.event_proxy.clone();
    let emit: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(AppEvent::ChromeIntent { source, intent });
    });

    let mut menu = ContextMenu::new().anchor(spec.anchor).centered(spec.centered);
    let mut letters = 'a'..='z';
    for item in &spec.items {
        let carrier = InteractionIntent::ActivateAction(WmAction::SubmitOverlay {
            overlay: id,
            action: item.id.clone(),
        });
        let emit_e = emit.clone();
        let mut entry = MenuEntry::new(item.label.clone(), move || emit_e(carrier.clone()))
            .danger(item.danger)
            .enabled(item.enabled);
        if let Some(glyph) = state.action_catalog.icon(&item.id) {
            entry = entry.icon(glyph);
        }
        // Host-assigned single-letter quick-pick (works while open), rendered as a bordered
        // keycap — the only accelerator shown on the row (no separate global-binding label).
        if item.enabled
            && let Some(k) = letters.next()
        {
            entry = entry.key(k);
        }
        menu = menu.entry(entry);
    }
    let emit_dismiss = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: id });
    let menu = menu.on_dismiss(move || emit_dismiss(close.clone())).open(true);

    // A menu **captures input and demands a choice**, so it covers for policy purposes even though
    // its panel is small: *a modal is an overlay with coverage* (F003/P086/T371). Its own entries
    // are unaffected — they dispatch `SubmitOverlay`, which is intercepted before routing.
    //
    // Without this, the prefix sequence that deliberately falls through the overlay key path
    // (`app/events.rs`, so `prefix+/` can still pick an entry) reaches the router and runs:
    // `prefix+x` with a menu open raised the close-pane confirm, which the blanket `top_modal` rule
    // this replaced had prevented (found by the user, 2026-07-30).
    state.layers.insert(
        id.0,
        LayerBand::Overlay,
        LayerKind::OnDemand,
        true,
        true,
        Box::new(menu),
    );

    let items = spec.items;
    state.overlays.completions.insert(
        id,
        Box::new(move |state, registry, result| {
            if let ModalResult::Action { id: chosen, .. } = result
                && let Some(item) = items.iter().find(|i| i.id == chosen)
            {
                // The entry's Intent goes through the ONE dispatch door, so a built-in and a
                // plugin's own action are dispatched identically — and the interaction policy and
                // the confirm gate still apply (a "Delete workspace" entry prompts exactly as the
                // keybinding does).
                dispatch_intent(
                    state,
                    registry,
                    source,
                    InteractionIntent::View(item.intent.clone()),
                );
            }
        }),
    );
    state.needs_redraw = true;
    id
}

/// Build the realized `Dialog` tree for a modal. Each action becomes a real `Button` wired the
/// SAME centralized way as every chrome button (AGENTS.md "Chrome buttons → action, tooltip,
/// KeyHint — do NOT hand-roll"): a KeyHint target + `on_click` both carry `SubmitOverlay`, and
/// the button is wrapped in [`action_tooltip`](super::action_tooltip) so its tip + shortcut come
/// from the action, never a hand-picked string. The `Dialog` itself owns focus/nav/activation.
fn build_modal_root(
    spec: &ModalSpec,
    id: OverlayId,
    theme: &heca_grid_ui::Theme,
    emit: &ChromeIntentEmitter,
    hints: &mut super::HintTargetRegistry,
    shortcuts: &super::ActionShortcuts,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    // `realize` speaks the model's own `Intent` and knows nothing of `InteractionIntent` or the
    // registry (F003/P017/T009). The carrier is put on here, at the boundary — for the click sink
    // by a wrapping closure, for the pick registry by `ViewHintTargets`.
    let body = {
        let view_emit: super::IntentEmitter = {
            let emit = emit.clone();
            Rc::new(move |intent| emit(InteractionIntent::View(intent)))
        };
        let mut targets = super::ViewHintTargets(hints);
        super::realize(&spec.body, theme, &view_emit, &mut targets, forms)
    };
    let mut dialog = Dialog::new(spec.title.clone()).body_boxed(body);
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
        let hid = hints.register(carrier.clone());
        let emit = emit.clone();
        let button = Button::new(action.label.clone())
            .variant(variant)
            .hint_target(hid)
            .on_click(move || emit(carrier.clone()));
        // Reactive validation: disable this button while a required form field is empty (blocks
        // blank submission). Binds the button's `disabled` signal to the field's live value.
        if let Some(field) = &action.disable_when_empty
            && let Some(sig) = forms.text_signal(field)
        {
            let disabled = button.base().disabled;
            create_effect(move |_| disabled.set(sig.get().trim().is_empty()));
        }
        // Tooltip + live shortcut from the action id — the one centralized path.
        dialog = dialog.action(super::action_tooltip(button, &action.id, &action.label, shortcuts));
    }
    // Esc / scrim dismissal flows through the same emitter as the buttons: a `CloseOverlay`
    // for this overlay, resolved to `ModalResult::Dismissed` in `dispatch_intent`.
    let emit_dismiss = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: id });
    Box::new(
        dialog
            .dismissible(spec.dismissible)
            .on_dismiss(move || emit_dismiss(close.clone()))
            .open(true),
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
    state.layers.remove(overlay.0);
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
    use super::*;
    use heca_view::PropValue;

    fn noop_emit() -> ChromeIntentEmitter {
        Rc::new(|_| {})
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

    #[test]
    fn build_registers_one_submit_intent_per_action() {
        let spec = ModalSpec::message("Delete pane?", "Gone forever.")
            .action(ModalAction::new("cancel", "Cancel"))
            .action(ModalAction::new("confirm", "Delete").danger(true))
            .dismissible(false);
        let id = OverlayId(super::super::LayerRegistry::default().reserve_id());
        let mut hints = super::super::HintTargetRegistry::default();
        let shortcuts = super::super::ActionShortcuts::default();
        let before = hints.checkpoint();
        let root = build_modal_root(&spec, id, &heca_grid_ui::Theme::default(), &noop_emit(), &mut hints, &shortcuts, &mut FormBindings::default());

        // Two actions → two hint targets, each a SubmitOverlay for this overlay.
        assert_eq!(hints.checkpoint() - before, 2);
        for (offset, action_id) in [(0, "cancel"), (1, "confirm")] {
            let intent = hints
                .get(heca_grid_ui::HintTargetId::new(before + offset))
                .unwrap();
            assert!(
                matches!(
                    intent,
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
}
