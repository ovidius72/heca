//! Opening a dialog, or a panel from a description, as a layer.

use heca_grid_ui::reactive::{SignalGet, SignalUpdate, create_effect};
use heca_grid_ui::{Button, ButtonVariant, Component, Dialog, HintExt as _};
use heca_view::ViewNode;

use super::spec::{ModalResult, ModalSpec, OverlayId};
use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::{ChromeIntentEmitter, FormBindings, LayerId, LayerKind};

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
    // said nothing about *which* surface acted.
    let emit = crate::chrome::layer_emitter(
        &state.event_proxy,
        crate::chrome::surface_key_of(None, id.0),
    );

    let mut forms = FormBindings::default();
    let root = build_modal_root(
        &spec,
        id,
        // The theme this tree is built with: a `PropValue::Color` naming a token resolves against
        // it now. A theme reload rebuilds every overlay, so the token follows.
        &crate::chrome::chrome_gui_theme(state),
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

/// Open a layer whose content is a **description** — the plugin / config / RPC path.
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
// The door plugins will open a described panel through; nothing in the app calls it yet.
#[allow(dead_code)]
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
    let emit = crate::chrome::layer_emitter(&state.event_proxy, crate::chrome::surface_key_of(None, id));
    let view_emit = emit.intent_sink();
    let theme = crate::chrome::chrome_gui_theme(state);
    let mut forms = FormBindings::default();
    // The identity rule's declarative half, said once per description rather than per realize —
    // this node is realized again on every theme reload.
    crate::chrome::input::identity::report_unkeyed_description("view layer", &node);
    let mut realized = crate::chrome::realize(&node, &theme, &view_emit, &mut forms);
    // **The surface declares what it obscures, on itself.** A described overlay says it the same
    // way a native one does, so the two authoring paths produce the same tree.
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

/// Build the realized `Dialog` tree for a modal. Each action becomes a real `Button` wired the
/// SAME centralized way as every chrome button (AGENTS.md "Chrome buttons → action, tooltip,
/// KeyHint — do NOT hand-roll"): its click and its `prefix+/` pick both carry `SubmitOverlay`, and
/// the button is wrapped in [`action_tooltip`](crate::chrome::action_tooltip) so its tip + shortcut come
/// from the action, never a hand-picked string. The `Dialog` itself owns focus/nav/activation.
pub(super) fn build_modal_root(
    spec: &ModalSpec,
    id: OverlayId,
    theme: &heca_grid_ui::Theme,
    emit: &ChromeIntentEmitter,
    shortcuts: &crate::chrome::ActionShortcuts,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    // One sink for the body's click and its `prefix+/` pick alike.
    crate::chrome::input::identity::report_unkeyed_description("modal body", &spec.body);
    let body = crate::chrome::realize(&spec.body, theme, &emit.intent_sink(), forms);
    let mut dialog = Dialog::new(spec.title.clone())
        .child(body)
        .danger(spec.danger);
    for action in &spec.actions {
        let variant = if action.danger {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Secondary
        };
        let fire = id.submitter(action.id.clone(), emit);
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
        dialog = dialog.action(crate::chrome::action_tooltip(
            button.on_hint(hint),
            &action.id,
            &action.label,
            shortcuts,
        ));
    }
    // Esc / scrim dismissal flows through the same emitter as the buttons, resolved to
    // `ModalResult::Dismissed` in `dispatch_intent`.
    Box::new(
        dialog
            .dismissible(spec.dismissible)
            .on_dismiss(id.closer(emit))
            .default_open(true),
    )
}
