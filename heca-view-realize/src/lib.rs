//! `realize` — the mapper from the declarative [`ViewNode`] model to a retained grid-ui
//! [`Component`] tree.
//!
//! [`ViewNode`] is the pure, serializable UI *description* (same model authored in Rust or
//! shipped by a WASM plugin), and it lives in `heca-view` with no dependencies at all. This
//! crate is the other half: it owns the grid-ui dependency and the theme wiring, so the model
//! stays free of both.
//!
//! It sits **below the app** on purpose. Everything it needs from a host
//! arrives as an argument, so anything that can build widgets can render a described tree —
//! including `heca-renderer`'s showcase, which is below `heca` and could not call this at all
//! while it lived in the app crate.
//!
//! One host service is threaded in — `emit`, an [`IntentEmitter`]: a realized actionable widget
//! fires the node's `Intent` on **click**. It is a **narrow seam** over the model's own [`Intent`],
//! not an app type, so this mapper can live and be called below `heca`. What an
//! intent means is the host's business; the app wraps it as `InteractionIntent::View`.
//!
//! The universal picker (`prefix+/`) needs no seam at all: a node's `hint` event (defaulting to its
//! `press`) is written straight into the widget's own `Base::hint` slot, and the framework collects
//! the declarations out of the laid-out tree ([`collect_hints`](heca_grid_ui::collect_hints)). There
//! used to be a `HintTargets` registry sink here, mirroring a host-side registry that mapped an
//! opaque id back to a `pub(crate)` app enum — two doors onto one feature, and the plugin-facing
//! one was the second-class half.
//!
//! Adding a widget = one [`WidgetKind`](heca_view::WidgetKind) arm here (+ its variant in the model).
//!
//! # Coverage
//! **Every `WidgetKind` realizes to a live widget.** The one exception is `ScrollBar`, which is
//! **host-only by design**: its state is live host signals (`content_extent` / `viewport_extent` /
//! `offset`), and static, serializable data fundamentally cannot drive a signal — a declarative one
//! would render a dead control. A plugin that needs scrolling uses `Scroll` (a `ScrollRegion`),
//! which owns its own offset. The same rule applies to an individual *builder* that binds a host
//! signal (e.g. `DockFrame::rail`): **a widget whose state is a live host signal is host-only.**
//!
//! This is enforced, not merely stated: `every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones`
//! walks [`WidgetKind::ALL`](heca_view::WidgetKind::ALL) and fails if a kind produces neither children
//! nor paint — so a new kind added without an arm cannot silently render an empty container.
//!

use std::rc::Rc;

use heca_grid_ui::reactive::Signal;
use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
use heca_grid_ui::{Component, Glyph, Theme};
use heca_view::{Intent, PropMap, PropValue, ViewNode};

use crate::intents::hint_intent;
use crate::kinds::realize_kind;
use crate::style::{merge_style_half, resolve_color};
use crate::vocab::{hint_placement, hint_tone, tooltip_side};

/// Where a realized tree's intents go: a widget fires the node's own [`Intent`], and the host
/// wraps it in whatever it dispatches (the app wraps it as `InteractionIntent::View`).
///
/// `Rc` because every actionable widget clones the sink into its own callback.
pub type IntentEmitter = Rc<dyn Fn(Intent)>;

/// Reads a form field's current value at submit time. Boxed because the concrete widget signal
/// type varies (String / bool / …). Native-side only (never crosses the plugin boundary).
type FieldReader = Box<dyn Fn() -> PropValue>;

/// The named value fields a realized tree exposes, collected into a [`PropMap`] when the overlay
/// is submitted (→ the host's `ModalResult::Action` `data`). A value node opts in by
/// carrying a `"name"` prop; `realize` binds a reader over its live value signal. Order is
/// registration order (deterministic).
#[derive(Default)]
pub struct FormBindings {
    fields: Vec<(String, FieldReader)>,
    /// Live value signals of named **text** fields (`Input`s), for reactive validation — e.g.
    /// disabling a submit button while a required field is empty. Populated alongside `fields`.
    text_signals: Vec<(String, Signal<String>)>,
}

impl FormBindings {
    fn bind(&mut self, name: String, reader: FieldReader) {
        self.fields.push((name, reader));
    }

    /// Record a named text field's live value signal (for validation-driven UI like a disabled
    /// submit button). Only `Input` nodes register here.
    fn bind_text_signal(&mut self, name: String, sig: Signal<String>) {
        self.text_signals.push((name, sig));
    }

    /// The live value signal of a named text field, if it is an `Input`. `None` for non-text
    /// fields or unknown names.
    pub fn text_signal(&self, name: &str) -> Option<Signal<String>> {
        self.text_signals
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| *s)
    }

    /// Read every bound field's current value into a name→value map.
    pub fn collect(&self) -> PropMap {
        self.fields.iter().map(|(k, r)| (k.clone(), r())).collect()
    }
}

/// Realize a [`ViewNode`] (and its subtree) into a retained grid-ui component.
///
/// Returns a `Box<dyn Component>` because the produced widget type depends on the runtime
/// [`kind`](heca_view::WidgetKind). NB: `Box<dyn Component>` is **not** itself `Component`, so a
/// container can't take it via `Parent::child` (which boxes an `impl Component`); realized
/// children are pushed straight onto `base_mut().children` (the `Vec<Box<dyn Component>>`).
/// **Entries into a menu — the one conversion, for both authoring paths**.
///
/// A menu entry is data carrying an [`Intent`]; this is what turns a list of them into the widget.
/// It lives here rather than in the app because a **described** node declares a menu too, and two
/// converters over one input is the drift this project forbids — the same pair drifted once before,
/// leaving heca's own menus with quick-pick keycaps and the declared ones without.
///
/// `icon_for` is passed in because resolving an entry's glyph from its id is the *host's*
/// judgement — the app asks its action catalog, and a caller with no catalog passes `|_| None`.
/// That is the only thing the two callers do differently.
pub fn menu_from_items(
    title: &str,
    description: &str,
    name: &str,
    items: Vec<heca_view::DropdownItem>,
    icon_for: &dyn Fn(&str) -> Option<Glyph>,
    on_choose: &dyn Fn(Intent) -> Box<dyn Fn()>,
) -> Menu {
    let mut menu = Menu::new(title, description);
    if !name.is_empty() {
        menu = menu.name(name);
    }
    for it in items {
        let run = on_choose(it.intent.clone());
        let mut row = MenuItem::new()
            .label(it.label.clone())
            .danger(it.danger)
            .enabled(it.enabled)
            .on_click(run);
        if let Some(glyph) = icon_for(&it.id) {
            row = row.icon(glyph);
        }
        menu = menu.child(row);
    }
    menu
}

pub fn realize(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    let mut realized = realize_kind(node, theme, emit, forms);
    // Style belongs to every kind, so it is read once here rather than in each arm — and it is read
    // *generically*, by name against each half's own fields. There is deliberately no list of
    // property names in this file: add a field to `Layout` or `Visual` and a description can set
    // it, with no change here.
    //
    // Both halves: the theme is the default, not a wall. `Visual` used to be
    // unserializable precisely so appearance could not be reached — that is the rule this replaced.
    let base = realized.base_mut();
    let layout = base.style.layout;
    if let Some(merged) = merge_style_half(node, theme, layout) {
        base.style.layout = merged;
    }
    let visual = base.style.visual;
    if let Some(merged) = merge_style_half(node, theme, visual) {
        base.style.visual = merged;
    }
    // **The menu this node opens**, read once here for every kind — like style and like the hint
    // below it.
    //
    // Natively this is one builder on any widget, so the described form is one declaration on any
    // node: never a `ContextMenu` kind an author constructs, sizes and anchors. The framework takes
    // the anchor from whatever triggered the menu, dismisses it and gives it the keyboard, so a
    // plugin's row gets the identical menu heca's own rows get with nothing written — which is the
    // whole of ⭐⭐ RULE ZERO here. Building it from parts would be the second path, and it would
    // get the anchor, the dismissal and the keys only approximately right.
    if !node.menu.is_empty() {
        let items = node.menu.clone();
        let emit = emit.clone();
        // Entries carry an `Intent`, so a plugin's menu runs its OWN registered actions and every
        // choice goes through the one dispatch door — the interaction policy and the confirm gate
        // apply exactly as they would for a keypress.
        let menu = menu_from_items(
            "",
            "",
            "",
            items,
            // A described tree names its icons as glyphs on its own nodes; an entry's icon is the
            // host's judgement from its action catalog, which this side has no access to. Passing
            // none is honest — the app's own path supplies them.
            &|_| None,
            &|intent| {
                let emit = emit.clone();
                Box::new(move || emit(intent.clone()))
            },
        );
        // **Set on the node itself, exactly as the native builder does** — not wrapped around it.
        // A wrapper would be a second widget in the tree the description never asked for, sitting
        // between a node and its parent with its own layout, and the framework's right-click walk
        // would then find the wrapper rather than the row.
        let panel = ContextMenu::new("").child(menu);
        realized.base_mut().context_menu = Some(Box::new(move |_at| panel.clone()));
    }

    // **What a leader-key pick does to this node**, read once here for every kind, like style.
    //
    // Written into the widget's own [`Base::hint`] slot. The native authoring surface is
    // `HintExt::on_hint` — on every widget — and the declarative one is
    // this event. Two authoring models, one slot, and the framework's collector sees no difference
    // between them: that is what makes a described row and a native row equally pickable. A wrapper
    // here would be a second widget in the tree that the description never asked for, sitting
    // between a node and its parent with its own layout.
    //
    // **The intent travels with the closure**, not only inside it: a host cannot ask its policy
    // about an opaque `Fn()`, and a candidate whose action would be refused must not be offered a
    // letter. A plugin's row therefore gets the same filtering heca's own rows do,
    // with nothing extra declared — which is the whole point of one slot.
    if let Some(carrier) = hint_intent(node) {
        let emit = emit.clone();
        let run = carrier.clone();
        realized.base_mut().hint = Some(heca_grid_ui::hint::Hint::of(carrier, move || {
            emit(run.clone())
        }));
    }
    // **Who this node is**, read once here for every kind, exactly like style and the hint above.
    //
    // It cannot be a per-kind property: `key` is `IdentityExt::key` natively — on *every* widget,
    // because a collection can be built from any of them — so there is no widget whose builder
    // surface it belongs to. Reading it here is the same statement, and it means a described row
    // and a native row land in the **same** `Base::key` slot: the keyboard cursor, the right-click
    // target, the drag identity and the picker's remembered letter all read that one string and
    // cannot tell the two authoring paths apart.
    //
    // `hintable` rides along for the same reason — universal on `Base`, so universal here.
    if let Some(key) = node.declared_key() {
        realized.base_mut().key = Some(key.to_string());
    }
    if let Some(PropValue::Bool(hintable)) = node.props.get("hintable") {
        realized.base_mut().hintable = *hintable;
    }
    // `keyboard_to` too: where a press on this node sends the keyboard is on every `Base`.
    if let Some(PropValue::Text(area)) = node.props.get("keyboard_to") {
        realized.base_mut().keyboard_to = Some(area.clone());
    }
    // **What this node says on hover**, read once here for every kind, exactly like the key above.
    //
    // `TooltipExt::tooltip` is on every widget natively — it stopped being a wrapper you put
    // *around* something precisely because a wrapper put the rule in every caller's discipline and
    // made a tooltip impossible on a widget a typed container holds. So there is no widget whose
    // builder surface it belongs to, and no `Tooltip` kind for a description to name: the same
    // sentence, in the same slot, from either authoring path.
    //
    // Side and delay are part of the tip rather than styles of their own, so a node that declared
    // no tooltip says nothing by setting them — the same no-op the native builders are.
    if let Some(PropValue::Text(text)) = node.props.get("tooltip") {
        realized.base_mut().tooltip = Some(heca_grid_ui::widgets::tooltip::Tip::new(text.clone()));
        if let Some(name) = node.props.get("tooltip_side").and_then(PropValue::as_text)
            && let Some(side) = tooltip_side(name)
            && let Some(tip) = realized.base_mut().tooltip.as_mut()
        {
            tip.side = side;
        }
        if let Some(delay) = node
            .props
            .get("tooltip_delay")
            .and_then(PropValue::as_float)
            && let Some(tip) = realized.base_mut().tooltip.as_mut()
        {
            tip.delay = (delay as f32).max(0.0);
        }
        if node.props.get("tooltip_quick").and_then(PropValue::as_bool) == Some(true)
            && let Some(tip) = realized.base_mut().tooltip.as_mut()
        {
            tip.delay = heca_grid_ui::widgets::tooltip::QUICK_DELAY;
        }
    }
    // **Whether this node's ink is drawn**, read once here for every kind — CSS `visibility`.
    //
    // Its twin, `hidden` (CSS `display: none`), needs nothing here: it is a `Layout` field, so the
    // generic style merge already carries it and a described node can collapse out of the layout
    // today. This one is a signal on `Base`, which no merge reaches — so a described node could
    // keep its box and hide its ink by no means at all, and the wrapper that does it natively
    // cannot be described. With both reachable there is nothing left for a `Visibility` kind to do.
    if let Some(PropValue::Bool(visible)) = node.props.get("visible") {
        heca_grid_ui::reactive::SignalUpdate::set(&realized.base().visible, *visible);
    }
    // **Where this node's letter sits**, read once here for every kind, like the tooltip above.
    //
    // `Base::hint_style` was universal long before there was a way to set it on anything but a
    // `KeyHint` wrapper — which is why the app wraps to move a letter, and why a described tree
    // could not move one at all. Now the four knobs are on every widget natively, so this is
    // the same sentence from the other authoring path, landing in the same slot.
    //
    // Placement only *says where*: being pickable is not opt-in, and a node with nothing to act on
    // wears no letter however it styles one.
    let style = &mut realized.base_mut().hint_style;
    if let Some(name) = node
        .props
        .get("hint_placement")
        .and_then(PropValue::as_text)
        && let Some(placement) = hint_placement(name)
    {
        style.placement = placement;
    }
    if let Some(px) = node.props.get("hint_size").and_then(PropValue::as_float) {
        style.size = Some(px as f32);
    }
    if let Some(name) = node.props.get("hint_tone").and_then(PropValue::as_text)
        && let Some(tone) = hint_tone(name)
    {
        style.tone = Some(tone);
    }

    if let Some(px) = node
        .props
        .get("hint_offset_y")
        .and_then(PropValue::as_float)
    {
        style.offset_y = px;
    }

    // **Which pickers this target answers to** — read for every kind, like the placement above.
    // A described surface gets two verbs over one tree exactly as a native one does; without it a
    // plugin could own a picker (`opens_on`) but not say what it letters, so it would letter
    // everything beneath it.
    if let Some(PropValue::List(list)) = node.props.get("hint_scope") {
        let scopes: Vec<String> = list
            .iter()
            .filter_map(PropValue::as_text)
            .map(str::to_string)
            .collect();
        if !scopes.is_empty() {
            realized.base_mut().hint_scopes = scopes;
        }
    }
    // A token name, resolved against the live theme here on the host side — the library has no
    // notion of a token, exactly as `prop_to_input` already handles every other colour.
    if let Some(PropValue::Color(token)) = node.props.get("hint_color")
        && let Some(hex) = resolve_color(token, theme)
        && let Ok(color) = hex.parse()
    {
        realized.base_mut().hint_style.color = Some(color);
    }
    // **The verbs this node answers to**, read once here for every kind, exactly like the hint.
    //
    // `EventExt::on_action` is on every widget, so this is universal too — a per-kind arm would
    // be the same framework rule written thirty-three times. It is what gives a described
    // **surface** a verb of its own: `[[keys.surface]] pick = "s"` names `mypanel.pick`, the host
    // walks the visible trees for whoever declares that name (`fire_widget_action`), and this is
    // where a described tree gets to be that whoever.
    //
    // An **event** is fired at a node by what the user did to it; an **action** is a name said out
    // loud. Different slots for that reason, and the same one door on the far side: `Base::actions`
    // holds a native closure and this one alike.
    for (name, carrier) in &node.actions {
        // **A verb that names itself never runs.** The intent goes back through the router, which
        // looks for a widget declaring that name — this one — and posts it again: a description
        // spelling `"mypanel.pick": {"action": "mypanel.pick"}` would spin the event loop forever.
        // Refusing it here is the only place that knows both names.
        if carrier.action == *name {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] realize: action {name:?} fires an intent of its own name — ignored (it                  would resolve back to this widget and re-post itself forever)",
            );
            continue;
        }
        let emit = emit.clone();
        let run = carrier.clone();
        realized
            .base_mut()
            .actions
            .push(heca_grid_ui::DeclaredAction {
                name: name.clone(),
                run: Box::new(move || emit(run.clone())),
            });
    }
    realized
}

mod intents;
mod kinds;
mod props;
mod style;
mod vocab;
mod widgets;

#[cfg(test)]
mod tests;
