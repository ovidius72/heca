//! `realize` — the mapper from the declarative [`ViewNode`] model to a retained grid-ui
//! [`Component`] tree (plugin-task-ui-3).
//!
//! [`ViewNode`] is the pure, serializable UI *description* (same model authored in Rust or
//! shipped by a WASM plugin), and it lives in `heca-view` with no dependencies at all. This
//! crate is the other half: it owns the grid-ui dependency and the theme wiring, so the model
//! stays free of both.
//!
//! It sits **below the app** on purpose (F003/P017/T009). Everything it needs from a host
//! arrives as an argument, so anything that can build widgets can render a described tree —
//! including `heca-renderer`'s showcase, which is below `heca` and could not call this at all
//! while it lived in the app crate.
//!
//! One host service is threaded in — `emit`, an [`IntentEmitter`]: a realized actionable widget
//! fires the node's `Intent` on **click**. It is a **narrow seam** over the model's own [`Intent`],
//! not an app type, so this mapper can live and be called below `heca` (F003/P017/T009). What an
//! intent means is the host's business; the app wraps it as `InteractionIntent::View`.
//!
//! The universal picker (`prefix+/`) needs no seam at all: a node's `hint` event (defaulting to its
//! `press`) is written straight into the widget's own `Base::hint` slot, and the framework collects
//! the declarations out of the laid-out tree ([`collect_hints`](heca_grid_ui::collect_hints)). There
//! used to be a `HintTargets` registry sink here, mirroring a host-side registry that mapped an
//! opaque id back to a `pub(crate)` app enum — two doors onto one feature, and the plugin-facing
//! one was the second-class half.
//!
//! Adding a widget = one [`WidgetKind`] arm here (+ its variant in the model).
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
//! walks [`WidgetKind::ALL`] and fails if a kind produces neither children
//! nor paint — so a new kind added without an arm cannot silently render an empty container.
//!

use std::rc::Rc;

use heca_grid_ui::reactive::{Signal, SignalGet};
use heca_grid_ui::widgets::{CardGrid, ContextMenu, GridCell, Menu, MenuItem};
use heca_grid_ui::{Action, Alert, Badge, BadgeButton, Button, ButtonVariant, Card, Checkbox, Choice, Component, DockFrame, Flex, Gauge, Glyph, Grid, Icon, IconButton, Input, Item, ItemGroup, KeyHintGroup, Label, LayoutExt, MarkerGroup, Overlay, Panel, PropInput, RailCell, Row as GridRow, NfGlyph, NfIcon, ProgressBar, ScrollRegion, Select, Separator, SetProp, SignalData, Spinner, StatusDot, Surface, Tabs, Tag, Theme, Toast, ToastPosition, ToastSeverity, Toggle, Track, WidgetSize};

use heca_view::{
    Intent, PropMap, PropValue, ViewNode, ViewSize, ViewVariant, WidgetKind,
};

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
/// [`kind`](WidgetKind). NB: `Box<dyn Component>` is **not** itself `Component`, so a
/// container can't take it via `Parent::child` (which boxes an `impl Component`); realized
/// children are pushed straight onto `base_mut().children` (the `Vec<Box<dyn Component>>`).
/// **Entries into a menu — the one conversion, for both authoring paths** (F003/P097/T501).
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
    // Both halves, since F003/P017/T7: the theme is the default, not a wall. `Visual` used to be
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
    // below it (F003/P097/T501).
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
        realized.base_mut().context_menu = Some(Box::new(move || panel.clone()));
    }

    // **What a leader-key pick does to this node**, read once here for every kind, like style.
    //
    // Written into the widget's own [`Base::hint`] slot. The native authoring surface is
    // `ComponentExt::on_hint` — on every widget since F003/P082/T432 — and the declarative one is
    // this event. Two authoring models, one slot, and the framework's collector sees no difference
    // between them: that is what makes a described row and a native row equally pickable. A wrapper
    // here would be a second widget in the tree that the description never asked for, sitting
    // between a node and its parent with its own layout.
    //
    // **The intent travels with the closure**, not only inside it: a host cannot ask its policy
    // about an opaque `Fn()`, and a candidate whose action would be refused must not be offered a
    // letter (F003/P082/T432). A plugin's row therefore gets the same filtering heca's own rows do,
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
    // It cannot be a per-kind property: `key` is `ComponentExt::key` natively — on *every* widget,
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
    // **What this node says on hover**, read once here for every kind, exactly like the key above.
    //
    // `ComponentExt::tooltip` is on every widget natively — it stopped being a wrapper you put
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
    // could not move one at all. Since T501 the four knobs are on every widget natively, so this is
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
    if let Some(px) = node
        .props
        .get("hint_offset_y")
        .and_then(PropValue::as_float)
    {
        style.offset_y = px;
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
    // `ComponentExt::on_action` is on every widget, so this is universal too — a per-kind arm would
    // be the same framework rule written thirty-three times. It is what gives a described
    // **surface** a verb of its own: `[[keys.surface]] pick = "s"` names `mypanel.pick`, the host
    // walks the visible trees for whoever declares that name (`fire_widget_action`), and this is
    // where a described tree gets to be that whoever (F003/P082/T436).
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
        realized.base_mut().actions.push(heca_grid_ui::DeclaredAction {
            name: name.clone(),
            run: Box::new(move || emit(run.clone())),
        });
    }
    realized
}

/// Overlay a node's properties onto one half of an **already-constructed** widget's style —
/// [`Layout`] or [`Visual`]. `None` when the node changes nothing about this half.
///
/// Generic over the half because the two are merged identically: read the live value as a JSON
/// object, overlay only the keys the node actually carries, read it back. It was written for
/// `Layout` alone; pointing it at `Visual` too was all F003/P017/T7 needed on this side, which is
/// why the split between the halves was worth keeping rather than undoing.
///
/// On top, not from scratch: widgets set deliberate non-default style in their constructors
/// (`ScrollRegion::new()` zeroes its min sizes and opts into shrinking so a viewport can be smaller
/// than its content). Rebuilding from `Default` would silently break those.
///
/// Total for untrusted input, as the rest of `realize` is: an unknown key is skipped, and a value
/// that does not fit its field is dropped *individually* — one bad property never discards the
/// good ones and never panics.
fn merge_style_half<T>(node: &ViewNode, theme: &Theme, current: T) -> Option<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    if node.props.is_empty() {
        return None;
    }
    let Ok(serde_json::Value::Object(current)) = serde_json::to_value(current) else {
        return None;
    };
    // Only keys that name a real field of this half; `current` IS that field list, derived not
    // written. A key that names neither half's field is simply not a style property.
    let incoming: Vec<(&String, serde_json::Value)> = node
        .props
        .iter()
        .filter(|(key, _)| current.contains_key(key.as_str()))
        .filter_map(|(key, value)| prop_to_json(value, theme).map(|v| (key, v)))
        .collect();
    if incoming.is_empty() {
        return None;
    }

    let mut merged = current.clone();
    for (key, value) in &incoming {
        merged.insert((*key).clone(), value.clone());
    }
    // Fast path: everything fits. Otherwise fall back to applying one key at a time so a single
    // bad value costs only itself.
    if let Ok(whole) = serde_json::from_value::<T>(serde_json::Value::Object(merged)) {
        return Some(whole);
    }
    let mut acc = current;
    for (key, value) in incoming {
        let mut candidate = acc.clone();
        candidate.insert(key.clone(), value);
        if serde_json::from_value::<T>(serde_json::Value::Object(candidate.clone())).is_ok() {
            acc = candidate;
        }
    }
    serde_json::from_value::<T>(serde_json::Value::Object(acc)).ok()
}

/// A [`PropValue`] as the JSON scalar its field expects — the enums travel as their
/// **names** (`"center"`, `"space_between"`, `"small"`), matching how glyphs and colours already
/// cross the boundary. Colours and glyphs never name a `Layout` field, so they are simply strings
/// here and get filtered out by the field-name check.
fn prop_to_json(value: &PropValue, theme: &Theme) -> Option<serde_json::Value> {
    Some(match value {
        PropValue::Bool(b) => serde_json::Value::Bool(*b),
        PropValue::Int(i) => serde_json::Value::from(*i),
        PropValue::Float(f) => serde_json::Number::from_f64(*f).map(Into::into)?,
        // A colour is a hex literal or a THEME TOKEN NAME. A token is resolved here, against the
        // theme this tree is being built with — the same moment every native widget bakes its
        // colours in. A theme reload drops the retained trees and rebuilds them (`reload_config`
        // clears `chrome_tree` and every pane header), so a token-named override follows the new
        // theme for free, without the colour having to stay unresolved all the way to paint.
        //
        // A hex literal is exactly what it says and does not track the theme. That is the trade a
        // caller makes by writing one, and it is the reason the docs steer toward token names.
        PropValue::Color(c) => serde_json::Value::String(resolve_color(c, theme)?),
        PropValue::Text(t) | PropValue::Glyph(t) => serde_json::Value::String(t.clone()),
        PropValue::Size(s) => serde_json::to_value(s).ok()?,
        PropValue::Variant(v) => serde_json::to_value(v).ok()?,
        PropValue::Align(a) => serde_json::to_value(a).ok()?,
        PropValue::List(items) => serde_json::Value::Array(
            items.iter().filter_map(|i| prop_to_json(i, theme)).collect(),
        ),
        // A struct-shaped property (`border`, `glow`) — handed to the field's own deserializer as
        // an object. Recursive, so a colour nested inside is a theme token like any other and is
        // resolved here against the same theme: without that a nested `"accent"` would reach serde
        // as the literal word and the whole field would be dropped. A member that cannot be
        // converted is skipped, leaving the rest — the same totality rule as everywhere else, and
        // it means a half-written object degrades to its usable fields rather than vanishing.
        PropValue::Map(fields) => serde_json::Value::Object(
            fields
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), prop_to_json(v, theme)?)))
                .collect(),
        ),
    })
}


/// A colour property as the hex string `Color`'s own deserializer accepts.
///
/// `#rrggbb` / `#rrggbbaa` / `#rgb` pass straight through — `Color::from_str` owns those spellings
/// and this does not second-guess it. Anything else is read as a **theme token name** and looked up
/// among the theme's own colour fields.
///
/// The lookup is derived, not written: [`heca_theme::Theme`] is `Serialize` and its colours
/// serialize as hex strings, so the accepted vocabulary *is* the theme's field list. Add a colour
/// token to the theme and a description can name it the same day, with no table here to update —
/// the same arrangement `merge_style_half` uses for the style fields themselves.
///
/// `None` for a name the theme does not have, which drops that one property and leaves its
/// neighbours alone (`merge_style_half` is total for untrusted input).
fn resolve_color(spec: &str, theme: &Theme) -> Option<String> {
    if spec.starts_with('#') {
        return Some(spec.to_string());
    }
    let serde_json::Value::Object(tokens) = serde_json::to_value(&theme.colors).ok()? else {
        return None;
    };
    match tokens.get(spec) {
        // A colour token. Non-colour fields (`name`, the numeric geometry) either are not strings
        // or do not parse as a colour, so they cannot be named by accident.
        Some(serde_json::Value::String(hex)) if hex.starts_with('#') => Some(hex.clone()),
        _ => None,
    }
}

/// Feed a node's properties to a widget through its **generated** surface.
///
/// This is the whole point of the arrangement: the app names no property here. Which keys a widget
/// accepts is decided by that widget's own `#[prop]` builders, so a capability added in the library
/// is reachable from a description the same day, and one that is forgotten fails the drift guard
/// rather than going quietly missing.
///
/// Call it **after** children are attached — properties are order-independent on that condition,
/// which is what lets a builder that clamps against its children (`Select::selected`) see them.
fn with_props<W: SetProp>(widget: W, node: &ViewNode, theme: &Theme) -> W {
    widget.apply_props(|key| node.props.get(key).and_then(|v| prop_to_input(v, theme)))
}

/// A [`PropValue`] as the library's neutral scalar. The library never sees the app's model; this
/// is the one conversion at the boundary.
///
/// Enums cross as their **names**, which is how glyphs and colours already travel, so the widget's
/// own variants are the accepted vocabulary and there is no table of strings on either side.
fn prop_to_input(value: &PropValue, theme: &Theme) -> Option<PropInput> {
    Some(match value {
        PropValue::Bool(b) => PropInput::Bool(*b),
        PropValue::Int(i) => PropInput::Number(*i as f64),
        PropValue::Float(f) => PropInput::Number(*f),
        // A colour token is resolved to hex HERE, on the host side, before it reaches the widget:
        // the library has no notion of a theme token, and `Color::from_str` only knows hex.
        PropValue::Color(c) => PropInput::Text(resolve_color(c, theme)?),
        PropValue::Text(t) | PropValue::Glyph(t) => PropInput::Text(t.clone()),
        PropValue::Size(s) => PropInput::Text(prop_enum_name(s)?),
        PropValue::Variant(v) => PropInput::Text(prop_enum_name(v)?),
        PropValue::Align(a) => PropInput::Text(prop_enum_name(a)?),
        // Neither shape fits a widget builder: `PropInput` is a scalar channel (Bool | Number |
        // Text) because no `#[prop]` builder takes a list or a struct. A `Map` is for the
        // struct-shaped fields of `Layout`/`Visual`, which reach the widget through
        // `merge_style_half` and serde instead — see `prop_to_json`. If a widget builder ever does
        // take one, `PropInput` grows a variant then, deliberately, and not before.
        PropValue::List(_) | PropValue::Map(_) => return None,
    })
}

/// The snake_case name serde already gives these enums — reused rather than re-spelled, so the two
/// paths (the layout merge and the widget surface) cannot disagree about what `"space_between"` is.
fn prop_enum_name<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}

/// A hint placement by the name serde gives [`ViewHintPlacement`] — `"top_center"`, `"center"`,
/// `"center_right"`, `"top_right"`, `"top_left"`. An unknown name keeps the default.
fn hint_placement(name: &str) -> Option<heca_grid_ui::widgets::HintPlacement> {
    use heca_grid_ui::widgets::HintPlacement as P;
    Some(match name {
        "top_center" => P::TopCenter,
        "center" => P::Center,
        "center_right" => P::CenterRight,
        "top_right" => P::TopRight,
        "top_left" => P::TopLeft,
        _ => return None,
    })
}

/// A tooltip side by the name serde gives [`ViewTooltipSide`] — `"top"`, `"bottom"`, `"left"`,
/// `"right"`. An unknown name keeps the default, as every other untrusted value here does.
fn tooltip_side(name: &str) -> Option<heca_grid_ui::TooltipSide> {
    Some(match name {
        "top" => heca_grid_ui::TooltipSide::Top,
        "bottom" => heca_grid_ui::TooltipSide::Bottom,
        "left" => heca_grid_ui::TooltipSide::Left,
        "right" => heca_grid_ui::TooltipSide::Right,
        _ => return None,
    })
}

/// The per-kind mapping — see [`realize`], which wraps it with the props every node can carry.
fn realize_kind(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    match node.kind {
        // ── Containers (attach realized children) ──
        WidgetKind::VStack => realize_flex(node, theme, Flex::column(), emit, forms),
        WidgetKind::HStack => realize_flex(node, theme, Flex::row(), emit, forms),
        // The interactive row — a container that is also a control. `active` / `nav_selected` /
        // `marker` arrive through the generated surface; the press is wired to BOTH a click and a
        // hint target, like `Button`, so `prefix+/` reaches it. Without `on_activate` the widget
        // stays non-focusable and paints no hover, which is the right answer for a row with no
        // press intent — a described row that nothing can activate should not pretend otherwise.
        WidgetKind::Row => {
            let mut row = with_props(GridRow::new(), node, theme);
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                row = row.on_activate(move || emit(carrier.clone()));
            }
            attach_children(Box::new(row), node, theme, emit, forms)
        }
        WidgetKind::Card => {
            attach_children(Box::new(Card::new(text_of(node))), node, theme, emit, forms)
        }
        WidgetKind::Surface => {
            attach_children(Box::new(Surface::new()), node, theme, emit, forms)
        }
        // `Panel` used to be an alias for `Surface`, which is why the published examples showed
        // `Panel::new().title(..)` against a widget that had no title (F003/P017/T008). It is its
        // own widget now; `text` is the heading, as it is for `Card` and `DockFrame`.
        WidgetKind::Panel => {
            let panel = with_props(Panel::new().title(text_of(node)), node, theme);
            attach_children(Box::new(panel), node, theme, emit, forms)
        }
        // **A described surface** — the same widget the exposé is, raised from data.
        //
        // Its children are its panel: one child is the panel itself, several are stacked into one,
        // because an `Overlay` centres and decorates exactly one. `animation_named` and `blocking`
        // arrive through the generated property surface, so how a described surface comes and goes
        // is the widget's own builder and not a list repeated here — `"animation": "zoom_fade"`
        // names the same gesture native code names.
        WidgetKind::Overlay => {
            let panel: Box<dyn Component> = match node.children.len() {
                1 => realize(&node.children[0], theme, emit, forms),
                _ => attach_children(Box::new(Flex::column()), node, theme, emit, forms),
            };
            let mut overlay = with_props(Overlay::new().panel_boxed(panel), node, theme);
            // **What the dismiss key means, declared like any other behaviour.** The overlay has
            // always held the keys; since F003/P097/T502 it answers this one, and this is the
            // described spelling — so a plugin's surface closes on Escape by saying what closing
            // means, and never by writing a key.
            if let Some(intent) = node.events.get("dismiss") {
                let (intent, emit) = (intent.clone(), emit.clone());
                overlay = overlay.on_dismiss(move || emit(intent.clone()));
            }
            Box::new(overlay)
        }
        // **A described picker** — the one thing a description could not have.
        //
        // Its children are what it letters, and it is a transparent wrapper, so several children
        // are a column rather than a stack: unwrapped, that is what they already were. `opens_on`
        // arrives through the generated property surface, so the verb is the widget's own builder
        // and not a name repeated here.
        WidgetKind::KeyHintGroup => {
            let subtree: Box<dyn Component> = match node.children.len() {
                1 => realize(&node.children[0], theme, emit, forms),
                _ => attach_children(Box::new(Flex::column()), node, theme, emit, forms),
            };
            Box::new(with_props(KeyHintGroup::new_boxed(subtree), node, theme))
        }
        WidgetKind::Scroll => {
            // `axes` reaches the widget through its own builder, so a declarative region can be
            // horizontal or two-axis — it was vertical-only for as long as this arm named its
            // properties by hand.
            let region = with_props(ScrollRegion::new(), node, theme);
            attach_children(Box::new(region), node, theme, emit, forms)
        }

        // ── Leaves ──
        WidgetKind::Label => {
            // bold / italic / underline / strikethrough / align / font_size all arrive through
            // the generated surface — `Label`'s builders decide which, not a list here.
            Box::new(with_props(Label::new(text_of(node)), node, theme))
        }
        WidgetKind::Button => realize_button(node, theme, emit, forms),
        WidgetKind::Badge => Box::new(Badge::new(text_of(node))),
        WidgetKind::Tag => Box::new(Tag::new(text_of(node))),
        WidgetKind::Alert => Box::new(Alert::new(text_of(node))),
        WidgetKind::StatusDot => Box::new(StatusDot::online()),
        // `orientation` and `length` both arrive through the generated surface, and the widget
        // recomputes both axes from the pair, so neither has to come first.
        WidgetKind::Separator => Box::new(with_props(Separator::horizontal(), node, theme)),
        WidgetKind::Gauge => {
            Box::new(with_props(Gauge::new(), node, theme))
        }
        // **Nobody knows how long** — no properties of its own: it animates off the frame clock,
        // and its diameter is `width`/`height`, which the generic style merge already carries.
        WidgetKind::Spinner => Box::new(Spinner::new()),
        // **How far along** — `value` arrives through the generated property surface, clamped by
        // the widget itself, so out-of-range input from a plugin is a full or empty bar and never a
        // panic. The easing is the widget's: a tree re-sent with a new value animates.
        WidgetKind::Progress => Box::new(with_props(ProgressBar::new(), node, theme)),
        // **A keyboard glyph, from its own font.** The name is parsed by the library's own
        // `from_prop_name` rather than a table repeated here — the enum derives that parser from
        // the same variants the mirror guard compares, so there is one vocabulary and no second
        // list of strings to fall behind. `size` and `color` ride the generated surface.
        //
        // An unknown or missing name realizes to nothing rather than a wrong key: a description is
        // untrusted input, and a ⌘ where an author asked for ⇧ is worse than a gap.
        // **A row of actions that gets out of its own way.** Its children are buttons, and they
        // are built by the one button path — `button_of` — so a grouped action is the same button
        // a standalone one is, press intent and composed content included. Anything that is not a
        // button is skipped rather than wrapped: the group reads a label, a glyph and a click out
        // of each child to build its overflow menu, and there is nothing to read in a `Label`.
        WidgetKind::ButtonGroup => {
            let mut group = with_props(heca_grid_ui::widgets::ButtonGroup::new(), node, theme);
            for child in &node.children {
                if child.kind == WidgetKind::Button {
                    group = group.child(button_of(child, theme, emit, forms));
                }
            }
            Box::new(group)
        }
        WidgetKind::NfIcon => match node
            .props
            .get("glyph")
            .and_then(PropValue::as_text)
            .and_then(<NfGlyph as heca_grid_ui::PropName>::from_prop_name)
        {
            Some(glyph) => Box::new(with_props(NfIcon::new(glyph), node, theme)),
            None => Box::new(Flex::empty()),
        },
        WidgetKind::Icon => match glyph_prop(node) {
            Some(glyph) => Box::new(Icon::new(glyph)),
            None => Box::new(Flex::empty()),
        },
        WidgetKind::Input => {
            let mut input = with_props(Input::new().value(text_of(node)), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = input.text();
                forms.bind(name.clone(), Box::new(move || PropValue::Text(sig.get_untracked())));
                // Also expose the live signal for reactive validation (disabled submit button).
                forms.bind_text_signal(name, sig);
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                input = input.on_change(move |_a: Action| emit(carrier.clone()));
            }
            Box::new(input)
        }
        WidgetKind::Toggle => {
            let mut t = with_props(Toggle::new(), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = t.state();
                forms.bind(name, Box::new(move || PropValue::Bool(sig.get_untracked())));
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                t = t.on_change(move |_a: Action| emit(carrier.clone()));
            }
            Box::new(t)
        }
        WidgetKind::Checkbox => {
            // `checked` arrives through the surface; the widget's own default is already false.
            let mut c = with_props(Checkbox::new().label(text_of(node)), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = c.state();
                forms.bind(name, Box::new(move || PropValue::Bool(sig.get_untracked())));
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                c = c.on_change(move |_a: Action| emit(carrier.clone()));
            }
            Box::new(c)
        }
        WidgetKind::BadgeButton => {
            let mut b = BadgeButton::new(text_of(node));
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                b = b.on_click(move || emit(carrier.clone()));
            }
            Box::new(b)
        }
        WidgetKind::IconButton => {
            let icon = Icon::new(glyph_prop(node).unwrap_or(Glyph::Circle));
            let mut b = IconButton::new(icon);
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                b = b.on_click(move || emit(carrier.clone()));
            }
            Box::new(b)
        }
        WidgetKind::Item => {
            let mut it = Item::new(text_of(node));
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                it = it.on_activate(move || emit(carrier.clone()));
            }
            // Slots: the row's leading / trailing affordances. It has **no default slot** — its
            // middle is the label, which comes from `text` — so a child with neither slot name is
            // ignored rather than silently dropped somewhere it doesn't belong.
            for child in &node.children {
                let realized = realize(child, theme, emit, forms);
                match slot_of(child) {
                    Some("leading") => it = it.leading_boxed(realized),
                    Some("trailing") => it = it.trailing_boxed(realized),
                    other => warn_unknown_slot(node, child, other, &["leading", "trailing"]),
                }
            }
            Box::new(it)
        }
        WidgetKind::RailCell => {
            let icon = Icon::new(glyph_prop(node).unwrap_or(Glyph::Circle));
            let mut cell = RailCell::new(icon);
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                cell = cell.on_activate(move || emit(carrier.clone()));
            }
            Box::new(cell)
        }

        // ── Options ──
        // An option is a node with a **value** and arbitrary content — and options are **children**,
        // not a `props["options"]` list of strings. That is what lets a declarative option compose
        // an icon + a label exactly like a native one, and what carries the chosen *value* back to
        // the author (see `realize_options`).
        WidgetKind::Choice => {
            let mut choice = realize_choice(node, theme, emit, forms);
            // A standalone `Choice` (outside a Select/Tabs) is activatable on its own. Inside a
            // container the container owns the click, so a `press` there is ignored, not half-wired.
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                choice = choice.on_activate(move || emit(carrier.clone()));
            }
            Box::new(choice)
        }
        WidgetKind::Select => {
            let mut select = Select::empty();
            for option in realize_options(node, theme, emit, forms) {
                select = select.option(option);
            }
            // After the options, so `selected` clamps against the real count. That ordering is a
            // property of this arm's construction, not something a property author must know.
            select = with_props(select, node, theme);
            if let Some(on_change) = option_change(node, emit) {
                select = select.on_change(on_change);
            }
            // A named `Select` is a form field like `Input`/`Toggle`/`Checkbox`: its chosen
            // option's **value** (not the index) is marshalled into the submitted `data`. The live
            // selection is an index signal, so bind a reader that maps it through the option values.
            if let Some(name) = name_prop(node) {
                let values = option_values(node);
                let idx = select.state();
                forms.bind(
                    name,
                    Box::new(move || {
                        PropValue::Text(values.get(idx.get_untracked()).cloned().unwrap_or_default())
                    }),
                );
            }
            Box::new(select)
        }
        WidgetKind::Tabs => {
            let mut tabs = Tabs::empty();
            for option in realize_options(node, theme, emit, forms) {
                tabs = tabs.tab(option);
            }
            tabs = with_props(tabs, node, theme);
            if let Some(on_change) = option_change(node, emit) {
                tabs = tabs.on_change(on_change);
            }
            Box::new(tabs)
        }

        // ── Groups ──
        WidgetKind::ItemGroup => {
            // `expanded` arrives through the surface; the widget already defaults to expanded.
            let mut group = with_props(ItemGroup::new(text_of(node)), node, theme);
            if let Some(on_toggle) = toggle_change(node, emit) {
                group = group.on_toggle(on_toggle);
            }
            // The group's own header is `children[0]`; the realized rows follow it.
            attach_children(Box::new(group), node, theme, emit, forms)
        }
        WidgetKind::MarkerGroup => {
            let markers = with_props(MarkerGroup::new(), node, theme);
            // An indicator: no events of its own — the rows inside carry their own intents.
            attach_children(Box::new(markers), node, theme, emit, forms)
        }

        // ── Layout ──
        WidgetKind::Grid => realize_grid(node, theme, emit, forms),
        WidgetKind::CardGrid => realize_card_grid(node, theme, emit, forms),

        WidgetKind::DockFrame => {
            // Everything DockFrame exposes arrives through the surface, at the widget's own
            // defaults when unset.
            let mut dock = with_props(DockFrame::new(text_of(node)), node, theme);
            if let Some(on_toggle) = toggle_change(node, emit) {
                dock = dock.on_toggle(on_toggle);
            }
            // Slots: `header` is the controls slot (a search field, a count badge); everything else
            // is body content — the body is the **default slot**, so an unslotted child lands there.
            for child in &node.children {
                let realized = realize(child, theme, emit, forms);
                match slot_of(child) {
                    Some("header") => dock = dock.header_boxed(realized),
                    None => dock = dock.child_boxed(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["header"]);
                        dock = dock.child_boxed(realized);
                    }
                }
            }
            // NB: `.rail(..)` is host-only — it binds a host-owned `Signal<RegionMode>`, which
            // static serializable data cannot drive (same rule as `ScrollBar`).
            Box::new(dock)
        }
        WidgetKind::Toast => {
            let mut toast = Toast::new(text_of(node)).severity(severity_prop(node));
            if let Some(glyph) = glyph_prop(node) {
                toast = toast.icon(glyph);
            }
            // `body_text` is applied first and a described `body` child overwrites the same slot
            // below, so children win without the arm having to look ahead.
            if let Some(body) = node.props.get("body_text").and_then(PropValue::as_text) {
                toast = toast.body_text(body);
            }
            if let Some(opened) = node.props.get("opened").and_then(PropValue::as_bool) {
                toast = toast.default_open(opened);
            }
            if let Some(position) = node
                .props
                .get("position")
                .and_then(PropValue::as_text)
                .and_then(toast_position)
            {
                toast = toast.position(position);
            }
            toast = with_props(toast, node, theme);

            // ── Slots ────────────────────────────────────────────────────────────────────────
            // `body` is the **default** slot, so an unslotted child is the body; `actions` are the
            // controls under it, one per child. F003/P076/T284 recorded "no slots, deliberately"
            // because the card hand-drew itself and could not hold arbitrary content — that reason
            // died with F003/P082/T481, and this reverses it (F003/P096/T488).
            //
            // A described action is an **ordinary described `Button`** carrying its own `press`
            // intent, which is what makes it a `prefix+/` target with nothing hint-related written:
            // being pickable is not opt-in.
            let mut described_actions = false;
            for child in &node.children {
                let realized = realize(child, theme, emit, forms);
                match slot_of(child) {
                    Some("actions") => {
                        described_actions = true;
                        toast = toast.action_boxed(realized);
                    }
                    Some("body") | None => toast = toast.body_boxed(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["body", "actions"]);
                        toast = toast.body_boxed(realized);
                    }
                }
            }

            // **Children win over the text sugar**, the way a `Button`'s children win over its
            // `text`/`icon`: one content model, two spellings, never two paint paths. The text
            // props stay because they are the plain-data path a `ToastSpec` uses.
            if !described_actions
                && let Some(label) = node.props.get("action_text").and_then(PropValue::as_text)
                && let Some(carrier) = intent_carrier(node, "action")
            {
                let emit = emit.clone();
                toast = toast.action(Button::new(label).on_click(move || emit(carrier.clone())));
            }
            if let Some(carrier) = intent_carrier(node, "press") {
                let emit = emit.clone();
                toast = toast.on_click(move || emit(carrier.clone()));
            }
            if let Some(carrier) = intent_carrier(node, "dismiss") {
                let emit = emit.clone();
                toast = toast.on_dismiss(move || emit(carrier.clone()));
            }
            Box::new(toast)
        }

        // `ScrollBar` is **host-only** by design: its state is live host signals
        // (`content_extent` / `viewport_extent` / `offset`), which static, serializable data
        // fundamentally cannot drive — a declarative one would render a dead control. A plugin uses
        // `Scroll` (a `ScrollRegion`), which owns its own offset. See `docs/widgets.md`.
        WidgetKind::ScrollBar => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] realize: ScrollBar is host-only — its state is a live host signal, which \
                 static data cannot drive. Use Scroll (a ScrollRegion), which owns its own offset."
            );
            Box::new(Flex::empty())
        }
    }
}

/// Push each realized child onto a boxed container's child vec. (`Box<dyn Component>` isn't
/// `Component`, so `Parent::child` can't take it; we push onto `base_mut().children` directly —
/// the same path `realize_flex` uses.)
fn attach_children(
    mut container: Box<dyn Component>,
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    for child in &node.children {
        container.base_mut().children.push(realize(child, theme, emit, forms));
    }
    container
}

/// The node's `"press"` (activation) intent, to fire on click. `None` when the node isn't
/// actionable.
fn press_intent(node: &ViewNode) -> Option<Intent> {
    Some(node.intent("press")?.clone())
}

/// **What a leader-key pick (`prefix+/`) does to this node.**
///
/// `hint` when the node declares one, otherwise `press` — so every actionable node stays reachable
/// by letter for free (the "everything is an action" rule), and a node that wants a pick to mean
/// something *else* than a click says so. That difference is the whole reason the two are separate
/// events: heca's own sidebar row activates the pane and leaves on a click, and stays in the
/// sidebar on a hint pick. Pointing one intent at both is what made `prefix+/` leave the sidebar.
fn hint_intent(node: &ViewNode) -> Option<Intent> {
    node.intent("hint").or_else(|| node.intent("press")).cloned()
}

/// The node's `"change"` intent as a carrier (value widgets — input/toggle/checkbox). Not a pick
/// target: a value change isn't a gesture a letter can stand for. Data marshalling into the intent is a later step
/// (plugin-task-ui-4 remainder); today the change simply fires the bound intent.
fn change_intent(node: &ViewNode) -> Option<Intent> {
    Some(node.intent("change")?.clone())
}

/// Realize a container node onto a base [`Flex`] (row or column), applying layout props and
/// recursively realizing + attaching children.
fn realize_flex(
    node: &ViewNode,
    theme: &Theme,
    mut flex: Flex,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    // `gap`, `align` and every other layout property are applied generically by
    // `merge_layout_props` in `realize`, for every kind — not read per-arm here.
    for child in &node.children {
        // `child()` takes an `impl Component` and boxes it; a `Box<dyn Component>` isn't
        // `Component`, so push the already-boxed child directly.
        flex.base_mut().children.push(realize(child, theme, emit, forms));
    }
    Box::new(flex)
}

/// Realize a [`Button`], wiring its `"press"` intent to both a click handler and a hint
/// target (the same intent for either input path — a click emits it, the picker fires it).
///
/// **The button's content is its children** (the widget composes, it does not draw text), so a
/// node can carry an arbitrary subtree — `Button > Column > [Row > [Icon, Label], Label]` — and it
/// is realized and mounted like any other content.
///
/// Precedence, so the two spellings never fight: **children win.** A node *with* children is
/// realized as an empty button holding them; a **childless** node falls back to the scalar sugar
/// (`text` → a bold `Label`, plus an optional leading `icon`), which is the common case and what
/// every existing caller writes. The sugar produces exactly the children the explicit form
/// would — there is one content model underneath.
fn realize_button(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    Box::new(button_of(node, theme, emit, forms))
}

/// The same button, **typed** — what a container holding `Button`s needs.
///
/// [`ButtonGroup::child`](heca_grid_ui::ButtonGroup::child) takes a `Button`, not a boxed component,
/// because it reads the button's label, glyph and click out of it to build the overflow menu — a
/// box would have erased all three. Same reason [`realize_choice`] is typed.
///
/// Split out rather than written twice: a second button-building path is exactly the drift this
/// project forbids, and the group's buttons must be the buttons a description asked for, down to
/// the press intent and the composed-children precedence.
fn button_of(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Button {
    let mut button = if node.children.is_empty() {
        // Sugar: the scalar props describe the content.
        let mut b = Button::new(text_of(node));
        if let Some(glyph) = glyph_prop(node) {
            b = b.icon(glyph);
        }
        b
    } else {
        // Composed: the children are the content.
        Button::empty()
    };
    if let Some(variant) = variant_prop(node) {
        button = button.variant(variant);
    }
    if let Some(size) = size_prop(node) {
        button = button.size(size);
    }
    if let Some(carrier) = press_intent(node) {
        let emit = emit.clone();
        button = button.on_click(move || emit(carrier.clone()));
    }
    // Attach the composed content. (`Box<dyn Component>` isn't `Component`, so it can't go through
    // `Parent::child`; push it the way every other container here does.)
    for child in &node.children {
        button
            .base_mut()
            .children
            .push(realize(child, theme, emit, forms));
    }
    button
}

/// Realize one [`Choice`] — an option: a **value** plus composed content.
///
/// Typed to `Choice` (not `Box<dyn Component>`) because that is what `Select::option` / `Tabs::tab`
/// take: those containers keep the option's state signals to drive selection in place, which a
/// boxed component would have erased.
///
/// Precedence mirrors [`Button`](realize_button): **children win**; a *childless* node falls back to
/// the `text` sugar (→ one `Label` child, exactly the child the explicit form would build). The
/// `value` prop is what the option *means*, independently of what it shows; with no `value`, the
/// text stands in for it, so `Choice { text: "HIGH" }` behaves like the native `Choice::labeled`.
fn realize_choice(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Choice {
    let value = value_prop(node)
        .map(|v| value_string(&v))
        .unwrap_or_else(|| text_of(node));
    let mut choice = if node.children.is_empty() {
        // Sugar: the scalar prop describes the content.
        Choice::labeled(value, text_of(node))
    } else {
        Choice::new(value)
    };
    for child in &node.children {
        choice
            .base_mut()
            .children
            .push(realize(child, theme, emit, forms));
    }
    choice
}

/// Realize the `Choice` children of a `Select`/`Tabs`, in order.
///
/// A child of another kind is **ignored** (with a debug log): `realize` is total for untrusted
/// input, and the options of an option-picker are options.
fn realize_options(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Vec<Choice> {
    node.children
        .iter()
        .filter(|child| {
            let is_choice = child.kind == WidgetKind::Choice;
            #[cfg(debug_assertions)]
            if !is_choice {
                eprintln!(
                    "[heca] realize: {:?} is not a valid option of a {:?} — ignored (options are Choice nodes)",
                    child.kind, node.kind
                );
            }
            is_choice
        })
        .map(|child| realize_choice(child, theme, emit, forms))
        .collect()
}

/// The change handler for an option picker (`Select`/`Tabs`) — **this is where the chosen value
/// reaches the author**.
///
/// The widgets track a selected *index* (they are indexable lists; that is their business). But an
/// index is meaningless to a plugin, and it silently breaks the moment the options are reordered. So
/// `realize` captures the options' `value` props here, and maps the index back through them when the
/// change fires: the bound intent is dispatched with `args["value"]` set to the chosen option's
/// value — `{"value": "high"}`, not an opaque `1`.
///
/// An option with no `value` falls back to `args["index"]`, so a value-less picker still reports
/// *something* rather than dispatching a bare intent.
fn option_change(node: &ViewNode, emit: &IntentEmitter) -> Option<impl Fn(Action) + 'static> {
    let intent = node.intent("change")?.clone();
    let values: Vec<Option<PropValue>> = node
        .children
        .iter()
        .filter(|child| child.kind == WidgetKind::Choice)
        .map(value_prop)
        .collect();
    let emit = emit.clone();
    Some(move |action: Action| {
        let SignalData::Usize(i) = action.data else {
            return;
        };
        let mut intent = intent.clone();
        match values.get(i).cloned().flatten() {
            Some(value) => intent.args.insert("value".into(), value),
            None => intent.args.insert("index".into(), PropValue::Int(i as i64)),
        };
        emit(intent);
    })
}

/// Realize a [`Grid`] — the one kind whose configuration is genuinely **list-shaped**: its track
/// templates.
///
/// Tracks are **CSS-like strings** (`"1fr"`, `"22px"`, `"auto"`), because CSS grid already has this
/// vocabulary and plugin authors know it — no new schema is invented. Areas are one string per grid
/// row, exactly as `grid-template-areas` writes them.
///
/// **Placement is a prop on the child**, not a structure in the parent: a child carries `area` (a
/// name from the template) *or* `col`/`row` (+ optional `col_span`/`row_span`); a child with neither
/// gets taffy's auto-placement. This keeps `ViewNode`'s shape flat — no second child vector, no
/// placement table to keep in sync with the children.
/// **A grid of cards with a cursor**, described (F003/P097/T501).
///
/// Each child is one card, and its own `key` is what activation hands back — the same key the
/// caller reads in `on_activate`, so a plugin's grid answers in its own vocabulary.
///
/// ⚠️ **The realizer makes the connection a description cannot.** Natively a caller hands the grid
/// each card's own state signal (`GridCell::new(key, card.nav_state())`), which a description has
/// no way to express: it cannot name another node's signal. This side *builds both halves*, so it
/// takes each card's signal itself and wires the cell. That is the whole reason a `CardGrid` can be
/// described while a `ScrollBar` cannot — there, the signal genuinely comes from outside.
///
/// Every card is wrapped in a `Row`, which is what holds the cursor state and the hover a grid
/// moves its cursor with. A card that is already a `Row` keeps its own behaviour: the wrapper is
/// transparent to layout and passes focus and drag straight through, the same trade the exposé's
/// own cards make.
fn realize_card_grid(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    use heca_grid_ui::builders::Parent as _;
    let mut grid = with_props(CardGrid::new(), node, theme);
    let mut columns: Vec<Vec<GridCell>> = Vec::new();
    let mut layout = Flex::row();
    let mut derived: Vec<String> = Vec::new();
    for child in &node.children {
        let body = realize(child, theme, emit, forms);
        // **A card never has to be named** — `key` is optional everywhere in this library, so it is
        // optional here. Gating on it is the mistake `.draggable()` made: silently dead on every
        // widget nobody had reason to name, and an internal rule an author had to learn before
        // anything worked (AGENTS.md § 0a).
        //
        // The fallback is not a rule this file invents. `Component::text_summary` IS the library's
        // accessible-name algorithm — the same one [`nav::identity_of`] reads at its second level —
        // so an unnamed `Icon` + `Label("nginx")` card is `nginx` with nothing wired, and the name
        // a plugin gets back is the name the card reads by on screen. Reading the node's own
        // `"text"` prop instead names a bare `Label` and leaves every real card anonymous: a card
        // is a subtree, and its text is nested inside it.
        //
        // Repeats carry the index `identity_of` uses, spelled the same way (`nginx`, `nginx[1]`),
        // so two identically-worded cards stay two cards rather than collapsing into one.
        let key = match child.declared_key() {
            Some(k) => k.to_owned(),
            None => {
                let name = body.text_summary().unwrap_or_default();
                let n = derived.iter().filter(|d| **d == name).count();
                derived.push(name.clone());
                if n == 0 { name } else { format!("{name}[{n}]") }
            }
        };
        let card = GridRow::new().child_boxed(body);
        // **One column per card, because the layout draws them side by side.** `CardGrid::row`
        // takes columns and states the contract itself: the cards must be in the same left-to-right
        // order the layout draws them. Putting them all in ONE column instead left arrow-right
        // doing nothing while arrow-down walked a visual row — the cursor and the picture
        // disagreeing, which is the one thing that doc warns about and which nothing fails on.
        columns.push(vec![
            GridCell::new(key, card.nav_state()).hovered(card.hovered()),
        ]);
        layout = layout.child(card);
    }
    grid = grid.row(columns, layout);
    // Behaviour is an Intent, as everywhere: the chosen card's key travels as an argument, so one
    // described action serves every card rather than a binding per card.
    if let Some(intent) = node.events.get("activate") {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid.on_activate(move |key| {
            emit(intent.clone().arg("key", PropValue::Text(key.to_string())))
        });
    }
    if let Some(intent) = node.events.get("move") {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid.on_move(move |key| {
            emit(intent.clone().arg("key", PropValue::Text(key.to_string())))
        });
    }
    if let Some(intent) = node.events.get("dismiss") {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid.on_dismiss(move || emit(intent.clone()));
    }
    Box::new(grid)
}

fn realize_grid(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    let mut grid = Grid::new();
    if let Some(columns) = track_list(node, "columns") {
        grid = grid.columns(columns);
    }
    if let Some(rows) = track_list(node, "rows") {
        grid = grid.rows(rows);
    }
    // Areas must be defined before a child can be placed into one by name.
    if let Some(areas) = string_list(node, "areas") {
        grid = grid.areas(areas.iter().map(String::as_str));
    }
    // How the items sit inside their cells — `align` vertically, `justify_items` horizontally —
    // arrives through the generic layout merge in `realize`, like every other layout property.
    // Both default to `Stretch`, which pins an explicitly-sized item to the top-left of its cell,
    // so a row of mixed-height content needs `align: center` to share a centre line.
    for child in &node.children {
        let realized = realize(child, theme, emit, forms);
        match child.props.get("area").and_then(PropValue::as_text) {
            Some(area) => grid = grid.area_boxed(realized, area),
            None => match (usize_prop(child, "col"), usize_prop(child, "row")) {
                (Some(col), Some(row)) => {
                    grid = grid.cell_boxed(
                        realized,
                        col as u16,
                        row as u16,
                        usize_prop(child, "col_span").unwrap_or(1) as u16,
                        usize_prop(child, "row_span").unwrap_or(1) as u16,
                    )
                }
                // Neither an area nor a cell: let the grid auto-place it.
                _ => grid.base_mut().children.push(realized),
            },
        }
    }
    Box::new(grid)
}

/// A [`PropValue::List`] of track strings, parsed to [`Track`]s (`"columns"` / `"rows"`).
fn track_list(node: &ViewNode, key: &str) -> Option<Vec<Track>> {
    let PropValue::List(items) = node.props.get(key)? else {
        return None;
    };
    Some(items.iter().map(parse_track).collect())
}

/// A [`PropValue::List`] of strings (`"areas"`). Non-text items are skipped.
fn string_list(node: &ViewNode, key: &str) -> Option<Vec<String>> {
    let PropValue::List(items) = node.props.get(key)? else {
        return None;
    };
    Some(
        items
            .iter()
            .filter_map(PropValue::as_text)
            .map(str::to_string)
            .collect(),
    )
}

/// Parse one CSS-like grid track: `"22px"` (or a bare `Int`/`Float` — pixels) · `"1fr"` · `"auto"` ·
/// `"min"`/`"min-content"` · `"max"`/`"max-content"`.
///
/// **Anything unrecognised degrades to [`Track::Auto`]** — never a panic, never an error. The model
/// is untrusted input (a plugin, an RPC caller), so a typo costs that author a differently-sized
/// track, not a broken host.
fn parse_track(value: &PropValue) -> Track {
    let text = match value {
        PropValue::Text(s) => s.trim().to_ascii_lowercase(),
        // A bare number is pixels, the same forgiving reading the size parser gives config.
        PropValue::Int(i) => return Track::Px(*i as f32),
        PropValue::Float(f) => return Track::Px(*f as f32),
        _ => return Track::Auto,
    };
    match text.as_str() {
        "auto" => Track::Auto,
        "min" | "min-content" => Track::MinContent,
        "max" | "max-content" => Track::MaxContent,
        _ => {
            if let Some(px) = text.strip_suffix("px").and_then(|n| n.trim().parse().ok()) {
                Track::Px(px)
            } else if let Some(fr) = text.strip_suffix("fr").and_then(|n| n.trim().parse().ok()) {
                Track::Fr(fr)
            } else if let Ok(px) = text.parse() {
                Track::Px(px)
            } else {
                Track::Auto
            }
        }
    }
}

/// The child's **`"slot"`** prop — which named place in its parent it belongs to (`"header"`,
/// `"leading"`, `"trailing"`).
///
/// This is how a widget with **several places to put children** stays expressible without changing
/// `ViewNode`'s shape: `children` remains one flat `Vec`, and the *child* says where it goes. No
/// `slots: Map<..>` on the node, no second child vector, and the JSON stays flat. It generalizes to
/// every future slotted widget for free.
fn slot_of(node: &ViewNode) -> Option<&str> {
    node.props.get("slot").and_then(PropValue::as_text)
}

/// A child named a slot its parent doesn't have (or none, where the parent has no default). Debug-log
/// it and move on: `realize` is total for untrusted input, so a plugin's typo costs it a misplaced
/// child, never a panic.
fn warn_unknown_slot(parent: &ViewNode, child: &ViewNode, slot: Option<&str>, known: &[&str]) {
    #[cfg(debug_assertions)]
    eprintln!(
        "[heca] realize: {:?} child of a {:?} has slot {:?} — known slots: {:?}",
        child.kind, parent.kind, slot, known,
    );
    #[cfg(not(debug_assertions))]
    let _ = (parent, child, slot, known);
}

/// A node's intent for `event`, wrapped as the carrier a widget callback fires. (`press_intent` also
/// registers a hint target; this is for events that aren't pick targets — `change`, `dismiss`, a
/// toast's inline `action`.)
fn intent_carrier(node: &ViewNode, event: &str) -> Option<Intent> {
    node.intent(event).cloned()
}

/// A `Toast`'s `"severity"` prop, by name. Unknown / absent → `Info` (the widget's own default).
/// A described position name → [`ToastPosition`]. An unknown name is **not** an error: like every
/// other named value a description carries, it degrades to the widget's default rather than
/// failing a plugin's tree (F003/P096/T483).
fn toast_position(name: &str) -> Option<ToastPosition> {
    Some(match name {
        "top-right" => ToastPosition::TopRight,
        "top-left" => ToastPosition::TopLeft,
        "top-center" => ToastPosition::TopCenter,
        "bottom-right" => ToastPosition::BottomRight,
        "bottom-left" => ToastPosition::BottomLeft,
        "bottom-center" => ToastPosition::BottomCenter,
        _ => return None,
    })
}

fn severity_prop(node: &ViewNode) -> ToastSeverity {
    match node.props.get("severity").and_then(PropValue::as_text) {
        Some("success") => ToastSeverity::Success,
        Some("warning") => ToastSeverity::Warning,
        Some("danger") => ToastSeverity::Danger,
        _ => ToastSeverity::Info,
    }
}

/// The handler for a **`"toggle"`** binding (a collapsible group) — the third canonical event name,
/// alongside `press` and `change`.
///
/// A toggle is only meaningful with its **new state**, so the bound intent is dispatched with
/// `args["expanded"]` set: an author binds one action and learns which way it went, instead of
/// having to track the group's state on their side.
fn toggle_change(node: &ViewNode, emit: &IntentEmitter) -> Option<impl Fn(Action) + 'static> {
    let intent = node.intent("toggle")?.clone();
    let emit = emit.clone();
    Some(move |action: Action| {
        let SignalData::Bool(expanded) = action.data else {
            return;
        };
        let mut intent = intent.clone();
        intent
            .args
            .insert("expanded".into(), PropValue::Bool(expanded));
        emit(intent);
    })
}

// ── Prop readers ──────────────────────────────────────────────────────────────────────
// Small helpers that pull a typed value out of the node's prop bag. Missing/mistyped props
// are simply absent (the widget keeps its default), never an error — the model is untrusted
// input (plugins/RPC) so realize is total.

/// The `"text"` prop as an owned string (empty if absent) — labels, button captions, tags.
fn text_of(node: &ViewNode) -> String {
    node.props
        .get("text")
        .and_then(PropValue::as_text)
        .unwrap_or("")
        .to_string()
}

/// An index prop (`"selected"`) as a `usize`. Negative values are ignored (the widget keeps its
/// default) rather than wrapping — the model is untrusted input.
fn usize_prop(node: &ViewNode, key: &str) -> Option<usize> {
    match node.props.get(key)? {
        PropValue::Int(i) => usize::try_from(*i).ok(),
        _ => None,
    }
}

/// An option's `"value"` prop — what the option *means*, as opposed to what it shows. `Text` or
/// `Int`; any other type is ignored. Kept as a [`PropValue`] so the type survives the round trip
/// into the change intent's args.
fn value_prop(node: &ViewNode) -> Option<PropValue> {
    match node.props.get("value")? {
        v @ (PropValue::Text(_) | PropValue::Int(_)) => Some(v.clone()),
        _ => None,
    }
}

/// An option value as the plain string the widget stores (`heca-grid-ui` never depends on the app,
/// so a `Choice` carries a `String`; the richer `PropValue` is re-attached at the boundary — see
/// [`option_change`]).
fn value_string(value: &PropValue) -> String {
    match value {
        PropValue::Text(s) => s.clone(),
        PropValue::Int(i) => i.to_string(),
        _ => String::new(),
    }
}

/// The option **values** of a `Select`/`Tabs` node, in child order — the strings its `Choice`
/// children carry. A named `Select` marshals the value at the chosen index into the form `data`
/// (mirrors [`realize_choice`]'s own value derivation: explicit `value` prop, else the label).
fn option_values(node: &ViewNode) -> Vec<String> {
    node.children
        .iter()
        .filter(|c| c.kind == WidgetKind::Choice)
        .map(|c| {
            value_prop(c)
                .map(|v| value_string(&v))
                .unwrap_or_else(|| text_of(c))
        })
        .collect()
}

/// The field `"name"` a value widget submits its value under (`ModalResult::Action`'s `data`).
/// Absent → the widget isn't collected.
fn name_prop(node: &ViewNode) -> Option<String> {
    node.props
        .get("name")
        .and_then(PropValue::as_text)
        .map(str::to_string)
}

/// The icon glyph from the `"icon"` (or `"glyph"`) prop, resolved from its Phosphor name.
fn glyph_prop(node: &ViewNode) -> Option<Glyph> {
    let value = node.props.get("icon").or_else(|| node.props.get("glyph"))?;
    match value {
        PropValue::Glyph(name) | PropValue::Text(name) => glyph_from_name(name),
        _ => None,
    }
}

/// Resolve a Phosphor glyph **name** (the model carries names, not codepoints) to a [`Glyph`].
/// `Glyph` is a closed, curated set with no serde/`FromStr`, so this is the single binding point
/// (mirrors [`map_variant`]/[`map_size`]). Unknown names → `None` (the widget draws no icon).
///
/// NB: this is a **curated stopgap of ~35 icons**. The complete app iconset + a generated
/// name↔`Glyph` mapping (so this can't drift from the enum) is tracked as `plugin-task-ui-8`.
fn glyph_from_name(name: &str) -> Option<Glyph> {
    // **Looked up, not listed.** This was a hand-written `match` of 36 arms beside a `Glyph::ALL`
    // of 52, so sixteen glyphs — `caret_left`, `pencil`, `x_circle`, `folder_simple_plus` and the
    // rest — could not be named from a description at all and silently rendered nothing. A second
    // copy of a mapping drifts; `Glyph::name` is the one source and a new glyph cannot compile
    // without joining it (F003/P082/T444).
    Glyph::ALL.iter().copied().find(|g| g.name() == name)
}

/// The `"variant"` prop mapped to the grid-ui [`ButtonVariant`].
fn variant_prop(node: &ViewNode) -> Option<ButtonVariant> {
    match node.props.get("variant")? {
        PropValue::Variant(v) => Some(map_variant(*v)),
        _ => None,
    }
}

/// The `"size"` prop mapped to the grid-ui [`WidgetSize`].
fn size_prop(node: &ViewNode) -> Option<WidgetSize> {
    match node.props.get("size")? {
        PropValue::Size(s) => Some(map_size(*s)),
        _ => None,
    }
}

// ── Semantic enum → grid-ui enum ──
// The model carries semantic mirrors of the grid-ui enums (so it never depends on grid-ui);
// realize is the single place that binds them across.

// There is no `ViewAlign` → `Align` mapper here on purpose. Alignment is an ordinary property:
// it travels through `prop_to_json` and lands on the widget's own `Layout` by name, on both axes
// and on children (`align_self` / `justify_self`) — see
// `grid_alignment_is_authorable_on_both_axes`. A hand-written mapper existed until the
// generic property surface landed and was dead from that day; the app's module-wide
// `allow(dead_code)` is what kept it invisible.

fn map_variant(v: ViewVariant) -> ButtonVariant {
    match v {
        ViewVariant::Primary => ButtonVariant::Primary,
        ViewVariant::Secondary => ButtonVariant::Secondary,
        ViewVariant::Destructive => ButtonVariant::Destructive,
        ViewVariant::Outline => ButtonVariant::Outline,
        ViewVariant::Ghost => ButtonVariant::Ghost,
        ViewVariant::Link => ButtonVariant::Link,
    }
}

fn map_size(s: ViewSize) -> WidgetSize {
    match s {
        ViewSize::Small => WidgetSize::Small,
        ViewSize::Normal => WidgetSize::Normal,
        ViewSize::Large => WidgetSize::Large,
        ViewSize::Header => WidgetSize::Header,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The fixed value sets are used by the tests, not by the mapper: a widget's own enum parses the
    // name, so `realize` never names these types.
    use heca_view::{
        ViewLabelSide, ViewMarker, ViewOrientation, ViewScrollAxes, ViewSeverity, ViewTextAlign,
    };
    use heca_grid_ui::{Justify, Length};
    use std::rc::Rc;

    /// **What the framework's picker will find in a realized tree**, in document order: one path
    /// per node that declared what a pick does to it. There is no registry to interrogate any more
    /// — the declarations are in the tree, so the tests read them from the tree.
    fn hints(root: &dyn Component) -> Vec<Vec<usize>> {
        heca_grid_ui::collect_hints(root)
            .into_iter()
            .map(|(path, _)| path)
            .collect()
    }

    /// **A described surface arrives and leaves exactly as a native one does** (F003/P082/T459).
    ///
    /// The declarative half of `Overlay::animation`: a plugin that can only send JSON names a
    /// built-in and gets the same gesture native code gets — one door, two spellings. The surface
    /// then holds itself on screen for the whole of its exit, which is the behaviour that makes an
    /// animated dismissal possible at all.
    #[test]
    fn a_described_overlay_names_how_it_arrives_and_leaves() {
        let emit: IntentEmitter = Rc::new(|_| {});
        let mut forms = FormBindings::default();
        let theme = Theme::default();

        let node = ViewNode::new(WidgetKind::Overlay)
            .prop("animation", PropValue::Text("zoom_fade".into()))
            .prop("default_open", PropValue::Bool(true))
            .child(ViewNode::new(WidgetKind::Label).text("MAP"));
        let mut surface = realize(&node, &theme, &emit, &mut forms);

        assert!(surface.presence().is_some(), "a described overlay is a surface a host can drive");
        surface.close();
        assert!(
            surface.presence().is_some_and(|p| p.is_leaving()),
            "the named animation plays on the way out",
        );
        let mut frames = 0;
        while surface.tick(1.0 / 60.0) && frames < 600 {
            frames += 1;
        }
        assert!(frames > 1, "it played rather than cutting: {frames} frames");
        assert!(!surface.presence().is_some_and(|p| p.is_leaving()), "and then it is gone");

        // Untrusted input stays total: an unknown name leaves the surface with its default.
        let unknown = ViewNode::new(WidgetKind::Overlay)
            .prop("animation", PropValue::Text("supernova".into()))
            .child(ViewNode::new(WidgetKind::Label).text("MAP"));
        let mut cut = realize(&unknown, &theme, &emit, &mut forms);
        cut.show();
        cut.close();
        assert!(!cut.presence().is_some_and(|p| p.is_leaving()), "a cut, not a panic");
    }

    /// **A described surface can ask for the frost the exposé uses** — the plugin half of the
    /// backdrop.
    ///
    /// It matters because the blur is GPU work: if a plugin could not *describe* it, frosting would
    /// be reachable only from native code and every plugin overlay would sit flat over a sharp
    /// session. It asks for the effect and never a radius — strength is the theme's, so one theme
    /// answers for every surface at once.
    #[test]
    fn a_described_overlay_can_ask_for_the_frost_behind_it() {
        let emit: IntentEmitter = Rc::new(|_| {});
        let mut forms = FormBindings::default();
        let theme = Theme::default();

        let mut backdrops = |frosted: bool| {
            let node = ViewNode::new(WidgetKind::Overlay)
                .prop("frosted", PropValue::Bool(frosted))
                .prop("default_open", PropValue::Bool(true))
                .child(ViewNode::new(WidgetKind::Label).text("MAP"));
            let mut w = realize(&node, &theme, &emit, &mut forms);
            heca_grid_ui::LayoutEngine::new()
                .compute(w.as_mut(), heca_core::layout::Size::new(800.0, 600.0));
            let mut scene = heca_grid_ui::Scene::new();
            {
                let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme)
                    .with_viewport(heca_core::layout::Size::new(800.0, 600.0));
                w.paint(&mut cx);
            }
            scene
                .iter()
                .filter(|c| {
                    matches!(
                        c,
                        heca_grid_ui::scene::DrawCommand::Host(h)
                            if matches!(h.draw, heca_grid_ui::scene::HostDraw::Backdrop { .. })
                    )
                })
                .count()
        };

        assert_eq!(backdrops(true), 1, "the described surface asked for its blur");
        assert_eq!(backdrops(false), 0, "and one that did not ask pays for no pass");
    }

    /// A confirm-dialog-shaped tree: a column with a message label + a row of two action
    /// buttons (Cancel / Delete), each carrying a `"press"` intent.
    fn confirm_tree() -> ViewNode {
        ViewNode::new(WidgetKind::VStack)
            .prop("gap", PropValue::Int(8))
            .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
            .child(
                ViewNode::new(WidgetKind::HStack)
                    .child(
                        ViewNode::new(WidgetKind::Button)
                            .text("Cancel")
                            .prop("variant", PropValue::Variant(ViewVariant::Secondary))
                            .on_press(Intent::new("confirm_cancel")),
                    )
                    .child(
                        ViewNode::new(WidgetKind::Button)
                            .text("Delete")
                            .prop("variant", PropValue::Variant(ViewVariant::Destructive))
                            .on_press(Intent::new("confirm_ok")),
                    ),
            )
    }

    fn noop_emitter() -> IntentEmitter {
        Rc::new(|_| {})
    }

    /// An emitter that keeps every intent a realized tree fires, so a test can pick a letter (or
    /// click) and read back exactly what the host would have been handed.
    fn recording_emitter() -> (IntentEmitter, Rc<std::cell::RefCell<Vec<Intent>>>) {
        let sink = Rc::new(std::cell::RefCell::new(Vec::new()));
        let out = sink.clone();
        (Rc::new(move |i| sink.borrow_mut().push(i)), out)
    }

    /// The realized tree mirrors the model's structure: the column has 2 children (label +
    /// row) and the row has 2 children (the buttons). Verifies recursion + child attachment
    /// through `base_mut().children` (the `Box<dyn Component>` push path).
    #[test]
    fn realizes_nested_structure() {
        let root = realize(&confirm_tree(), &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(root.base().children.len(), 2, "column: label + row");
        let row = &root.base().children[1];
        assert_eq!(row.base().children.len(), 2, "row: two buttons");
    }

    /// Every actionable node (a `"press"` binding) declares exactly one hint carrying the node's
    /// own intent — so the picker fires the identical action a click would. Non-actionable nodes
    /// declare nothing.
    #[test]
    fn actionable_nodes_declare_the_hint_their_click_would_fire() {
        let (emit, fired) = recording_emitter();
        let mut root = realize(&confirm_tree(), &Theme::default(), &emit, &mut FormBindings::default());
        let found = hints(root.as_ref());
        assert_eq!(
            found.len(),
            2,
            "only the two buttons are actionable (label + containers are not)",
        );
        // Document order: the row is the column's second child, Cancel its first.
        assert_eq!(found, vec![vec![1, 0], vec![1, 1]]);
        for path in &found {
            assert!(heca_grid_ui::fire_hint(root.as_mut(), path));
        }
        let actions: Vec<String> = fired.borrow().iter().map(|i| i.action.clone()).collect();
        assert_eq!(actions, vec!["confirm_cancel", "confirm_ok"]);
    }

    /// **A pick is not a click.** A node that declares its own `hint` fires *that*, not its
    /// `press` — which is the whole reason the two are separate events: heca's sidebar row
    /// activates the pane and leaves on a click, and stays in the sidebar on a hint pick.
    #[test]
    fn a_declared_hint_wins_over_the_press() {
        let (emit, fired) = recording_emitter();
        let node = ViewNode::new(WidgetKind::Row)
            .on_press(Intent::new("activate_and_leave"))
            .on_hint(Intent::new("hint_and_stay"));
        let mut row = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert!(heca_grid_ui::fire_hint(row.as_mut(), &[]));
        assert_eq!(
            fired.borrow().iter().map(|i| i.action.clone()).collect::<Vec<_>>(),
            vec!["hint_and_stay"],
        );
    }

    /// A `Button` node's **children are its content**: an arbitrary subtree is realized and mounted
    /// inside the button. Before the button composed its content, `realize` had nowhere to put them
    /// and dropped them silently — a declarative `Button(Icon + Label)` rendered as a bare button.
    #[test]
    fn button_children_are_realized_as_its_content() {
        let node = ViewNode::new(WidgetKind::Button)
            .prop("variant", PropValue::Variant(ViewVariant::Destructive))
            .on_press(Intent::new("confirm_ok"))
            .child(
                ViewNode::new(WidgetKind::VStack)
                    .prop("gap", PropValue::Int(4))
                    .child(
                        ViewNode::new(WidgetKind::HStack)
                            .child(
                                ViewNode::new(WidgetKind::Icon)
                                    .prop("icon", PropValue::Glyph("trash".into())),
                            )
                            .child(ViewNode::new(WidgetKind::Label).text("Delete")),
                    )
                    .child(ViewNode::new(WidgetKind::Label).text("Ctrl+D")),
            );

        let button = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let column = &button.base().children;
        assert_eq!(column.len(), 1, "the button holds its composed subtree");
        let column = &column[0].base().children;
        assert_eq!(column.len(), 2, "column: the icon+label row, then the accelerator label");
        assert_eq!(column[0].base().children.len(), 2, "row: icon + label");

        // The button is still one pick target, whatever it composes.
        assert_eq!(hints(button.as_ref()), vec![Vec::<usize>::new()], "the button, not its content");
    }

    /// A **childless** Button node falls back to the scalar sugar — `text` (+ an optional leading
    /// `icon`) — which builds the very same children the explicit form would. One content model,
    /// two spellings.
    #[test]
    fn childless_button_node_uses_the_scalar_sugar() {
        let node = ViewNode::new(WidgetKind::Button)
            .text("Delete")
            .prop("icon", PropValue::Glyph("trash".into()));
        let button = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(
            button.base().children.len(),
            2,
            "text + icon props desugar into [Icon, Label] children",
        );
    }

    // ── Appearance is overridable; the theme is the default (F003/P017/T7) ──

    /// A description sets **any** of `Visual`, not just colour: fill, border, glow, radius and
    /// font size all arrive through the same generic merge as `Layout`.
    #[test]
    fn a_description_can_override_the_whole_of_visual() {
        let node = ViewNode::new(WidgetKind::Surface)
            .prop("fill", PropValue::Color("#ff8800".into()))
            .prop("radius", PropValue::Float(12.0))
            .prop("font_size", PropValue::Float(18.0));
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        let visual = w.base().style.visual;
        assert_eq!(visual.fill, Some(heca_grid_ui::Color::rgb(0xff, 0x88, 0x00)));
        assert_eq!(visual.radius, 12.0);
        assert_eq!(visual.font_size, 18.0);
    }

    /// **Unset still means "ask the theme".** A node that overrides one field leaves every other
    /// one at the widget's own value, so nothing about a non-overriding widget changed.
    #[test]
    fn an_unset_appearance_field_is_left_alone() {
        let plain = realize(
            &ViewNode::new(WidgetKind::Surface),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        let one_field = realize(
            &ViewNode::new(WidgetKind::Surface).prop("radius", PropValue::Float(9.0)),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(one_field.base().style.visual.radius, 9.0, "the one it set");
        assert_eq!(
            one_field.base().style.visual.fill,
            plain.base().style.visual.fill,
            "everything else is untouched — unset is not 'set to nothing'",
        );
    }

    /// **A theme token name resolves against the theme the tree is built with**, so the same
    /// description gives a different pixel under a different theme — which is what makes a token
    /// follow `prefix+Shift+r`, since a reload drops the retained trees and rebuilds them.
    ///
    /// A hex literal is the same pixel under either theme. That is the trade a caller makes by
    /// writing one, and the reason the docs steer toward token names.
    #[test]
    fn a_theme_token_follows_the_theme_and_a_hex_literal_does_not() {
        let mut dark = Theme::default();
        dark.colors.accent = heca_grid_ui::Color::rgb(0x11, 0x22, 0x33);
        let mut light = Theme::default();
        light.colors.accent = heca_grid_ui::Color::rgb(0xee, 0xdd, 0xcc);

        let fill_of = |node: &ViewNode, theme: &Theme| {
            realize(
                node,
                theme,
                &noop_emitter(),
                &mut FormBindings::default(),
            )
            .base()
            .style
            .visual
            .fill
        };

        let token = ViewNode::new(WidgetKind::Surface).prop("fill", PropValue::Color("accent".into()));
        assert_eq!(fill_of(&token, &dark), Some(dark.colors.accent));
        assert_eq!(fill_of(&token, &light), Some(light.colors.accent));

        let hex = ViewNode::new(WidgetKind::Surface).prop("fill", PropValue::Color("#ff0000".into()));
        let literal = Some(heca_grid_ui::Color::rgb(0xff, 0, 0));
        assert_eq!(fill_of(&hex, &dark), literal);
        assert_eq!(fill_of(&hex, &light), literal, "a literal is not a token");
    }

    /// The accepted token vocabulary **is** the theme's own colour fields — nothing is written down
    /// twice. Add a colour to the theme and a description can name it with no table to update.
    #[test]
    fn the_token_vocabulary_is_the_themes_own_fields() {
        let theme = Theme::default();
        for token in ["accent", "foreground", "muted", "border", "danger", "warning", "success"] {
            assert!(
                resolve_color(token, &theme).is_some(),
                "{token} is a theme colour and should resolve",
            );
        }
        assert!(resolve_color("chartreuse", &theme).is_none(), "not a theme colour");
        // A non-colour theme field cannot be named by accident.
        assert!(resolve_color("name", &theme).is_none(), "the theme's NAME is not a colour");
    }

    /// A token reaches a **widget's own colour builder**, not just `Visual`.
    ///
    /// `Label::color`, `Icon::color`, `Tag`'s hue, `Row::highlight` and the rest were all marked
    /// `host_only("colour — reachable once F003/P017/T7 makes appearance overridable")`. This is
    /// that promise being kept: the host resolves the token to hex before the value crosses, so the
    /// library still knows nothing about themes and `Color::from_str` still only knows hex.
    #[test]
    fn a_token_reaches_a_widgets_own_colour_builder() {
        let mut theme = Theme::default();
        theme.colors.danger = heca_grid_ui::Color::rgb(0xc0, 0x10, 0x20);

        let painted = |node: &ViewNode, theme: &Theme| {
            use heca_grid_ui::{LayoutEngine, PaintCx, Scene};
            use heca_core::layout::Size;
            let mut w = realize(
                node,
                theme,
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            LayoutEngine::new().compute(w.as_mut(), Size::new(300.0, 40.0));
            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, theme);
                w.paint(&mut cx);
            }
            scene
        };

        let tinted = ViewNode::new(WidgetKind::Label)
            .text("This action cannot be undone.")
            .prop("color", PropValue::Color("danger".into()));
        let plain = ViewNode::new(WidgetKind::Label).text("This action cannot be undone.");

        // The label paints; the tinted one does not paint the same thing as the untinted one.
        let a = format!("{:?}", painted(&tinted, &theme));
        let b = format!("{:?}", painted(&plain, &theme));
        assert!(!a.is_empty(), "the label painted something");
        assert_ne!(a, b, "the token override changed what was drawn");
        // And the colour it used is the THEME's danger, so a different theme paints differently.
        let mut other = Theme::default();
        other.colors.danger = heca_grid_ui::Color::rgb(0x10, 0xc0, 0x20);
        assert_ne!(
            a,
            format!("{:?}", painted(&tinted, &other)),
            "a token follows the theme it was built with",
        );
    }

    /// A **struct-shaped** appearance property reaches the widget.
    ///
    /// `border` and `glow` were unreachable from a description for as long as a property value was
    /// a closed list of scalars: `Border` is `{color, width}` and nothing could carry it.
    /// F003/P017/T007's commit claimed "the whole of `Visual`" on the strength of adding serde to
    /// the two types, which is not the same thing, and no test looked — so nothing failed.
    /// `PropValue::Map` is what makes the claim true (F003/P011/T018).
    #[test]
    fn a_struct_shaped_appearance_property_reaches_the_widget() {
        let node = ViewNode::new(WidgetKind::Surface)
            .prop(
                "border",
                PropValue::Map(PropMap::from([
                    ("color".into(), PropValue::Color("#ff8800".into())),
                    ("width".into(), PropValue::Float(2.0)),
                ])),
            )
            .prop(
                "glow",
                PropValue::Map(PropMap::from([
                    ("color".into(), PropValue::Color("#00ccff".into())),
                    ("radius".into(), PropValue::Float(12.0)),
                    ("intensity".into(), PropValue::Float(0.4)),
                ])),
            );
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        let border = w.base().style.visual.border.expect("a described border reaches the widget");
        assert_eq!(border.width, 2.0);
        assert_eq!(border.color, heca_grid_ui::Color::rgb(0xff, 0x88, 0x00));
        let glow = w.base().style.visual.glow.expect("a described glow reaches the widget");
        assert_eq!(glow.radius, 12.0);
        assert_eq!(glow.intensity, 0.4);
        assert_eq!(glow.color, heca_grid_ui::Color::rgb(0x00, 0xcc, 0xff));
    }

    /// A described node stretches to its parent, the way CSS flexbox does.
    ///
    /// Set no width and a node fills its parent's cross axis; set `width_pct(0.5)` and it takes
    /// half; set `width(px)` and it takes exactly that. Authors rely on the first without asking
    /// for it — the showcase's panels are only a fixed size because they say so — and it is
    /// currently true because a container's default `align` is `Stretch`, which is taffy agreeing
    /// with CSS.
    ///
    /// **Nothing pinned that until this test.** Changing a container's default alignment would
    /// silently turn every full-width described panel into a content-width one, with no test
    /// failing and nothing to read in a diff. It is a contract now.
    #[test]
    fn a_described_node_stretches_to_its_parent_like_css() {
        use heca_view::build::{self, Parent as _, Style as _};

        // Mount a described tree in a plain column of a known width, as a page lays sections out,
        // and report the node's width plus its first child's.
        let mounted = |node: ViewNode| {
            let realized = realize(
                &node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            let mut parent = Flex::column().width(heca_grid_ui::Length::Px(600.0));
            parent.base_mut().children.push(realized);
            heca_grid_ui::LayoutEngine::new()
                .compute(&mut parent, heca_core::layout::Size::new(600.0, 400.0));
            let child = &parent.base().children[0];
            let inner = child
                .base()
                .children
                .last()
                .map(|c| c.base().bounds.size.w)
                .unwrap_or_default();
            (child.base().bounds.size.w, inner)
        };

        let panel = |b: build::Panel| {
            b.child(build::Row::new().child(build::Label::new("nginx")))
                .into_node()
        };

        // No width: full parent width, and the row inside fills the panel's content box (600 less
        // the panel's 10px padding a side).
        assert_eq!(
            mounted(panel(build::Panel::new().title("P"))),
            (600.0, 580.0),
            "an unsized node fills its parent, and its child fills it in turn",
        );

        // The same result asked for explicitly.
        assert_eq!(
            mounted(panel(build::Panel::new().title("P").width_pct(1.0))).0,
            600.0,
            "width_pct(1.0) is the full parent width",
        );

        // A fraction, which is the case a percentage is actually needed for.
        assert_eq!(
            mounted(panel(build::Panel::new().title("P").width_pct(0.5))).0,
            300.0,
            "width_pct(0.5) is half the parent",
        );

        // And a fixed size wins over the stretch — this is what keeps the showcase's two demo
        // panels side by side instead of splitting the row.
        assert_eq!(
            mounted(panel(build::Panel::new().title("P").width(240.0))),
            (240.0, 220.0),
            "an explicit width is honoured, padding still taken off the child",
        );
    }

    /// Every widget property is reachable from the typed SDK.
    ///
    /// `heca-view::build` is hand-written — that was the choice, over generating it — so the thing
    /// that keeps it honest is this. It walks each widget's generated `PROP_NAMES` and fails when a
    /// property has no named setter on that kind's builder, which is how a capability added to the
    /// library reaches the authoring layer instead of quietly not existing.
    ///
    /// It **fails closed**: a property must be reachable unless it is named below with a reason.
    /// The opposite arrangement — a list you must remember to add to — is what let `placeholder`
    /// and the scroll axes go unreachable for months (F003/P017).
    ///
    /// The check reads the SDK's source rather than calling it, because "does a method exist" is
    /// not a question a running test can ask. That is the same technique `prop_drift.rs` uses, and
    /// for the same reason.
    #[test]
    fn every_widget_property_is_reachable_from_the_sdk() {
        /// Properties with no setter, each with the reason. Keep it short — an entry here is a
        /// capability an author cannot reach.
        const NOT_IN_SDK: &[(&str, &str, &str)] = &[
            (
                "Button",
                "font_size",
                "on Style already — every kind takes font_size, so a per-kind copy would be a \
                 second way to say the same thing",
            ),
            (
                "Input",
                "font_size",
                "on Style already",
            ),
            (
                "Select",
                "font_size",
                "on Style already",
            ),
            (
                "Tabs",
                "font_size",
                "on Style already",
            ),
            (
                "Item",
                "font_size",
                "on Style already",
            ),
            (
                "Label",
                "font_size",
                "on Style already",
            ),
            (
                "Label",
                "font_scale",
                "on Style already",
            ),
        ];

        let sdk = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../heca-view/src/build.rs"),
        )
        .expect("the SDK source is where it is expected");

        // Which builder impl a method sits in: the guard is per-kind, so a setter on the wrong
        // builder must not satisfy another's property.
        let block_for = |widget: &str| -> Option<String> {
            let head = format!("\nimpl {widget} {{\n");
            let start = sdk.find(&head)? + head.len();
            let rest = &sdk[start..];
            let end = rest.find("\n}\n").unwrap_or(rest.len());
            Some(rest[..end].to_string())
        };

        let mut missing: Vec<String> = Vec::new();
        let mut check = |widget: &str, props: &[&str], sdk_name: &str| {
            let block = block_for(sdk_name).unwrap_or_default();
            for prop in props {
                let excused = NOT_IN_SDK
                    .iter()
                    .any(|(w, p, _)| *w == sdk_name && p == prop);
                if excused {
                    continue;
                }
                // A setter reaches the property if it writes that key, whatever the method is
                // called: `Button::glowing` sets "glow", because `glow` on Style means the halo.
                let writes_key = block.contains(&format!("self.prop(\"{prop}\"")) 
                    || block.contains(&format!("props.insert(\"{prop}\""));
                if !writes_key {
                    missing.push(format!("{widget}::{prop} (builder {sdk_name})"));
                }
            }
        };

        check("Alert", <Alert as SetProp>::PROP_NAMES, "Alert");
        check("Badge", <Badge as SetProp>::PROP_NAMES, "Badge");
        check("BadgeButton", <BadgeButton as SetProp>::PROP_NAMES, "BadgeButton");
        check("Button", <Button as SetProp>::PROP_NAMES, "Button");
        check("Checkbox", <Checkbox as SetProp>::PROP_NAMES, "Checkbox");
        check("Choice", <Choice as SetProp>::PROP_NAMES, "Choice");
        check("DockFrame", <DockFrame as SetProp>::PROP_NAMES, "DockFrame");
        check("Gauge", <Gauge as SetProp>::PROP_NAMES, "Gauge");
        check("Icon", <Icon as SetProp>::PROP_NAMES, "Icon");
        check("IconButton", <IconButton as SetProp>::PROP_NAMES, "IconButton");
        check("Input", <Input as SetProp>::PROP_NAMES, "Input");
        check("Item", <Item as SetProp>::PROP_NAMES, "Item");
        check("ItemGroup", <ItemGroup as SetProp>::PROP_NAMES, "ItemGroup");
        check("KeyHintGroup", <KeyHintGroup as SetProp>::PROP_NAMES, "KeyHintGroup");
        check("Label", <Label as SetProp>::PROP_NAMES, "Label");
        check("MarkerGroup", <MarkerGroup as SetProp>::PROP_NAMES, "MarkerGroup");
        check("Overlay", <Overlay as SetProp>::PROP_NAMES, "Overlay");
        check("Panel", <Panel as SetProp>::PROP_NAMES, "Panel");
        check("RailCell", <RailCell as SetProp>::PROP_NAMES, "RailCell");
        check("Row", <GridRow as SetProp>::PROP_NAMES, "Row");
        check("ScrollRegion", <ScrollRegion as SetProp>::PROP_NAMES, "Scroll");
        check("Select", <Select as SetProp>::PROP_NAMES, "Select");
        check("Separator", <Separator as SetProp>::PROP_NAMES, "Separator");
        check("Tabs", <Tabs as SetProp>::PROP_NAMES, "Tabs");
        check("Tag", <Tag as SetProp>::PROP_NAMES, "Tag");
        check("Toast", <Toast as SetProp>::PROP_NAMES, "Toast");
        check("Toggle", <Toggle as SetProp>::PROP_NAMES, "Toggle");
        check(
            "ProgressBar",
            <ProgressBar as SetProp>::PROP_NAMES,
            "Progress",
        );
        check("NfIcon", <NfIcon as SetProp>::PROP_NAMES, "NfIcon");
        check(
            "ButtonGroup",
            <heca_grid_ui::widgets::ButtonGroup as SetProp>::PROP_NAMES,
            "ButtonGroup",
        );

        assert!(
            missing.is_empty(),
            "these widget properties have no setter in heca-view/src/build.rs, so a description \
             cannot reach them through the SDK: {missing:#?}\n\nAdd a setter, or add the property \
             to NOT_IN_SDK with the reason.",
        );

        // ── And the list above cannot silently fall behind the vocabulary ────────────────────
        //
        // Everything above is a **hand-written** call per widget, which is the shape this file's
        // own doc calls the defect: a list you must remember to add to. It failed exactly that way
        // — `Progress` was added to `WidgetKind` with a property, and every guard stayed green
        // because nobody had written its line, so the check silently covered 33 of 35 kinds.
        //
        // It cannot be derived (a kind does not name its Rust type), so instead the omission is
        // made loud: every kind must be checked above, or excused here by name with a reason.
        const NO_PROPERTIES_TO_CHECK: &[(&str, &str)] = &[
            ("VStack", "a plain box: `Flex` has only Style properties"),
            ("HStack", "the same box, laid out the other way"),
            ("Grid", "tracks are read by hand, not via the surface"),
            ("CardGrid", "cells are built by the realizer, not props"),
            ("Card", "a titled surface; its title is the `text` sugar"),
            ("Surface", "a plain painted box"),
            ("Spinner", "no properties; animates off the clock"),
            ("StatusDot", "its state picks the constructor"),
            ("ScrollBar", "host-only; `realize` refuses it"),
        ];

        let own_source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
        )
        .expect("this file is where it is expected");
        // The third argument of each `check(..)` is the kind's SDK name — the one that has to line
        // up with `WidgetKind`.
        // Whitespace-insensitive on purpose: written to match `PROP_NAMES, "` it read nothing the
        // moment rustfmt wrapped one of these calls across lines, and a guard that quietly stops
        // reading is worse than the gap it exists for — which is why the size assertion below is
        // not decoration.
        let checked: std::collections::BTreeSet<&str> = own_source
            .match_indices("PROP_NAMES,")
            .filter_map(|(i, m)| {
                let rest = own_source[i + m.len()..].trim_start();
                let rest = rest.strip_prefix('"')?;
                rest.find('"').map(|e| &rest[..e])
            })
            .collect();
        assert!(
            checked.len() > 20,
            "this guard has stopped reading the check list, which is worse than the gap it exists \
             for — it found only {} entries",
            checked.len(),
        );

        let unchecked: Vec<String> = WidgetKind::ALL
            .iter()
            .map(|k| format!("{k:?}"))
            .filter(|name| !checked.contains(name.as_str()))
            .filter(|name| !NO_PROPERTIES_TO_CHECK.iter().any(|(n, _)| n == name))
            .collect();
        assert!(
            unchecked.is_empty(),
            "these kinds are in the vocabulary but no `check(..)` line above covers them, so their \
             properties could be unreachable from the SDK and nothing would say so: {unchecked:#?}\
             \n\nAdd a `check(..)` line, or name the kind in NO_PROPERTIES_TO_CHECK with a reason.",
        );
    }

    /// The mirrored glyph list cannot fall behind the real one.
    ///
    /// `heca-view` must not depend on `heca-grid-ui` — that independence is why a plugin can depend
    /// on the vocabulary at all — so the 52 icon names are copied into it. A copy of a list that
    /// grows is the failure this codebase keeps repeating, so this crate, which sees both, is where
    /// the copy is held to account. It compares **both ways** and names what is wrong: a glyph
    /// added to the library and not mirrored, or a mirror naming something that no longer exists.
    #[test]
    fn every_glyph_name_has_a_mirror() {
        use std::collections::BTreeSet;
        let library: BTreeSet<&str> = <Glyph as heca_grid_ui::PropName>::VARIANT_NAMES
            .iter()
            .copied()
            .collect();
        let mirrored: BTreeSet<&str> = heca_view::ViewGlyph::ALL.iter().map(|g| g.name()).collect();

        let missing: Vec<_> = library.difference(&mirrored).collect();
        assert!(
            missing.is_empty(),
            "these glyphs exist in heca-grid-ui but not in ViewGlyph, so a description cannot name \
             them: {missing:?} — add them to the generated block in heca-view/src/lib.rs",
        );
        let stale: Vec<_> = mirrored.difference(&library).collect();
        assert!(
            stale.is_empty(),
            "ViewGlyph names glyphs the library no longer has: {stale:?}",
        );

        // **The keyboard font is a second vocabulary, held to the same account** (F003/P097/T501).
        //
        // Checked here rather than in a test of its own: it is the same question about the same
        // kind of copied list, and two tests would be two places to remember when a third font
        // arrives.
        let nf_library: BTreeSet<&str> = <NfGlyph as heca_grid_ui::PropName>::VARIANT_NAMES
            .iter()
            .copied()
            .collect();
        let nf_mirrored: BTreeSet<&str> = heca_view::ViewNfGlyph::ALL
            .iter()
            .map(|g| g.name())
            .collect();
        let missing: Vec<_> = nf_library.difference(&nf_mirrored).collect();
        assert!(
            missing.is_empty(),
            "these keyboard glyphs exist in heca-grid-ui but not in ViewNfGlyph, so a description \
             cannot name them: {missing:?}",
        );
        let stale: Vec<_> = nf_mirrored.difference(&nf_library).collect();
        assert!(
            stale.is_empty(),
            "ViewNfGlyph names keyboard glyphs the library no longer has: {stale:?}",
        );
    }

    /// A mirrored glyph actually resolves to the icon it names.
    ///
    /// Matching names is not the same as the name being understood: this takes one through
    /// `PropValue` into a realized widget and asserts a different glyph paints differently.
    #[test]
    fn a_mirrored_glyph_reaches_the_icon() {
        let painted = |g: heca_view::ViewGlyph| {
            let node = ViewNode::new(WidgetKind::Icon).prop("glyph", g.into());
            let w = realize(
                &node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            let mut scene = heca_grid_ui::Scene::new();
            let theme = Theme::default();
            {
                let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
                w.paint(&mut cx);
            }
            format!("{:?}", scene.iter().collect::<Vec<_>>())
        };
        let folder = painted(heca_view::ViewGlyph::Folder);
        assert!(!folder.is_empty(), "a named glyph painted something");
        assert_ne!(
            folder,
            painted(heca_view::ViewGlyph::Terminal),
            "the name picked the icon, rather than every name giving the same one",
        );

        // **And the keyboard font, the same way** (F003/P097/T501). A shortcut drawn with the
        // wrong key is worse than one drawn with none — ⌘ where the author asked for ⇧ reads as
        // correct — and matching names cannot catch that, because the two lists agree by name while
        // the codepoints behind them are a separate mapping.
        let nf_painted = |g: heca_view::ViewNfGlyph| {
            let node = ViewNode::new(WidgetKind::NfIcon).prop("glyph", g.into());
            let w = realize(
                &node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            let mut scene = heca_grid_ui::Scene::new();
            let theme = Theme::default();
            {
                let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
                w.paint(&mut cx);
            }
            format!("{:?}", scene.iter().collect::<Vec<_>>())
        };
        let shift = nf_painted(heca_view::ViewNfGlyph::Shift);
        assert!(!shift.is_empty(), "a named key painted something");
        assert_ne!(
            shift,
            nf_painted(heca_view::ViewNfGlyph::Command),
            "the name picked the key, rather than every name giving the same one",
        );
    }

    /// Every fixed value set reaches the widget it belongs to.
    ///
    /// The types added by F003/P011/T019 stop a misspelling from compiling, but nothing about them
    /// guarantees the *name* they travel under is one the widget parses — that agreement runs
    /// across two crates and a serde attribute. So each is checked against a real realized widget
    /// rather than against a parse: written through the type, it must change the widget.
    ///
    /// `PropValue::from` is what the authoring layer calls, so this exercises the whole path.
    #[test]
    fn every_fixed_value_set_reaches_its_widget() {
        let realize_one = |node: &ViewNode| {
            realize(
                node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            )
        };
        let theme = Theme::default();
        let painted = |node: &ViewNode| {
            let mut w = realize_one(node);
            heca_grid_ui::LayoutEngine::new()
                .compute(w.as_mut(), heca_core::layout::Size::new(300.0, 80.0));
            let mut scene = heca_grid_ui::Scene::new();
            {
                let mut cx = heca_grid_ui::PaintCx::new(&mut scene, &theme);
                w.paint(&mut cx);
            }
            format!("{:?}", scene.iter().collect::<Vec<_>>())
        };

        // Orientation shows up in layout: a vertical rule is tall and thin, a horizontal one wide
        // and thin, so the property is visible in the box rather than merely stored.
        let rule = |o: ViewOrientation| {
            let n = ViewNode::new(WidgetKind::Separator)
                .prop("orientation", o.into())
                .prop("length", PropValue::Float(40.0));
            let w = realize_one(&n);
            format!("{:?}", w.base().style.layout)
        };
        assert_ne!(
            rule(ViewOrientation::Vertical),
            rule(ViewOrientation::Horizontal),
            "orientation reached the separator",
        );

        // Axes are the region's own state, not a `Layout` field, so ask the widget — the same way
        // `a_scroll_regions_axes_are_authorable` does.
        let region = |a: ViewScrollAxes| {
            let node = ViewNode::new(WidgetKind::Scroll).prop("axes", a.into());
            with_props(ScrollRegion::new(), &node, &Theme::default()).clone_axes()
        };
        assert_ne!(
            region(ViewScrollAxes::Both),
            region(ViewScrollAxes::Vertical),
            "axes reached the scroll region",
        );

        let toast = |s: ViewSeverity| {
            painted(
                &ViewNode::new(WidgetKind::Toast)
                    .text("Build failed")
                    .prop("severity", s.into()),
            )
        };
        assert_ne!(
            toast(ViewSeverity::Danger),
            toast(ViewSeverity::Info),
            "severity reached the toast",
        );

        let row = |m: ViewMarker| {
            painted(
                &ViewNode::new(WidgetKind::Row)
                    .prop("active", PropValue::Bool(true))
                    .prop("marker", m.into())
                    .child(ViewNode::new(WidgetKind::Label).text("x")),
            )
        };
        assert_ne!(row(ViewMarker::Check), row(ViewMarker::Bar), "marker reached the row");

        let label = |a: ViewTextAlign| {
            painted(
                &ViewNode::new(WidgetKind::Label)
                    .text("STATUS")
                    .prop("align", a.into()),
            )
        };
        assert_ne!(
            label(ViewTextAlign::Center),
            label(ViewTextAlign::Start),
            "text align reached the label",
        );

        // Truncation is a described property too: a plugin's label must be able to fit its box
        // without the plugin measuring anything. `Start` and `End` keep opposite halves, so the two
        // paint differently the moment the box is too small for the text.
        // The label is wrapped in a narrow container, because that is the only way truncation is
        // ever reached: a `Label` sizes itself from its text, and the cut happens when the layout
        // hands it less than that. A width prop on the label itself would be overwritten by its own
        // remeasure.
        let cut = |mode: heca_view::ViewEllipsis| {
            painted(
                &ViewNode::new(WidgetKind::HStack)
                    .prop("width", PropValue::Int(60))
                    .child(
                        ViewNode::new(WidgetKind::Label)
                            .text("projects/heca/src/widgets")
                            .prop("truncate", mode.into()),
                    ),
            )
        };
        assert_ne!(
            cut(heca_view::ViewEllipsis::Start),
            cut(heca_view::ViewEllipsis::End),
            "truncate reached the label — and the two ends keep different halves",
        );

        let checkbox = |side: ViewLabelSide| {
            painted(
                &ViewNode::new(WidgetKind::Checkbox)
                    .prop("label", PropValue::Text("Enable".into()))
                    .prop("label_side", side.into()),
            )
        };
        assert_ne!(
            checkbox(ViewLabelSide::Left),
            checkbox(ViewLabelSide::Right),
            "label side reached the checkbox",
        );
    }


    /// A theme token **nested inside** an object is still a token.
    ///
    /// This is the part that had to be got right: resolution happens per value, at any depth, so a
    /// nested `"accent"` becomes the live theme's accent rather than reaching serde as the literal
    /// word — which would drop the whole field, and drop it silently.
    #[test]
    fn a_token_nested_in_an_object_resolves_against_the_theme() {
        let bordered = |theme: &Theme| {
            let node = ViewNode::new(WidgetKind::Surface).prop(
                "border",
                PropValue::Map(PropMap::from([
                    ("color".into(), PropValue::Color("accent".into())),
                    ("width".into(), PropValue::Float(1.0)),
                ])),
            );
            realize(
                &node,
                theme,
                &noop_emitter(),
                &mut FormBindings::default(),
            )
            .base()
            .style
            .visual
            .border
            .expect("the token resolved")
            .color
        };

        let theme = Theme::default();
        assert_eq!(bordered(&theme), theme.colors.accent, "the token is the theme's accent");

        let mut other = Theme::default();
        other.colors.accent = heca_grid_ui::Color::rgb(0x10, 0xc0, 0x20);
        assert_eq!(bordered(&other), other.colors.accent, "and it follows the theme");
    }

    /// A malformed member costs only itself, at depth too.
    ///
    /// The object keeps its usable fields and the node keeps its other properties — the same
    /// totality rule the flat case has, now that values nest.
    #[test]
    fn a_bad_member_of_an_object_does_not_discard_the_rest() {
        let node = ViewNode::new(WidgetKind::Surface)
            .prop(
                "border",
                PropValue::Map(PropMap::from([
                    ("color".into(), PropValue::Color("not-a-colour".into())),
                    ("width".into(), PropValue::Float(3.0)),
                ])),
            )
            .prop("radius", PropValue::Float(5.0));
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(w.base().style.visual.radius, 5.0, "the neighbouring property still applied");
        assert!(
            w.base().style.visual.border.is_none(),
            "a border with no usable colour is dropped, not fatal",
        );
    }

    /// A bad colour costs only itself — `realize` stays total for untrusted input, so the good
    /// properties on the same node still apply.
    #[test]
    fn a_bad_colour_does_not_discard_its_neighbours() {
        let node = ViewNode::new(WidgetKind::Surface)
            .prop("fill", PropValue::Color("not-a-colour".into()))
            .prop("radius", PropValue::Float(7.0));
        let w = realize(
            &node,
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(w.base().style.visual.radius, 7.0, "the good one still applied");
        assert!(w.base().style.visual.fill.is_none(), "the bad one was dropped, not fatal");
    }

    /// **A described key lands in the very slot a native `.key(..)` writes.** That is the whole of
    /// the declarative half of the identity rule: one slot, so the keyboard cursor, the right-click
    /// target, the drag identity and the picker's remembered letter cannot tell a described row
    /// from a native one.
    #[test]
    fn a_described_key_lands_in_the_widgets_own_slot() {
        let emit: IntentEmitter = Rc::new(|_| {});
        let node = ViewNode::new(WidgetKind::Row)
            .key("pane:7")
            .on_press(Intent::new("focus_pane"));

        let row = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert_eq!(row.base().key.as_deref(), Some("pane:7"));
    }

    /// It is read **generically, for every kind** — not in one arm. A key belongs to no widget in
    /// particular, because a collection can be built from any of them.
    #[test]
    fn every_kind_carries_a_described_key() {
        let emit: IntentEmitter = Rc::new(|_| {});
        for kind in [
            WidgetKind::Row,
            WidgetKind::Item,
            WidgetKind::Button,
            WidgetKind::Card,
            WidgetKind::Label,
            WidgetKind::VStack,
        ] {
            let node = ViewNode::new(kind).key("k");
            let w = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
            assert_eq!(w.base().key.as_deref(), Some("k"), "{kind:?} dropped its key");
        }
    }

    /// A node that declares nothing carries nothing — its identity is derived from its content, and
    /// an empty string would be a name that collides with every other empty one.
    #[test]
    fn a_node_with_no_key_declares_none() {
        let emit: IntentEmitter = Rc::new(|_| {});
        let node = ViewNode::new(WidgetKind::Row).on_press(Intent::new("focus_pane"));
        let row = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert_eq!(row.base().key, None);
    }

    /// `hintable` rides the same generic pass — universal on `Base`, so universal here. Being
    /// pickable is not opt-in, so the only thing a description has to say is "not me".
    #[test]
    fn a_node_can_keep_itself_out_of_the_picker() {
        let emit: IntentEmitter = Rc::new(|_| {});
        let node = ViewNode::new(WidgetKind::Button)
            .text("×")
            .prop("hintable", PropValue::Bool(false))
            .on_press(Intent::new("close"));

        let w = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert!(!w.base().hintable);
        assert!(
            realize(
                &ViewNode::new(WidgetKind::Button).on_press(Intent::new("close")),
                &Theme::default(),
                &emit,
                &mut FormBindings::default(),
            )
            .base()
            .hintable,
            "the default is pickable — a node says only \"not me\"",
        );
    }

    /// **A described `Row` is the interactive widget, not a box.** Click it and its intent fires;
    /// press Enter on it and the same intent fires; it takes one hint target so `prefix+/` reaches
    /// it; and it holds whatever content it was given.
    ///
    /// Before the rename, `WidgetKind::Row` meant `Flex::row()` — a plain box with no focus, no
    /// hover and no activation — so none of this was reachable from a description at all, and
    /// `docs/chrome-and-ui.md` shipped an example that assumed otherwise.
    #[test]
    fn a_described_row_is_clickable_and_keyboard_activatable() {
        use heca_grid_ui::{Event, GridKey, LayoutEngine};
        use heca_core::layout::{Point, Size};
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Row)
            .prop("active", PropValue::Bool(true))
            .on_press(Intent::new("docker.select").arg("id", PropValue::Text("web".into())))
            .child(ViewNode::new(WidgetKind::Label).text("nginx"))
            .child(ViewNode::new(WidgetKind::Badge).text("UP"));

        let mut row = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        LayoutEngine::new().compute(row.as_mut(), Size::new(400.0, 40.0));

        assert_eq!(row.base().children.len(), 2, "it holds its composed content");
        assert!(row.base().focusable, "an actionable row is focusable");
        assert_eq!(hints(row.as_ref()), vec![Vec::<usize>::new()], "one pick target: the row itself");

        let b = row.base().bounds;
        // A click is a press and the release that completes it — pressing and dragging off the row
        // cancels, the way every other control behaves.
        let at = Point::new(b.loc.x + 5.0, b.loc.y + b.size.h / 2.0);
        heca_grid_ui::dispatch(row.as_mut(), &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left));
        heca_grid_ui::dispatch(row.as_mut(), &Event::pointer_released(at, heca_grid_ui::PointerButton::Left));
        // A raw key reaches only the widget that owns the keyboard — a real surface focuses the row
        // before sending one, and an unfocused row taking Enter is what let a card eat the key
        // meant for the list around it.
        heca_grid_ui::reactive::SignalUpdate::set(&row.base_mut().focused, true);
        heca_grid_ui::dispatch(row.as_mut(), &Event::Key { key: GridKey::Enter, pressed: true });

        let fired = fired.borrow();
        assert_eq!(fired.len(), 2, "a click and an Enter each fire it: {fired:?}");
        for intent in fired.iter() {
            assert_eq!(intent.action, "docker.select");
            assert_eq!(intent.args.get("id"), Some(&PropValue::Text("web".into())));
        }
    }

    /// A `Row` with no press intent stays inert — not focusable, nothing for a letter to land on.
    /// A described row that nothing can activate should not pretend to be a control.
    #[test]
    fn a_described_row_without_a_press_intent_is_inert() {
        let node = ViewNode::new(WidgetKind::Row)
            .child(ViewNode::new(WidgetKind::Label).text("just content"));
        let row = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert!(!row.base().focusable);
        assert!(hints(row.as_ref()).is_empty());
        assert_eq!(row.base().children.len(), 1, "it still holds its content");
    }

    /// A still-deferred structured kind (needs track/slot model support — `Grid`, `choice-6`)
    /// realizes to an empty container instead of panicking — the tree stays total for untrusted
    /// plugin/RPC input.
    #[test]
    fn deferred_kind_is_empty_not_panic() {
        let node = ViewNode::new(WidgetKind::Grid);
        let realized = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 0);
        assert!(hints(realized.as_ref()).is_empty(), "an empty fallback declares no hint");
    }

    // ── Options (Choice / Select / Tabs) ──

    /// An option node: a `value` plus composed content.
    fn option_node(value: &str, label: &str) -> ViewNode {
        ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text(value.into()))
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("circle".into())))
            .child(ViewNode::new(WidgetKind::Label).text(label))
    }

    /// A `Select`'s options are its **children** — not a list of strings in a prop — so each one
    /// composes its own content (here an icon + a label), exactly like a native `Choice`.
    #[test]
    fn select_node_realizes_its_choice_children_as_options() {
        let node = ViewNode::new(WidgetKind::Select)
            .prop("selected", PropValue::Int(1))
            .child(option_node("low", "LOW"))
            .child(option_node("high", "HIGH"));

        let select = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let options = &select.base().children;
        assert_eq!(options.len(), 2, "one option per Choice child");
        assert_eq!(
            options[1].base().children.len(),
            2,
            "the option composes its own content: icon + label",
        );
        assert_eq!(
            options[1].text_summary().as_deref(),
            Some("HIGH"),
            "the option is named by the content it composes",
        );
    }

    /// **The point of the whole model.** The widget tracks an index (it is an indexable list), but an
    /// index is meaningless to a plugin and breaks the moment the options are reordered. `realize`
    /// maps it back through the options' `value` props, so the author's intent fires with
    /// `{"value": "high"}`.
    #[test]
    fn the_change_intent_carries_the_chosen_options_value_not_its_index() {
        use heca_grid_ui::{Event, LayoutEngine};
        use heca_core::layout::{Point, Size};
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Select)
            .on("change", Intent::new("set_level"))
            .child(option_node("low", "LOW"))
            .child(option_node("high", "HIGH"));
        let mut select = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        LayoutEngine::new().compute(select.as_mut(), Size::new(400.0, 300.0));

        // Open the dropdown, then click the second option where it actually is (its real bounds).
        let trigger = select.base().bounds;
        heca_grid_ui::dispatch(select.as_mut(), &Event::pointer_pressed(Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0), heca_grid_ui::PointerButton::Left));
        let high = select.base().children[1].base().bounds;
        heca_grid_ui::dispatch(select.as_mut(), &Event::pointer_pressed(Point::new(high.loc.x + 5.0, high.loc.y + high.size.h / 2.0), heca_grid_ui::PointerButton::Left));

        let fired = fired.borrow();
        let [intent] = fired.as_slice() else {
            panic!("expected exactly one intent, got {fired:?}");
        };
        assert_eq!(intent.action, "set_level");
        assert_eq!(
            intent.args.get("value"),
            Some(&PropValue::Text("high".into())),
            "the chosen option's value, not an opaque index",
        );
    }

    /// The same model, and the same value mapping, drives a `Tabs` node — one option primitive, two
    /// consumers.
    #[test]
    fn tabs_node_realizes_choice_children_and_reports_the_chosen_value() {
        use heca_grid_ui::{Event, LayoutEngine, WidgetIntent};
        use heca_core::layout::Size;
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Tabs)
            .on("change", Intent::new("show_tab"))
            .child(option_node("files", "FILES"))
            .child(option_node("issues", "ISSUES"));
        let mut tabs = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        LayoutEngine::new().compute(tabs.as_mut(), Size::new(400.0, 100.0));
        assert_eq!(tabs.base().children.len(), 2, "one tab per Choice child");

        // A described tree is driven by the keyboard exactly like a hand-built one: an intent goes
        // to the focus owner, so the tab strip has to be holding the keyboard to answer one
        // (F004/P084/T400).
        heca_grid_ui::reactive::SignalUpdate::set(&tabs.base_mut().focused, true);
        heca_grid_ui::dispatch(tabs.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
        let fired = fired.borrow();
        let [intent] = fired.as_slice() else {
            panic!("expected exactly one intent, got {fired:?}");
        };
        assert_eq!(intent.action, "show_tab");
        assert_eq!(intent.args.get("value"), Some(&PropValue::Text("issues".into())));
    }

    /// A **childless** `Choice` falls back to the scalar sugar — `text` → one `Label` child, the very
    /// child the composed form would build. Same precedence rule as `Button`: children win.
    #[test]
    fn childless_choice_node_desugars_its_text_to_a_label_child() {
        let node = ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text("high".into()))
            .text("HIGH");
        let choice = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(choice.base().children.len(), 1, "text desugars to a Label child");
        assert_eq!(choice.text_summary().as_deref(), Some("HIGH"));
    }

    /// A stray non-`Choice` child of an option picker is **ignored**, not realized into a broken
    /// option and not a panic: `realize` is total for untrusted plugin/RPC input.
    #[test]
    fn a_non_choice_child_of_a_select_is_ignored() {
        let node = ViewNode::new(WidgetKind::Select)
            .child(option_node("low", "LOW"))
            .child(ViewNode::new(WidgetKind::Button).text("I am not an option"))
            .child(option_node("high", "HIGH"));
        let select = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(
            select.base().children.len(),
            2,
            "only the two Choice children became options",
        );
    }

    // ── Groups (ItemGroup / MarkerGroup) ──

    /// An `ItemGroup` node realizes its header from `text` and its rows from its children — the
    /// widget's own header is `children[0]`, so the rows follow it.
    #[test]
    fn item_group_node_realizes_its_header_and_rows() {
        use heca_grid_ui::LayoutEngine;
        use heca_core::layout::Size;

        let node = ViewNode::new(WidgetKind::ItemGroup)
            .text("EXPLORER")
            .prop("expanded", PropValue::Bool(false))
            .child(ViewNode::new(WidgetKind::Item).text("src"))
            .child(ViewNode::new(WidgetKind::Item).text("tests"));

        let mut group = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        // The group applies its expanded state during layout (`remeasure`), like the native widget.
        LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));
        assert_eq!(
            group.base().children.len(),
            3,
            "the group's own header, then the two realized rows",
        );
        // Collapsed: the rows leave layout (`display: none`), the header stays.
        assert!(!group.base().children[0].base().style.layout.hidden, "the header stays");
        assert!(
            group.base().children[1..]
                .iter()
                .all(|row| row.base().style.layout.hidden),
            "`expanded: false` folds the rows away",
        );
    }

    /// **`toggle`** — the third canonical event name, alongside `press` and `change`. A toggle is
    /// only meaningful with its new state, so the intent carries `args["expanded"]`: an author binds
    /// one action and learns which way it went.
    #[test]
    fn the_toggle_intent_carries_the_new_expanded_state() {
        use heca_grid_ui::{Event, LayoutEngine};
        use heca_core::layout::{Point, Size};
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::ItemGroup)
            .text("EXPLORER")
            .on("toggle", Intent::new("fold_group"))
            .child(ViewNode::new(WidgetKind::Item).text("src"));
        let mut group = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));

        // Click the header (it starts expanded) → it collapses.
        let header = group.base().children[0].base().bounds;
        let at = Point::new(header.loc.x + 5.0, header.loc.y + header.size.h / 2.0);
        heca_grid_ui::dispatch(group.as_mut(), &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left));
        heca_grid_ui::dispatch(group.as_mut(), &Event::pointer_released(at, heca_grid_ui::PointerButton::Left));

        let fired = fired.borrow();
        let [intent] = fired.as_slice() else {
            panic!("expected exactly one intent, got {fired:?}");
        };
        assert_eq!(intent.action, "fold_group");
        assert_eq!(
            intent.args.get("expanded"),
            Some(&PropValue::Bool(false)),
            "the toggle reports the state it moved to",
        );
    }

    /// A `MarkerGroup` is an indicator: bools + children, no events of its own (the rows inside carry
    /// their own intents).
    #[test]
    fn marker_group_node_realizes_its_rows_and_flags() {
        let node = ViewNode::new(WidgetKind::MarkerGroup)
            .prop("active", PropValue::Bool(true))
            .prop("nav_selected", PropValue::Bool(true))
            .child(ViewNode::new(WidgetKind::Item).text("pane 1"))
            .child(ViewNode::new(WidgetKind::Item).text("pane 2"));

        let markers = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(markers.base().children.len(), 2, "the two realized rows");
        assert!(hints(markers.as_ref()).is_empty(), "an indicator is not a pick target");
    }

    // ── Grid (tracks / areas / placement) ──

    /// Tracks are CSS-like strings — the vocabulary plugin authors already know. Anything
    /// unrecognised **degrades to `Auto`**: a typo costs that author a differently-sized track, not
    /// a broken host.
    #[test]
    fn grid_tracks_parse_and_garbage_degrades_to_auto() {
        let text = |s: &str| parse_track(&PropValue::Text(s.into()));
        assert_eq!(text("22px"), Track::Px(22.0));
        assert_eq!(text("1fr"), Track::Fr(1.0));
        assert_eq!(text("2.5fr"), Track::Fr(2.5));
        assert_eq!(text("auto"), Track::Auto);
        assert_eq!(text("min"), Track::MinContent);
        assert_eq!(text("min-content"), Track::MinContent);
        assert_eq!(text("max"), Track::MaxContent);
        assert_eq!(text("max-content"), Track::MaxContent);
        assert_eq!(text("  1FR  "), Track::Fr(1.0), "trimmed + case-insensitive");
        assert_eq!(text("22"), Track::Px(22.0), "a bare number is pixels");
        // Garbage of every shape.
        assert_eq!(text("minmax(1fr, 2fr)"), Track::Auto);
        assert_eq!(text("banana"), Track::Auto);
        assert_eq!(text(""), Track::Auto);
        assert_eq!(parse_track(&PropValue::Bool(true)), Track::Auto);
        assert_eq!(parse_track(&PropValue::Int(22)), Track::Px(22.0));
    }

    /// A `Grid` node: list-shaped tracks + areas on the grid, and **placement as a prop on the
    /// child** — `area` by name, or `col`/`row` (+ spans). A child with neither auto-places.
    #[test]
    fn grid_node_realizes_tracks_areas_and_per_child_placement() {
        let tracks = |t: [&str; 3]| {
            PropValue::List(t.iter().map(|s| PropValue::Text((*s).into())).collect())
        };
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("columns", tracks(["auto", "1fr", "auto"]))
            .prop("rows", PropValue::List(vec![PropValue::Text("auto".into())]))
            .prop(
                "areas",
                PropValue::List(vec![
                    PropValue::Text("icon title status".into()),
                    PropValue::Text("icon subtext .".into()),
                ]),
            )
            // Placed by area name.
            .child(
                ViewNode::new(WidgetKind::Icon)
                    .prop("icon", PropValue::Glyph("terminal".into()))
                    .prop("area", PropValue::Text("icon".into())),
            )
            // Placed by explicit cell + span.
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("zsh")
                    .prop("col", PropValue::Int(2))
                    .prop("row", PropValue::Int(1))
                    .prop("col_span", PropValue::Int(2)),
            )
            // Neither → auto-placed.
            .child(ViewNode::new(WidgetKind::Label).text("~/proj"));

        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let children = &grid.base().children;
        assert_eq!(children.len(), 3);

        // The `icon` area spans both rows of column 1 (it appears twice in the template).
        assert_eq!(
            children[0].base().style.layout.grid_cell,
            Some(heca_grid_ui::GridCell { col: 1, row: 1, col_span: 1, row_span: 2 }),
            "placed into the named area, spanning what the template gives it",
        );
        assert_eq!(
            children[1].base().style.layout.grid_cell,
            Some(heca_grid_ui::GridCell { col: 2, row: 1, col_span: 2, row_span: 1 }),
            "placed by explicit cell; an omitted span defaults to 1",
        );
        assert_eq!(
            children[2].base().style.layout.grid_cell,
            None,
            "no placement props → taffy auto-placement",
        );
    }

    /// Alignment is authorable too, on both axes: `align` / `justify_items` on the grid, and
    /// `align_self` / `justify_self` on any child (they are properties of a node *inside its
    /// parent*, so `realize` reads them for every kind, not just grid items).
    #[test]
    fn grid_alignment_is_authorable_on_both_axes() {
        use heca_grid_ui::Align;
        use heca_view::ViewAlign;

        let node = ViewNode::new(WidgetKind::Grid)
            .prop("align", PropValue::Align(ViewAlign::Center))
            .prop("justify_items", PropValue::Align(ViewAlign::Center))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("pinned")
                    .prop("align_self", PropValue::Align(ViewAlign::End))
                    .prop("justify_self", PropValue::Align(ViewAlign::End)),
            );

        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let style = grid.base().style.layout;
        assert_eq!(style.align, Align::Center, "vertical: the items in their cells");
        assert_eq!(
            style.justify_items,
            Some(Align::Center),
            "horizontal: `justify_items`, not `justify` (which moves the track set)",
        );
        let child = grid.base().children[0].base().style.layout;
        assert_eq!(child.align_self, Some(Align::End));
        assert_eq!(child.justify_self, Some(Align::End));
    }

    /// An unknown area name is not an error — the child simply auto-places (realize stays total).
    #[test]
    fn grid_child_in_an_unknown_area_auto_places() {
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("areas", PropValue::List(vec![PropValue::Text("a b".into())]))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("x")
                    .prop("area", PropValue::Text("nope".into())),
            );
        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(grid.base().children[0].base().style.layout.grid_cell, None);
    }

    /// A `Label`'s text attributes are authorable: weight + slant (font attributes) and underline +
    /// strikethrough (decorations the widget draws). Absent props keep the widget's default.
    #[test]
    fn label_text_attributes_are_authorable() {
        let node = ViewNode::new(WidgetKind::Label)
            .text("DONE")
            .prop("bold", PropValue::Bool(true))
            .prop("italic", PropValue::Bool(true))
            .prop("strikethrough", PropValue::Bool(true));

        let label = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        // Realize hands back a `Box<dyn Component>`, so read the state through the scene: paint it
        // and check the run carries the font attributes and the strike is drawn as a rect.
        use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, TextStyle, Theme};
        use heca_core::layout::Size;
        let mut label = label;
        LayoutEngine::new().compute(label.as_mut(), Size::new(200.0, 40.0));
        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            label.paint(&mut cx);
        }
        let style = scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text(t) => Some(t.style),
                _ => None,
            })
            .expect("the label paints its run");
        assert_eq!(style, TextStyle::REGULAR.bold(true).italic(true));
        let rules = scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(_)))
            .count();
        assert_eq!(rules, 1, "the strikethrough, drawn as a rect (not shaped)");
    }

    // ── Named child slots ──

    /// A child says **where it goes** with a `slot` prop, so a widget with several places for
    /// children needs no change to `ViewNode`'s shape: `children` stays one flat vector.
    ///
    /// `DockFrame` has a **default** slot (the body), so an unslotted child lands there.
    #[test]
    fn dock_frame_routes_its_header_slot_and_defaults_the_rest_to_the_body() {
        let node = ViewNode::new(WidgetKind::DockFrame)
            .text("EXPLORER")
            .prop("frameless", PropValue::Bool(true))
            .child(
                ViewNode::new(WidgetKind::Badge)
                    .text("3")
                    .prop("slot", PropValue::Text("header".into())),
            )
            .child(ViewNode::new(WidgetKind::Item).text("src"))
            .child(ViewNode::new(WidgetKind::Item).text("tests"));

        let dock = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        // The widget's own shape: children[0] = header row [toggle, CONTROLS], children[1] = body.
        let header_controls = &dock.base().children[0].base().children[1];
        assert_eq!(
            header_controls.text_summary().as_deref(),
            Some("3"),
            "the slot=\"header\" child fills the controls slot",
        );
        assert_eq!(
            dock.base().children[1].base().children.len(),
            2,
            "the unslotted children are body content (the body is the default slot)",
        );
    }

    /// `Item` has **no default slot** — its middle is the label, which comes from `text` — so a child
    /// naming no slot (or an unknown one) is ignored rather than dropped somewhere it doesn't belong.
    /// Either way: no panic. Realize stays total for untrusted input.
    #[test]
    fn item_routes_leading_and_trailing_slots_and_ignores_the_rest() {
        let node = ViewNode::new(WidgetKind::Item)
            .text("main.rs")
            .child(
                ViewNode::new(WidgetKind::StatusDot)
                    .prop("slot", PropValue::Text("leading".into())),
            )
            .child(
                ViewNode::new(WidgetKind::Badge)
                    .text("M")
                    .prop("slot", PropValue::Text("trailing".into())),
            )
            .child(ViewNode::new(WidgetKind::Label).text("nowhere")) // no slot → ignored
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("also nowhere")
                    .prop("slot", PropValue::Text("bogus".into())), // unknown → ignored
            );

        let item = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        // The widget's own shape: [LEADING, LABEL, TRAILING].
        let slots = &item.base().children;
        assert_eq!(slots.len(), 3, "the row keeps its three slots — nothing appended");
        assert_eq!(
            slots[2].text_summary().as_deref(),
            Some("M"),
            "the trailing slot holds the badge",
        );
        assert_eq!(
            slots[1].text_summary().as_deref(),
            Some("main.rs"),
            "the label is still the middle — an unslotted child did not overwrite it",
        );
    }

    /// The card's props and its three intents still work — the plain-data path a `ToastSpec` uses.
    #[test]
    fn toast_node_realizes_its_props_and_three_intents() {
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Toast)
            .text("Build failed")
            .prop("severity", PropValue::Text("danger".into()))
            .prop("body_text", PropValue::Text("3 errors in heca-grid-ui".into()))
            .prop("action_text", PropValue::Text("RETRY".into()))
            .on("action", Intent::new("rebuild"))
            .on("dismiss", Intent::new("close_toast"));

        let toast = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert_eq!(toast.base().children.len(), 3, "icon, text column, dismiss");

        // An unknown severity degrades to the widget's default rather than erroring.
        let bogus = ViewNode::new(WidgetKind::Toast)
            .text("x")
            .prop("severity", PropValue::Text("catastrophic".into()));
        assert_eq!(severity_prop(&bogus), heca_grid_ui::ToastSeverity::Info);
        assert_eq!(severity_prop(&node), heca_grid_ui::ToastSeverity::Danger);
    }

    /// The card's text column: `[title, body, actions]`.
    fn toast_column(toast: &dyn Component) -> &[Box<dyn Component>] {
        &toast.base().children[1].base().children
    }

    /// **A described toast has slots again** — `body` (the default) and `actions`.
    ///
    /// F003/P076/T284 recorded "no slots, deliberately" because the card hand-drew itself and could
    /// not hold arbitrary content. That reason died with F003/P082/T481, and this is the reversal
    /// (F003/P096/T488). An unslotted child is the body, so the common case needs no slot name.
    #[test]
    fn a_described_toast_takes_a_body_and_several_actions() {
        use heca_core::layout::Point;
        use heca_grid_ui::Event;
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Toast)
            .text("Build failed")
            // No slot named: the body is the DEFAULT slot.
            .child(ViewNode::new(WidgetKind::Label).text("3 errors"))
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("Retry")
                    .prop("slot", PropValue::Text("actions".into()))
                    .on_press(Intent::new("rebuild")),
            )
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("View log")
                    .prop("slot", PropValue::Text("actions".into()))
                    .on_press(Intent::new("open_log")),
            );

        let toast = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        let column = toast_column(toast.as_ref());
        assert_eq!(
            column[1].text_summary().as_deref(),
            Some("3 errors"),
            "an unslotted child is the body",
        );
        let actions = &column[2].base().children;
        assert_eq!(actions.len(), 2, "two described actions, two controls");

        // Each action fires its OWN intent — a described action is an ordinary described Button,
        // so a real click on it is the whole of the wiring.
        for (i, want) in ["rebuild", "open_log"].iter().enumerate() {
            fired.borrow_mut().clear();
            let mut a = realize(
                &node.children[i + 1],
                &Theme::default(),
                &emit,
                &mut FormBindings::default(),
            );
            heca_grid_ui::LayoutEngine::new()
                .base_font(14.0)
                .compute(a.as_mut(), heca_core::layout::Size::new(200.0, 60.0));
            let b = a.base().bounds;
            let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
            heca_grid_ui::dispatch(
                a.as_mut(),
                &Event::pointer_pressed(at, heca_grid_ui::PointerButton::Left),
            );
            heca_grid_ui::dispatch(
                a.as_mut(),
                &Event::pointer_released(at, heca_grid_ui::PointerButton::Left),
            );
            assert_eq!(
                fired.borrow().first().map(|i| i.action.as_str()),
                Some(*want),
                "action {i} fired the wrong intent",
            );
        }
    }

    /// **Children win over the text sugar** — one content model, two spellings, the way a
    /// `Button`'s children win over its `text`/`icon`.
    #[test]
    fn a_described_toasts_children_win_over_its_text_props() {
        let node = ViewNode::new(WidgetKind::Toast)
            .text("Build failed")
            .prop("body_text", PropValue::Text("the sugar body".into()))
            .prop("action_text", PropValue::Text("SUGAR".into()))
            .on("action", Intent::new("rebuild"))
            .child(ViewNode::new(WidgetKind::Label).text("the composed body"))
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("Composed")
                    .prop("slot", PropValue::Text("actions".into()))
                    .on_press(Intent::new("composed")),
            );

        let toast = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let column = toast_column(toast.as_ref());
        assert_eq!(
            column[1].text_summary().as_deref(),
            Some("the composed body"),
            "the text sugar overwrote a composed body",
        );
        let actions = &column[2].base().children;
        assert_eq!(actions.len(), 1, "action_text built a second control beside the composed one");
        assert_eq!(actions[0].text_summary().as_deref(), Some("Composed"));
    }

    /// **An unknown slot name is logged and falls back to the body** — `realize` is total for
    /// untrusted input, so a plugin's typo costs it a misplaced child, never a panic.
    #[test]
    fn a_described_toasts_unknown_slot_falls_back_to_the_body() {
        let node = ViewNode::new(WidgetKind::Toast).text("t").child(
            ViewNode::new(WidgetKind::Label)
                .text("typo")
                .prop("slot", PropValue::Text("bodyy".into())),
        );
        let toast = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(
            toast_column(toast.as_ref())[1].text_summary().as_deref(),
            Some("typo"),
            "an unknown slot should fall back to the body, not vanish",
        );
    }

    // ── Vocabulary coverage (the guard that keeps this from rotting) ──

    /// A representative node for `kind` — enough of one that a correct arm produces a *live* widget.
    ///
    /// The match is **exhaustive on purpose**: adding a `WidgetKind` without adding an arm to
    /// `realize` cannot compile past this point, so the coverage test below fires instead of the new
    /// kind silently rendering an empty container. That failure mode is exactly what this guards —
    /// it is invisible in review and invisible at runtime.
    fn sample_node(kind: WidgetKind) -> ViewNode {
        let node = ViewNode::new(kind);
        match kind {
            // Containers: give them a child, which a correct arm attaches.
            WidgetKind::VStack
            | WidgetKind::HStack
            | WidgetKind::CardGrid
            | WidgetKind::Card
            | WidgetKind::Scroll
            | WidgetKind::Panel
            | WidgetKind::Surface
            | WidgetKind::Grid
            | WidgetKind::MarkerGroup
            | WidgetKind::ItemGroup
            | WidgetKind::DockFrame
            | WidgetKind::Overlay => node
                .text("TITLE")
                .child(ViewNode::new(WidgetKind::Label).text("child")),

            // Option pickers: their children are `Choice` nodes.
            WidgetKind::Select | WidgetKind::Tabs => node.child(
                ViewNode::new(WidgetKind::Choice)
                    .prop("value", PropValue::Text("a".into()))
                    .text("A"),
            ),
            WidgetKind::Choice => node.prop("value", PropValue::Text("a".into())).text("A"),

            // The interactive row: a container that is also a control, so it needs both a child
            // and a press intent — without the intent it is deliberately inert (no focus, no
            // hover), which would read as "realized to nothing" here.
            WidgetKind::Row => node
                .child(ViewNode::new(WidgetKind::Label).text("row"))
                .on_press(Intent::new("noop")),

            // A slotted row: the label is a prop, the slots are children.
            WidgetKind::Item => node.text("row").child(
                ViewNode::new(WidgetKind::StatusDot)
                    .prop("slot", PropValue::Text("leading".into())),
            ),

            // Leaves that carry text.
            WidgetKind::Label
            | WidgetKind::Button
            | WidgetKind::Badge
            | WidgetKind::BadgeButton
            | WidgetKind::Tag
            | WidgetKind::Alert
            | WidgetKind::Toast
            | WidgetKind::Input
            | WidgetKind::Checkbox => node.text("TEXT"),

            // Leaves that carry a glyph.
            WidgetKind::Icon | WidgetKind::IconButton | WidgetKind::RailCell => {
                node.prop("icon", PropValue::Glyph("terminal".into()))
            }

            // Leaves with their own state.
            WidgetKind::Toggle => node.prop("on", PropValue::Bool(true)),
            WidgetKind::Gauge => node.prop("value", PropValue::Float(0.5)),
            WidgetKind::Progress => node.prop("value", PropValue::Float(0.5)),
            WidgetKind::NfIcon => node.prop("glyph", PropValue::Text("command".into())),
            // Its children are buttons and nothing else, and each needs the pair the group reads:
            // the words for its menu row and hover bubble, the icon for when there is no room.
            WidgetKind::ButtonGroup => node.child(
                ViewNode::new(WidgetKind::Button)
                    .text("Close")
                    .prop("icon", PropValue::Glyph("close".into()))
                    .on_press(Intent::new("noop")),
            ),
            // It has no properties at all: it paints from the clock, and a root node has no
            // container to take a size from, so give it the one every other leaf gets implicitly.
            WidgetKind::Spinner => node
                .prop("width", PropValue::Float(28.0))
                .prop("height", PropValue::Float(28.0)),
            WidgetKind::StatusDot => node,

            // A rule normally stretches to its container; as a root it has none, so give it a
            // span — and take the chance to drive both of its properties, in the order that would
            // have been wrong before the widget started recomputing from the pair.
            WidgetKind::Separator => node
                .prop("length", PropValue::Float(120.0))
                .prop("orientation", PropValue::Text("vertical".into())),

            // A picker: its children are what it letters, and one of them must be pickable for
            // the picker to be worth anything — so the sample carries a hint, not a press.
            WidgetKind::KeyHintGroup => node
                .prop("opens_on", PropValue::Text("sample.pick".into()))
                .child(ViewNode::new(WidgetKind::Label).text("target").on_hint(Intent::new("noop"))),

            // Host-only — see the coverage test.
            WidgetKind::ScrollBar => node,
        }
    }

    /// **Every widget in the library can be described, or somebody said why not**
    /// (F003/P097/T501, C11).
    ///
    /// The test below asks the vocabulary is *complete* — every kind realizes. This asks the other
    /// direction, which is the one that actually failed: at the start of T501 **thirteen** library
    /// widgets had no declarative form, and nothing anywhere said so. A plugin author found out by
    /// not finding one. The census that discovered them was a person reading two crates by hand,
    /// and a census run once is a census that is wrong a month later.
    ///
    /// So the list is derived from the library's own exports, and every widget must be one of:
    ///
    /// - a [`WidgetKind`] of the same name — it can be described;
    /// - a **declaration** on any node rather than a thing you place (a tooltip, a menu, a hint
    ///   letter, visibility) — the wrapper survives for regions that are not widgets, but nobody
    ///   needs to name it to get the behaviour;
    /// - **host-only by design**, with the reason.
    ///
    /// Anything else is a widget a description cannot ask for, and this says so by name.
    #[test]
    fn every_library_widget_is_describable_or_deliberately_not() {
        /// Widgets with no `WidgetKind`, each with why. An entry is a deliberate decision, not a
        /// place to park an omission — read the three categories in the doc above before adding one.
        const NO_KIND: &[(&str, &str)] = &[
            // ── Declarations: the behaviour is on every node, so nothing needs naming ──
            (
                "Tooltip",
                "a declaration on any node; the wrapper is for regions",
            ),
            (
                "ContextMenu",
                "a declaration on any node, never a thing you place",
            ),
            ("KeyHint", "a declaration on any node: hint + placement"),
            (
                "Visibility",
                "two declarations on any node: visible, hidden",
            ),
            // ── Host-only by design ──
            (
                "ChromeRegion",
                "host-only: a region of the window, which the host owns",
            ),
            ("Pane", "host-only: the app's own concept, not a primitive"),
            ("PaneFrame", "host-only: the frame of the above"),
            (
                "FocusScope",
                "host-only: focus containment the host arranges",
            ),
            (
                "ScrollBar",
                "host-only: live host signals; realize refuses it",
            ),
            ("ToastStack", "host-only: it holds a queue the host drives"),
            // ── Not widgets: parts, values and helpers that happen to be exported ──
            (
                "GridCell",
                "not a widget: a CardGrid cell, built by realize",
            ),
            ("Menu", "not a widget: the data a menu declaration carries"),
            ("MenuItem", "not a widget: one entry of the above"),
            ("MenuEntry", "not a widget: one entry of the above"),
            ("MenuAnchor", "not a widget: where a menu opens"),
            ("Command", "not a widget: one entry of a command palette"),
            ("Tip", "not a widget: what a tooltip declaration carries"),
            ("Flex", "the arrangement: VStack/HStack are its names"),
            ("Container", "not a widget: a helper for building a `Flex`"),
            ("ScrollRegion", "described as `Scroll`"),
            ("ProgressBar", "described as `Progress`"),
            // ── Opened through the host API, not placed in a tree ──
            //
            // `docs/chrome-and-ui.md` § 3.5 is the agreed plugin contract: a plugin opens these
            // with `app.overlay.openModal(..)` / `openDropdown(..)` and fires actions with
            // `app.actions.dispatch(..)`. They are not kinds a plugin constructs — it would then
            // own the scrim, the open state and the dismissal, and get the chosen button back
            // without the form data beside it. `Dialog` stays a widget **native** code composes.
            ("Dialog", "opened via app.overlay.openModal (§3.5)"),
            ("CommandPalette", "opened via the host API (§3.5)"),
        ];

        let exports = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../heca-grid-ui/src/widgets/mod.rs"),
        )
        .expect("the widget exports are where they are expected");

        // Every type name a `pub use` line publishes. Types, not values: a widget is a type, and
        // the lower-case items on these lines are constructors and constants.
        let published: std::collections::BTreeSet<String> = exports
            .lines()
            .filter(|l| l.trim_start().starts_with("pub use"))
            .flat_map(|l| {
                l.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .skip(1)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|n| n.chars().next().is_some_and(char::is_uppercase))
            .collect();
        assert!(
            published.len() > 30,
            "this guard has stopped reading the library's exports, which is worse than the gap it \
             exists for — it found only {}",
            published.len(),
        );

        // The value types (`ButtonVariant`, `Glyph`, `Display`, …) are vocabularies a widget reads,
        // not widgets. They are told apart by being named in a kind's properties rather than by a
        // list here, which would be one more thing to keep in step.
        let kinds: std::collections::BTreeSet<String> =
            WidgetKind::ALL.iter().map(|k| format!("{k:?}")).collect();

        let unreachable: Vec<&String> = published
            .iter()
            .filter(|n| !kinds.contains(*n))
            .filter(|n| !NO_KIND.iter().any(|(w, _)| w == *n))
            .filter(|n| !VALUE_TYPES.contains(&n.as_str()))
            .collect();
        assert!(
            unreachable.is_empty(),
            "these library widgets have no `WidgetKind` and no recorded reason, so a plugin can \
             neither name them nor find out why: {unreachable:#?}\n\nAdd a kind, or add the name \
             to NO_KIND with which of the three categories it is in.",
        );
    }

    /// Vocabularies a widget reads — enums and value types, not widgets. Listed once because two
    /// guards ask the same question of the same exports.
    const VALUE_TYPES: &[&str] = &[
        "AlertVariant",
        "BadgeVariant",
        "ButtonVariant",
        "LabelSide",
        "RegionMode",
        "Glyph",
        "NfGlyph",
        "ActiveMarker",
        "Ellipsis",
        "Orientation",
        "DotStatus",
        "RevealAlign",
        "ScrollAxes",
        "ScrollInfo",
        "ToastAction",
        "ToastPosition",
        "ToastSeverity",
        "ToastSpec",
        "Display",
        "TooltipSide",
        "HintPlacement",
        "KeyCap",
        "KeycapVariant",
        "HintStyle",
        "DEFAULT_LETTERS",
        // The dialog panel recipe — spacing steps every dialog-shaped surface reads, so a
        // described one is laid out identically to a native one (F003/P097/T502).
        "DIALOG_PAD",
        "DIALOG_GAP",
        "DIALOG_BTN_GAP",
        "NamedAnimation",
    ];

    /// **Every `WidgetKind` realizes to a live widget** — one that either holds the children it was
    /// given or paints something. The fallback (an empty `Flex`) does neither, so a kind with no arm
    /// fails here loudly instead of rendering nothing and being noticed months later by a plugin
    /// author.
    ///
    /// The single exception is `ScrollBar`, which is **host-only by design**: its state is live host
    /// signals (`content_extent` / `viewport_extent` / `offset`), and static serializable data
    /// fundamentally cannot drive a signal — a declarative one would render a dead control. The test
    /// asserts it realizes to *nothing*, so that decision is pinned rather than merely documented.
    #[test]
    fn every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones() {
        use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};
        use heca_core::layout::Size;

        let theme = Theme::default();
        for &kind in WidgetKind::ALL {
            let mut widget = realize(
                &sample_node(kind),
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            LayoutEngine::new().compute(widget.as_mut(), Size::new(400.0, 200.0));
            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                widget.paint(&mut cx);
            }
            let alive = !widget.base().children.is_empty() || !scene.is_empty();

            if kind == WidgetKind::ScrollBar {
                assert!(
                    !alive,
                    "ScrollBar is host-only: it must NOT realize (its state is a live host signal — \
                     a plugin uses Scroll instead)",
                );
            } else {
                assert!(
                    alive,
                    "{kind:?} realized to nothing — it needs a `realize` arm (or, if its state is a \
                     live host signal, to be documented as host-only like ScrollBar)",
                );
            }
        }
    }

    /// **What a plugin actually writes** — the SDK, end to end (F003/P082/T435).
    ///
    /// A plugin cannot hand us a Rust function, so without a declarative spelling it can draw a row
    /// and never make that row pickable. The JSON is not a convenience, it is the feature.
    ///
    /// The kind here is a `Label` on purpose: nothing about it is clickable, `realize` wires no
    /// `press` for it, and it is still a perfectly good thing to point at — which is why a hint is
    /// enough on its own to make a node a target.
    #[test]
    fn a_plugin_can_make_anything_pickable_from_the_sdk() {
        use heca_view::build;

        let (emit, fired) = recording_emitter();
        let node: ViewNode = build::Label::new("nginx")
            .on_hint(Intent::new("docker.reveal").arg("id", PropValue::Text("abc".into())))
            .into();

        let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert_eq!(hints(widget.as_ref()), vec![Vec::<usize>::new()], "the label is a target");
        assert!(heca_grid_ui::fire_hint(widget.as_mut(), &[]));
        assert_eq!(
            fired.borrow()[0].args.get("id"),
            Some(&PropValue::Text("abc".into())),
            "and picking it fires the plugin's own intent, arguments and all",
        );
    }

    /// **Nothing paints outside the box it was given** — every kind, at every width
    /// (F003/P082/T438).
    ///
    /// The rule `components/mod.rs` already states for anything sized by its container — *lay it out
    /// in a box and assert it never exceeds it* — asked of the **whole vocabulary** rather than one
    /// composition at a time. It walks `WidgetKind::ALL`, so a kind added next month is covered
    /// with nobody remembering, and a plugin's tree is covered by construction: it is built from
    /// these kinds.
    ///
    /// **The clip stack and the overlay band are the framework's answer**, not this test's:
    /// `Scene::draws_outside` honours the clips in force and reads the base layer only, because
    /// the overlay band exists precisely for what must escape its box — a dropdown opened inside a
    /// scroll region, a hint keycap on a half-visible row. This sweep and the showcase catalog's
    /// asked the same question with two copies of that arithmetic, and both copies were wrong the
    /// same way (F003/P082/T481).
    ///
    /// The failure it exists for is invisible to every "was this drawn?" assertion: a name that is
    /// drawn *somewhere*, across its neighbour.
    #[test]
    fn no_kind_paints_outside_the_box_it_is_given() {
        use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};
        use heca_core::layout::{Point, Rectangle, Size};

        let theme = Theme::default();
        let mut escapes: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            // Down to 32px. **A box is never narrower than its own padding** — that is the box
            // model, not a defect: a card padded 13 a side has a 26px floor, and asking it to fit
            // in 24 is asking it to have no box at all. Every escape this sweep exists for still
            // shows at 32 — all five of the ones it was carrying did — so the floor costs it
            // nothing (F003/P082/T481).
            for box_w in [400.0f64, 120.0, 60.0, 32.0] {
                // A parent that hands it a definite width: a kind sized as a share has nothing to
                // be a share *of* at the root of a layout.
                let node = ViewNode::new(WidgetKind::VStack).child(sample_node(kind));
                let mut root = realize(
                    &node,
                    &theme,
                    &noop_emitter(),
                    &mut FormBindings::default(),
                );
                root.base_mut().style.layout.width = heca_grid_ui::Length::Px(box_w as f32);
                LayoutEngine::new().compute(root.as_mut(), Size::new(box_w, 200.0));

                // **Do not ask a box to hold a control in less room than a control needs.** A card
                // pads itself 13 a side, so at 32px it has six pixels of content space — narrower
                // than the smallest thing that can stand in it (a glyph plus its padding). Below
                // that, "keep your content inside" is not a defect report, it is the box model:
                // something has to be cut, and which one is a design decision, not this sweep's.
                // The kinds that escape all do so at widths where they *did* have room
                // (F003/P096/T483).
                // The kind itself, not the wrapper that hands it a definite width.
                let pad = root
                    .base()
                    .children
                    .first()
                    .map_or(0.0, |kind| kind.base().style.layout.padding as f64);
                let one_control = (theme.font_size * 2.0) as f64;
                if box_w - 2.0 * pad < one_control {
                    continue;
                }

                let mut scene = Scene::new();
                {
                    let mut cx = PaintCx::new(&mut scene, &theme);
                    root.paint(&mut cx);
                }
                let box_ = Rectangle::new(Point::default(), Size::new(box_w, 200.0));
                if let Some(escape) = scene.draws_outside(box_).first() {
                    escapes.push(format!("{kind:?} at {box_w}px: {escape}"));
                }
            }
        }
        assert!(escapes.is_empty(), "these draw outside their box:\n{}", escapes.join("\n"));
    }

    /// **A widget with content never renders as nothing** — every kind, with room to spare.
    ///
    /// The other half, and the one that caught the first attempt at fixing the first: a label that
    /// may shrink to zero *does*, in a parent that sizes to its minimum, and a `Card`'s title
    /// vanished outright. "Nothing drawn" is a worse answer than "drawn too wide", and no test
    /// anywhere asserted against it.
    #[test]
    fn no_kind_with_content_renders_as_nothing() {
        use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};
        use heca_core::layout::Size;

        let theme = Theme::default();
        let mut silent: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            if kind == WidgetKind::ScrollBar {
                continue; // host-only: `realize` refuses it outright, by design
            }
            if kind == WidgetKind::Overlay {
                // A **closed** surface draws nothing, and a described `Overlay` starts closed
                // unless it says `opened`. That is the widget working, not a silent one.
                continue;
            }
            let node = ViewNode::new(WidgetKind::VStack).child(sample_node(kind));
            let mut root = realize(&node, &theme, &noop_emitter(), &mut FormBindings::default());
            root.base_mut().style.layout.width = heca_grid_ui::Length::Px(400.0);
            LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 200.0));

            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                root.paint(&mut cx);
            }
            if scene.is_empty() {
                silent.push(format!("{kind:?}"));
            }
        }
        assert!(
            silent.is_empty(),
            "these draw nothing at all despite having content:\n{}",
            silent.join("\n"),
        );
    }

    /// **What a plugin actually writes to own a picker** — the SDK, end to end (F003/P082/T436).
    ///
    /// The last of the six. After T435 a plugin could be *picked*; this is the other half — opening
    /// a picker of its own over its own panel, which heca's exposé has had since T427. Everything
    /// here is a string in the plugin's own tree plus a key in the user's own config: no registry,
    /// no id to hold, no signal, no closure.
    #[test]
    fn a_plugin_can_own_a_picker_from_the_sdk() {
        use heca_view::build;
        use heca_view::build::Parent as _;

        let (emit, fired) = recording_emitter();
        let node: ViewNode = build::KeyHintGroup::new()
            .opens_on("mypanel.pick")
            .child(build::Row::new().on_hint(Intent::new("docker.restart")))
            .into();

        let mut picker = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());

        // 1. The verb is on screen, so the host's `[[keys.surface]] pick = "s"` can reach it —
        //    by name, with no path to go stale when the tree is rebuilt.
        assert_eq!(
            heca_grid_ui::collect_actions(picker.as_ref()),
            vec!["mypanel.pick".to_string()],
        );
        assert!(!heca_grid_ui::fire_action(picker.as_ref(), "heca.expose.pick"), "its own name only");

        // 2. Running it opens the picker and letters what is beneath it.
        assert!(heca_grid_ui::fire_action(picker.as_ref(), "mypanel.pick"));
        picker.tick(0.0);
        let row = &picker.base().children[0];
        assert_eq!(
            row.base().hint_label.get_untracked().as_deref(),
            Some("a"),
            "the plugin's own row wears the letter, and draws it itself",
        );

        // 3. And the letter runs the row's own intent, back out to the plugin.
        picker.on_event_capture(&heca_grid_ui::Event::TextInput("a".to_string()));
        assert_eq!(
            fired.borrow().iter().map(|i| i.action.clone()).collect::<Vec<_>>(),
            vec!["docker.restart"],
        );
    }

    /// **A described node declares verbs by name**, on any kind (F003/P082/T436).
    ///
    /// `ComponentExt::on_action` is universal natively, so this is read once for every kind rather
    /// than wired per arm. It is the seam a **surface** has and a dock got from `Provider::actions`:
    /// without it a described panel can only bind verbs the app already compiled in.
    #[test]
    fn a_described_node_answers_to_the_verb_it_declares() {
        use heca_view::build;
        use heca_view::build::Style as _;

        let (emit, fired) = recording_emitter();
        let node: ViewNode = build::Panel::new()
            .on_action("mypanel.reload", Intent::new("docker.refresh"))
            .into();

        let panel = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        assert!(heca_grid_ui::fire_action(panel.as_ref(), "mypanel.reload"));
        assert_eq!(
            fired.borrow().iter().map(|i| i.action.clone()).collect::<Vec<_>>(),
            vec!["docker.refresh"],
            "the verb the surface named fired the intent it was bound to",
        );
    }

    /// **A verb that names itself never runs.** The router answers a name by looking for a widget
    /// on screen declaring it, so `"mypanel.pick" -> Intent("mypanel.pick")` would find this widget
    /// again and re-post itself forever — a description spinning the event loop.
    ///
    /// Refused where both names are known, which is here. `realize` is total for untrusted input:
    /// the verb is simply not declared, and resolves to nothing.
    #[test]
    fn a_verb_that_fires_its_own_name_is_refused() {
        use heca_view::build;
        use heca_view::build::Style as _;

        let node: ViewNode = build::Panel::new()
            .on_action("mypanel.reload", Intent::new("mypanel.reload"))
            .into();

        let panel = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert!(
            heca_grid_ui::collect_actions(panel.as_ref()).is_empty(),
            "a self-naming verb is not declared at all, so nothing can reach it",
        );
    }

    /// **T432's rule holds for a described tree too**: a pick acts on the widget it named and on
    /// nothing else, so a plugin's nested pickable rows behave exactly like heca's own.
    ///
    /// Without it a plugin composing a pickable card out of pickable rows would fire both and land
    /// the user on the card — and would have to hand-write the DOM's `e.target !== e.currentTarget`
    /// guard, which it has no way to express at all.
    #[test]
    fn a_described_pick_lands_on_the_node_it_named_and_not_its_container() {
        use heca_view::build;

        let (emit, fired) = recording_emitter();
        use heca_view::build::Parent as _;
        let node: ViewNode = build::Card::new("Containers")
            .on_hint(Intent::new("the_card"))
            .child(build::Label::new("row").on_hint(Intent::new("the_row")))
            .into();

        let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
        let inner = hints(widget.as_ref())
            .into_iter()
            .find(|p| !p.is_empty())
            .expect("the nested label is its own target");
        assert!(heca_grid_ui::fire_hint(widget.as_mut(), &inner));
        assert_eq!(
            fired.borrow().iter().map(|i| i.action.clone()).collect::<Vec<_>>(),
            vec!["the_row"],
            "the card it sits in must not answer for it",
        );
    }

    /// **A menu declared on ANY kind reaches the node itself** (F003/P097/T501).
    ///
    /// A menu is a declaration, not a widget an author assembles — natively it is one builder on
    /// any widget, so the described form is one field on any node. Until this, a plugin building
    /// its UI as a described tree could not attach a menu to its own row at all, while heca's own
    /// rows did it in a line: a second-class version of a shipped feature, which ⭐⭐ RULE ZERO
    /// exists to forbid.
    ///
    /// It asserts the declaration lands **on the node**, never on a wrapper around it: a wrapper is
    /// a widget the description never asked for, and the right-click walk would find it instead of
    /// the row.
    #[test]
    fn a_menu_declared_on_any_kind_lands_on_the_node_itself() {
        let mut missing: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            let (emit, _fired) = recording_emitter();
            let node = sample_node(kind).menu([heca_view::DropdownItem::with_intent(
                "close",
                "Close",
                Intent::new("plugin.close"),
            )]);
            let widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
            if widget.base().context_menu.is_none() {
                missing.push(format!("{kind:?}"));
            }
        }
        assert!(
            missing.is_empty(),
            "a menu declared on these kinds never reached the widget, so a described row cannot \
             have one while a native row can: {missing:#?}",
        );
    }

    /// **Every universal capability is reachable from the typed SDK** (F003/P097/T501, C3).
    ///
    /// [`every_widget_property_is_reachable_from_the_sdk`] walks each *kind's* generated
    /// `PROP_NAMES`, and that is the whole of its reach. A capability that belongs to no particular
    /// widget — `tooltip`, `hintable`, the ones that come next — lives on `ComponentExt`, a plain
    /// trait with no generated property surface, so it appears in no kind's `PROP_NAMES` and that
    /// guard cannot see it. `tooltip` was undeclarable for exactly as long as that was true, with
    /// nothing failing anywhere.
    ///
    /// So this asks the other half of the question, and **fails closed** in the same way: every
    /// `#[prop]` builder on `ComponentExt` must have a setter of the same name in the SDK, or be
    /// named below with a reason. It reads both sources rather than calling them, because *"does a
    /// method exist"* is not a question a running test can ask.
    #[test]
    fn every_universal_capability_is_reachable_from_the_sdk() {
        /// Universal builders with no SDK spelling, each with the reason. An entry here is a
        /// capability a described tree cannot use — keep it empty if you can.
        const NOT_IN_SDK: &[(&str, &str)] = &[];

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let native = std::fs::read_to_string(dir.join("../heca-grid-ui/src/builders.rs"))
            .expect("the native builder source is where it is expected");
        let sdk = std::fs::read_to_string(dir.join("../heca-view/src/build.rs"))
            .expect("the SDK source is where it is expected");

        // `ComponentExt` is the trait a capability lands on when it belongs to every widget rather
        // than to one, so it is the list this guard is about.
        let head = "\npub trait ComponentExt: Component + Sized {\n";
        let start = native
            .find(head)
            .expect("ComponentExt is where it is expected")
            + head.len();
        let body = &native[start..start + native[start..].find("\n}\n").unwrap_or(0)];

        let universal: Vec<&str> = body
            .split("#[heca_grid_ui_macros::prop]")
            .skip(1)
            .filter_map(|after| {
                let f = after.find("fn ")? + 3;
                let rest = &after[f..];
                Some(&rest[..rest.find(['(', '<'])?])
            })
            .collect();
        assert!(
            !universal.is_empty(),
            "no `#[prop]` builders found on ComponentExt — this guard has stopped reading anything, \
             which is worse than the gap it exists for",
        );

        let missing: Vec<&str> = universal
            .iter()
            .filter(|name| !NOT_IN_SDK.iter().any(|(n, _)| n == *name))
            .filter(|name| !sdk.contains(&format!("fn {name}(")))
            .copied()
            .collect();
        assert!(
            missing.is_empty(),
            "these capabilities are on every widget natively and cannot be said in a description at \
             all, so a plugin can draw a row and never give it what heca's own rows have: {missing:#?}",
        );
    }

    /// **Hiding the ink and collapsing the box are two things, and both are declarable**
    /// (F003/P097/T501, C5).
    ///
    /// There is no `Visibility` widget kind and there must not be one, because the wrapper's whole
    /// job is one of these two properties — and a wrapper cannot be put around a widget a typed
    /// container holds, nor expressed in a description at all.
    ///
    /// The pair is CSS's, deliberately: `hidden` is `display: none` (out of the layout, neighbours
    /// close up) and `visible` is `visibility: hidden` (ink gone, box kept). Confusing them is the
    /// whole trap — a row of four status slots showing one at a time stays still only under the
    /// second, and `hidden` was already reachable while `visible` was reachable from neither
    /// authoring path, which is exactly the sort of half-capability that reads as working.
    ///
    /// So this asks both, on every kind, and asks that each leaves the *other* alone.
    #[test]
    fn hiding_the_ink_and_collapsing_the_box_are_separately_declarable_on_any_kind() {
        use heca_grid_ui::reactive::SignalGet as _;

        let mut wrong: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            let (emit, _fired) = recording_emitter();
            let realized = |node: &ViewNode| {
                realize(node, &Theme::default(), &emit, &mut FormBindings::default())
            };

            // `visible: false` — the ink goes, the box stays.
            let w = realized(&sample_node(kind).prop("visible", PropValue::Bool(false)));
            if w.base().visible.get_untracked() {
                wrong.push(format!("{kind:?} (`visible: false`, still draws)"));
            }
            if w.base().is_hidden() {
                wrong.push(format!("{kind:?} (`visible` collapsed the box)"));
            }

            // `hidden: true` — out of the layout entirely, and the ink flag untouched.
            let h = realized(&sample_node(kind).prop("hidden", PropValue::Bool(true)));
            if !h.base().is_hidden() {
                wrong.push(format!("{kind:?} (`hidden`, still takes space)"));
            }
            if !h.base().visible.get_untracked() {
                wrong.push(format!("{kind:?} (`hidden` cleared `visible`)"));
            }

            // Nothing declared changes neither.
            let plain = realized(&sample_node(kind));
            if !plain.base().visible.get_untracked() || plain.base().is_hidden() {
                wrong.push(format!("{kind:?} (declared neither, not shown)"));
            }
        }
        assert!(
            wrong.is_empty(),
            "a described node cannot say what it shows on these kinds, or the two ways of not \
             showing have been collapsed into one: {wrong:#?}",
        );
    }

    /// **A described action row is the same row heca's own chrome uses** (F003/P097/T501, C9).
    ///
    /// The pane header's action row is a `ButtonGroup`, and a whole session went into making it
    /// behave: words become icons as the room runs out, and whatever still does not fit collapses
    /// into a ⋮ that runs the same actions. A plugin could not ask for any of it — it would have
    /// hand-built a row of buttons and got the collapse, the overflow menu and the hover words
    /// approximately right, which is the second path this project forbids.
    ///
    /// What it asks is that the **buttons are real buttons**: the group reads a label, a glyph and
    /// a click out of each child to build that menu, so a described action must arrive with all
    /// three or the menu row it produces is blank and does nothing. Building them through anything
    /// but the one button path is how that would quietly happen.
    #[test]
    fn a_described_action_row_carries_its_actions_into_the_overflow_menu() {
        let (emit, fired) = recording_emitter();
        let action = |name: &str, icon: &str| {
            ViewNode::new(WidgetKind::Button)
                .text(name)
                .prop("icon", PropValue::Glyph(icon.into()))
                .on_press(Intent::new(format!("pane.{}", name.to_lowercase())))
        };
        let node = ViewNode::new(WidgetKind::ButtonGroup)
            .prop("display", PropValue::Text("icon_only".into()))
            .child(action("Close", "close"))
            .child(action("Split", "square_split_horizontal"))
            // Not a button: the group has no label, glyph or click to read out of it, so it is
            // skipped rather than wrapped into an action that does nothing.
            .child(ViewNode::new(WidgetKind::Label).text("not an action"));

        let mut group = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        assert_eq!(
            group.base().children.len(),
            3,
            "two actions and the ⋮ the group builds for itself — the label is not an action",
        );

        // **Asked through the picker**, not by reaching into the group. The group wraps each button
        // in its own arrangement, so an index into its children is a fact about today's internals;
        // `prefix+/` is the contract, and it is also how a user reaches a collapsed action.
        let targets = hints(group.as_ref());
        assert_eq!(
            targets.len(),
            3,
            "each described action is pickable in its own right, plus the ⋮ that reaches the ones \
             that did not fit — and the label, which is not an action, is not among them",
        );
        assert!(heca_grid_ui::fire_hint(group.as_mut(), &targets[0]));
        assert_eq!(
            fired.borrow().first().map(|i| i.action.clone()),
            Some("pane.close".to_string()),
            "a grouped action runs the intent the description gave it — which is what makes its \
             overflow row run the plugin's own action rather than nothing",
        );
    }

    /// **Where a letter sits is declarable on ANY kind** (F003/P097/T501, C4).
    ///
    /// `Base::hint_style` was universal from the day the picker stopped drawing every cap the same
    /// way — but the only builders that wrote it were on [`KeyHint`], the wrapper. So moving a
    /// letter meant wrapping the widget, which is the wrapper rule living in every caller's
    /// discipline, is impossible on a widget a typed container holds, and could not be said in a
    /// description at all. A plugin could make its row pickable and then had to accept whatever
    /// position heca chose for it.
    ///
    /// **Placement is not what makes a node pickable** — anything actionable already wears a letter
    /// with nothing declared — so this asks only that the four knobs arrive, on every kind.
    #[test]
    fn where_a_letter_sits_is_declarable_on_any_kind() {
        let mut missing: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            let (emit, _fired) = recording_emitter();
            let node = sample_node(kind)
                .prop("hint_placement", PropValue::Text("center_right".into()))
                .prop("hint_size", PropValue::Int(18))
                .prop("hint_offset_y", PropValue::Float(4.0))
                .prop("hint_color", PropValue::Color("danger".into()));
            let theme = Theme::default();
            let widget = realize(&node, &theme, &emit, &mut FormBindings::default());
            let style = widget.base().hint_style;
            if style.placement != heca_grid_ui::widgets::HintPlacement::CenterRight {
                missing.push(format!("{kind:?} (placement ignored)"));
            }
            // An integer size is the same number of pixels as a fraction to whoever wrote it.
            if style.size != Some(18.0) {
                missing.push(format!("{kind:?} (size ignored: {:?})", style.size));
            }
            if (style.offset_y - 4.0).abs() > f64::EPSILON {
                missing.push(format!("{kind:?} (offset ignored: {})", style.offset_y));
            }
            // A **token name**, resolved against the live theme — never a hex literal written into
            // the description, so a letter follows a theme change with nothing rewritten.
            let danger: Option<heca_grid_ui::Color> =
                resolve_color("danger", &theme).and_then(|hex| hex.parse().ok());
            if style.color != danger {
                missing.push(format!(
                    "{kind:?} (colour token unresolved: {:?})",
                    style.color
                ));
            }
        }
        assert!(
            missing.is_empty(),
            "a described node cannot say where its letter goes on these kinds, while a native one \
             can: {missing:#?}",
        );
    }

    /// **A described tree can space itself from the theme, without a pixel** (F003/P097/T502).
    ///
    /// The library has had semantic spacing all along — steps resolved from the inherited font at
    /// layout, so padding follows a font or theme change with nothing rewritten. The **described**
    /// side had only raw px, so a plugin author had no way *not* to hardcode: the rule this project
    /// states everywhere was one a plugin could not keep.
    ///
    /// It asks the steps arrive as steps, not as numbers frozen at authoring time.
    #[test]
    fn a_described_tree_spaces_itself_from_the_theme() {
        use heca_grid_ui::style::Spacing;
        use heca_view::ViewSpacing;
        use heca_view::build::{self, Style as _};

        let (emit, _fired) = recording_emitter();
        let node: ViewNode = build::Surface::new()
            .pad_all(ViewSpacing::Md)
            .gap_spacing(ViewSpacing::Xs)
            .into();
        let w = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        let layout = &w.base().style.layout;
        assert_eq!(layout.pad_spacing_x, Some(Spacing::Md), "padding is a step");
        assert_eq!(layout.pad_spacing_y, Some(Spacing::Md), "on both axes");
        assert_eq!(layout.gap_spacing, Some(Spacing::Xs), "and so is the gap");
    }

    /// **A tooltip declared on ANY kind reaches the node itself** (F003/P097/T501, C3).
    ///
    /// A tooltip is a declaration, not a widget an author places. It stopped being a wrapper
    /// natively for a concrete reason — a wrapper puts the rule in every caller's discipline, and
    /// makes a tooltip impossible on a widget a typed container holds, because wrapping it changes
    /// what it is — so it lives in a slot on every widget's base. The described form is therefore
    /// one field on every node, and there is no `Tooltip` kind for a plugin to construct, size and
    /// anchor (⭐⭐ RULE ZERO — one door, never two).
    ///
    /// Until this, a plugin drawing its UI as a described tree could not put a tooltip on anything,
    /// while heca's own chrome put one on every button in a line. Nothing failed: the shared
    /// builder trait has no generated property surface, so neither the prop applier nor
    /// [`every_widget_property_is_reachable_from_the_sdk`] can see a universal capability at all —
    /// which is why this asks the question directly, for every kind.
    #[test]
    fn a_tooltip_declared_on_any_kind_lands_on_the_node_itself() {
        let mut missing: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            let (emit, _fired) = recording_emitter();
            let node = sample_node(kind)
                .prop("tooltip", PropValue::Text("Close the pane".into()))
                .prop("tooltip_side", PropValue::Text("bottom".into()))
                .prop("tooltip_delay", PropValue::Int(2));
            let widget = realize(
                &node,
                &Theme::default(),
                &emit,
                &mut FormBindings::default(),
            );
            match &widget.base().tooltip {
                None => missing.push(format!("{kind:?} (no tooltip on the realized widget)")),
                Some(tip) => {
                    if tip.side != heca_grid_ui::TooltipSide::Bottom {
                        missing.push(format!("{kind:?} (declared side ignored)"));
                    }
                    // An integer delay is the same number of seconds as a float one to whoever
                    // wrote it, and JSON does not keep them apart.
                    if (tip.delay - 2.0).abs() > f32::EPSILON {
                        missing.push(format!("{kind:?} (declared delay ignored: {})", tip.delay));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "a tooltip declared on these kinds never reached the widget, so a described row \
             cannot say what it is while every native one can: {missing:#?}",
        );
    }

    /// **A card never has to be named** (F003/P097/T501, correcting C2).
    ///
    /// `key` is optional everywhere in this library, so it is optional on a card. A grid whose
    /// cards declare none still has to report *which* card was chosen — otherwise a plugin gets a
    /// picker that lights up, moves, activates, and hands back nothing, with no error anywhere.
    /// That is the shape `.draggable()` shipped in: a capability gated on `Base::key`, silently
    /// dead on every widget nobody had reason to name (AGENTS.md § 0a).
    ///
    /// The name is the card's **own content**, through the accessible-name algorithm the library
    /// already uses for exactly this — [`Component::text_summary`], which is what
    /// `nav::identity_of` reads at its second level. So a card is named the way it reads on
    /// screen, and a caller writes nothing.
    ///
    /// ⚠️ It asks a card whose text is **nested**, because that is the documented shape: a card is
    /// a `Surface` holding an icon and a label, and the card node itself carries no `text` prop of
    /// its own. Reading only the node's own `text` prop passes a one-`Label` card and leaves every
    /// real one anonymous.
    #[test]
    fn an_unnamed_card_is_still_reported_by_the_grid() {
        let (emit, fired) = recording_emitter();

        let card = |name: &str| {
            ViewNode::new(WidgetKind::Surface)
                .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Text("box".into())))
                .child(ViewNode::new(WidgetKind::Label).text(name))
        };
        let node = ViewNode::new(WidgetKind::CardGrid)
            .on("activate", Intent::new("docker.open"))
            .child(card("nginx"))
            .child(card("redis"));

        use heca_grid_ui::event::{Event, WidgetIntent};
        use heca_grid_ui::reactive::SignalUpdate as _;

        let mut grid = realize(
            &node,
            &Theme::default(),
            &emit,
            &mut FormBindings::default(),
        );
        // Keys reach the focus owner and nowhere else, so a test that activates says who is
        // holding the keyboard — exactly as a real surface has to.
        grid.base_mut().focused.set(true);
        heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::Activate));

        assert_eq!(
            fired.borrow().first().and_then(|i| i.args.get("key")),
            Some(&PropValue::Text("nginx".into())),
            "an unnamed card must still come back by name, or a described grid reports nothing",
        );
    }

    /// **The cursor must walk the cards the way the eye does** (F003/P097/T501, correcting C2).
    ///
    /// [`CardGrid::row`] takes *columns* of cards and says so in its own contract: "`cards` must be
    /// in the same left-to-right order the layout draws them, or the cursor and the picture
    /// disagree." A described grid draws its children with `Flex::row` — left to right — so each
    /// child is its own column. Handing the grid one column holding every card instead makes
    /// arrow-right do nothing at all while arrow-down walks a row: the picture says one thing and
    /// the keyboard another, and nothing anywhere fails.
    ///
    /// It also pins the repeat spelling: two cards that read the same are still two cards, indexed
    /// the way [`nav::identity_of`] indexes identically-named widgets (`nginx`, `nginx[1]`) rather
    /// than collapsing onto whichever the grid met first.
    #[test]
    fn a_described_grid_walks_its_cards_the_way_it_draws_them() {
        use heca_grid_ui::event::{Event, WidgetIntent};
        use heca_grid_ui::reactive::SignalUpdate as _;

        let card = |name: &str| {
            ViewNode::new(WidgetKind::Surface).child(ViewNode::new(WidgetKind::Label).text(name))
        };
        let node = ViewNode::new(WidgetKind::CardGrid)
            .on("activate", Intent::new("docker.open"))
            .child(card("nginx"))
            .child(card("redis"))
            .child(card("nginx"));

        let chosen_after = |steps: usize| {
            let (emit, fired) = recording_emitter();
            let mut grid = realize(
                &node,
                &Theme::default(),
                &emit,
                &mut FormBindings::default(),
            );
            grid.base_mut().focused.set(true);
            for _ in 0..steps {
                heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
            }
            heca_grid_ui::dispatch(grid.as_mut(), &Event::Widget(WidgetIntent::Activate));
            fired
                .borrow()
                .last()
                .and_then(|i| i.args.get("key"))
                .cloned()
        };

        assert_eq!(
            chosen_after(1),
            Some(PropValue::Text("redis".into())),
            "one card right",
        );
        assert_eq!(
            chosen_after(2),
            Some(PropValue::Text("nginx[1]".into())),
            "and the second card reading `nginx` is its own card, not the first one again",
        );
    }

    /// **Anything a widget can be given in Rust, a description must be able to ask for.**
    ///
    /// The runtime half: a `hint` written into a node of **any** kind survives `realize` and is
    /// found by the picker. `on_hint` is on `ComponentExt` (F003/P082/T432), so natively *every*
    /// widget can be told what a pick does to it; this holds the described side to the same reach.
    ///
    /// `ScrollBar` is the one exception, and the same one everywhere else: it is host-only, its
    /// state is a live host signal, and `realize` refuses it rather than producing a dead control.
    #[test]
    fn a_hint_written_into_any_kind_is_found_by_the_picker() {
        let mut unreachable: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            if kind == WidgetKind::ScrollBar {
                continue;
            }
            let (emit, fired) = recording_emitter();
            let node = sample_node(kind).on_hint(Intent::new("picked"));
            let mut widget = realize(&node, &Theme::default(), &emit, &mut FormBindings::default());
            if !heca_grid_ui::fire_hint(widget.as_mut(), &[]) {
                unreachable.push(format!("{kind:?} (no hint declaration on the realized widget)"));
                continue;
            }
            if fired.borrow().iter().all(|i| i.action != "picked") {
                unreachable.push(format!("{kind:?} (declared a hint that fired something else)"));
            }
        }
        assert!(
            unreachable.is_empty(),
            "a `hint` event written into these kinds does not reach the picker, so a plugin can \
             draw them but never make them pickable: {unreachable:#?}",
        );
    }

    /// **The same reach, through the typed SDK.**
    ///
    /// The raw `ViewNode` form is the wire; `heca_view::build` is what an author actually writes,
    /// and it is hand-written — so the thing that keeps it honest is a guard, exactly as
    /// [`every_widget_property_is_reachable_from_the_sdk`] does for properties. A capability that
    /// exists on the wire and not in the SDK is one nobody will find.
    ///
    /// It reads the SDK's source rather than calling it, because *"does a method exist"* is not a
    /// question a running test can ask — the same technique, for the same reason.
    ///
    /// ⚠️ **Written red on purpose** (F003/P082/T434): `on_hint` sits on seven kinds today. It is
    /// **F003/P082/T435** that makes it pass, by extending the `with_event!` table. Written
    /// afterwards this test would only describe what was built, which protects nothing.
    #[test]
    fn every_kind_can_be_given_a_hint_from_the_sdk() {
        /// Kinds with no `on_hint`, each with the reason. An entry here is a capability an author
        /// cannot reach — keep it short, and never add one to make the test pass.
        const NO_HINT: &[(&str, &str)] = &[(
            "ScrollBar",
            "host-only: its state is live host signals, and `realize` refuses it outright",
        )];

        let sdk = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-view/src/build.rs"),
        )
        .expect("the SDK source is where it is expected");

        // `with_event!` is the one table that gives a builder its event setters, so this asks the
        // table rather than looking for a method: `Row { on_press => "press", on_hint => "hint" }`.
        let events = {
            let start = sdk.find("with_event!(").expect("the SDK binds its events in one table");
            let rest = &sdk[start..];
            let end = rest.find("\n);").unwrap_or(rest.len());
            rest[..end].to_string()
        };

        let mut missing: Vec<String> = Vec::new();
        for &kind in WidgetKind::ALL {
            let name = format!("{kind:?}");
            if NO_HINT.iter().any(|(k, _)| *k == name) {
                continue;
            }
            let declares = events
                .lines()
                .filter(|l| l.trim_start().starts_with(&format!("{name} {{")))
                .any(|l| l.contains("on_hint"));
            if !declares {
                missing.push(name);
            }
        }

        assert!(
            missing.is_empty(),
            "these kinds take a hint natively (`ComponentExt::on_hint` is on every widget) but \
             cannot be given one through the typed SDK, so an author writing them can draw a \
             thing and never make it pickable: {missing:#?}\n\nAdd `on_hint => \"hint\"` to the \
             kind's `with_event!` row, or add it to NO_HINT with the reason.",
        );
    }

    /// Glyph names resolve to their `Glyph`; unknown names are `None` (no icon), never a panic.
    #[test]
    fn glyph_from_name_resolves_known_and_rejects_unknown() {
        assert_eq!(glyph_from_name("terminal"), Some(Glyph::Terminal));
        assert_eq!(glyph_from_name("git_branch"), Some(Glyph::GitBranch));
        assert_eq!(glyph_from_name("x_square"), Some(Glyph::XSquare));
        assert_eq!(glyph_from_name("not_a_real_icon"), None);
    }

    /// A newly-mapped container attaches its realized children. `Surface` injects no title, so
    /// its child count is exactly the content (unlike `Card`/`DockFrame`, whose `new(title)` adds
    /// a title child first).
    #[test]
    fn container_kind_attaches_children() {
        let node = ViewNode::new(WidgetKind::Surface)
            .child(ViewNode::new(WidgetKind::Label).text("a"))
            .child(ViewNode::new(WidgetKind::Label).text("b"));
        let realized = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 2, "surface holds its two content children");

        // `Card` prepends a title child, so title + 2 content = 3.
        let card = realize(
            &ViewNode::new(WidgetKind::Card)
                .text("Title")
                .child(ViewNode::new(WidgetKind::Label).text("a")),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(card.base().children.len(), 2, "card = title + 1 content");
    }

    /// A `"change"` binding (value widgets) fires the intent but is NOT a pick target (a value
    /// change isn't a gesture a letter can stand for); a `"press"` binding (Item) is.
    #[test]
    fn change_binding_declares_no_hint_but_press_does() {
        let input = realize(
            &ViewNode::new(WidgetKind::Input).on("change", Intent::new("q_changed")),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert!(hints(input.as_ref()).is_empty(), "a change binding is not a pick target");

        let item = realize(
            &ViewNode::new(WidgetKind::Item)
                .text("Row")
                .on_press(Intent::new("row_activated")),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(hints(item.as_ref()), vec![Vec::<usize>::new()], "an actionable Item is");
    }

    /// A value widget with a `"name"` prop is bound into the form; `collect()` reads its current
    /// value under that name. An unnamed value widget is not collected.
    #[test]
    fn named_value_widgets_are_collected() {
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::VStack)
            .child(
                ViewNode::new(WidgetKind::Input)
                    .text("hello")
                    .prop("name", PropValue::Text("q".into())),
            )
            .child(
                ViewNode::new(WidgetKind::Checkbox)
                    .prop("checked", PropValue::Bool(true))
                    .prop("name", PropValue::Text("agree".into())),
            )
            // Unnamed → not collected.
            .child(ViewNode::new(WidgetKind::Input).text("ignored"));
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

        let data = forms.collect();
        assert_eq!(data.len(), 2, "only the two named widgets are collected");
        assert_eq!(data.get("q").and_then(PropValue::as_text), Some("hello"));
        assert_eq!(data.get("agree").and_then(PropValue::as_bool), Some(true));
    }

    /// A named `Select` is a form field: `collect()` returns the **value** of the chosen option
    /// (not its index), mapped through the options' `value` props at the live selection.
    #[test]
    fn named_select_is_collected_as_its_chosen_value() {
        let option = |value: &str, label: &str| {
            ViewNode::new(WidgetKind::Choice)
                .prop("value", PropValue::Text(value.into()))
                .text(label)
        };
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Select)
            .prop("name", PropValue::Text("priority".into()))
            .prop("selected", PropValue::Int(2))
            .child(option("low", "Low"))
            .child(option("medium", "Medium"))
            .child(option("high", "High"));
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

        assert_eq!(
            forms.collect().get("priority").and_then(PropValue::as_text),
            Some("high"),
            "the chosen option's value, not its index",
        );
    }

    /// A rich modal body with one of every value-widget kind marshals each named field into
    /// `collect()` under its own name — the end-to-end shape `ModalResult::Action.data` returns.
    #[test]
    fn rich_modal_body_marshals_every_named_field() {
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::VStack)
            .child(
                ViewNode::new(WidgetKind::Input)
                    .text("nginx")
                    .prop("name", PropValue::Text("host".into())),
            )
            .child(
                ViewNode::new(WidgetKind::Toggle)
                    .prop("on", PropValue::Bool(true))
                    .prop("name", PropValue::Text("tls".into())),
            )
            .child(
                ViewNode::new(WidgetKind::Checkbox)
                    .prop("checked", PropValue::Bool(false))
                    .prop("name", PropValue::Text("force".into())),
            )
            .child(
                ViewNode::new(WidgetKind::Select)
                    .prop("name", PropValue::Text("region".into()))
                    .prop("selected", PropValue::Int(1))
                    .child(
                        ViewNode::new(WidgetKind::Choice)
                            .prop("value", PropValue::Text("eu".into()))
                            .text("Europe"),
                    )
                    .child(
                        ViewNode::new(WidgetKind::Choice)
                            .prop("value", PropValue::Text("us".into()))
                            .text("US"),
                    ),
            );
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

        let data = forms.collect();
        assert_eq!(data.len(), 4, "every named field is collected");
        assert_eq!(data.get("host").and_then(PropValue::as_text), Some("nginx"));
        assert_eq!(data.get("tls").and_then(PropValue::as_bool), Some(true));
        assert_eq!(data.get("force").and_then(PropValue::as_bool), Some(false));
        assert_eq!(data.get("region").and_then(PropValue::as_text), Some("us"));
    }

    /// A named text `Input` exposes its **live value signal** for reactive validation (used to
    /// disable a submit button while empty). Only text inputs register; other kinds / unnamed
    /// inputs do not.
    #[test]
    fn named_input_exposes_live_text_signal() {
        use heca_grid_ui::reactive::SignalUpdate;
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Input)
            .text("term")
            .prop("name", PropValue::Text("name".into()));
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut forms);

        let sig = forms.text_signal("name").expect("named input exposes its signal");
        assert_eq!(sig.get_untracked(), "term", "signal reflects the initial value");
        // The signal is live: updating it is what `collect()` / validation later read.
        sig.set("renamed".to_string());
        assert_eq!(
            forms.collect().get("name").and_then(PropValue::as_text),
            Some("renamed"),
        );
        assert!(forms.text_signal("missing").is_none());
    }

    // ── Generic layout merge (F003/P017/T2) ──────────────────────────────────────────────
    // The point of these: `realize` holds NO list of layout property names. Everything below
    // works because `Layout`'s own fields are the vocabulary.

    /// Properties that NO arm in this file has ever read — `padding`, `width`, `justify`,
    /// `flex_grow`, `margin` — reach the widget anyway, on a kind with no layout code of its own.
    /// This is the regression guard for the whole task: it fails the moment someone reintroduces
    /// a hand-written property list that happens to omit one of these.
    #[test]
    fn layout_properties_never_named_in_realize_still_reach_the_widget() {
        let node = ViewNode::new(WidgetKind::VStack)
            .prop("padding", PropValue::Int(12))
            .prop("width", PropValue::Int(240))
            .prop("justify", PropValue::Text("space_between".into()))
            .prop("flex_grow", PropValue::Float(1.0))
            .prop("margin", PropValue::Float(6.0));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let l = w.base().style.layout;
        assert_eq!(l.padding, 12.0, "padding — in the original design doc, never implemented");
        assert_eq!(l.width, Length::Px(240.0));
        assert_eq!(l.justify, Justify::SpaceBetween, "enum by name, snake_case");
        assert_eq!(l.flex_grow, 1.0);
        assert_eq!(l.margin, 6.0);
    }

    /// `Length` reads the way an author would write it: a bare number is px, `"auto"` is auto,
    /// and a percentage string is a fraction — not the enum's `{"px": 240}` shape.
    #[test]
    fn length_accepts_the_spelling_an_author_would_reach_for() {
        let case = |p: PropValue| {
            let node = ViewNode::new(WidgetKind::Surface).prop("width", p);
            realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default())
                .base()
                .style
                .layout
                .width
        };
        assert_eq!(case(PropValue::Int(240)), Length::Px(240.0));
        assert_eq!(case(PropValue::Float(12.5)), Length::Px(12.5));
        assert_eq!(case(PropValue::Text("auto".into())), Length::Auto);
        assert_eq!(case(PropValue::Text("50%".into())), Length::Pct(0.5));
    }

    /// Untrusted input stays total, and — the part that matters — a single bad value costs only
    /// itself. A wrong type, an unparseable length and an unknown key all get dropped while the
    /// good properties on the same node still land.
    #[test]
    fn a_bad_property_never_takes_the_good_ones_with_it() {
        let node = ViewNode::new(WidgetKind::VStack)
            .prop("gap", PropValue::Int(8))
            .prop("padding", PropValue::Text("not a number".into()))
            .prop("width", PropValue::Text("50 furlongs".into()))
            .prop("nonsense_key", PropValue::Int(3))
            .prop("justify", PropValue::Text("sideways".into()))
            .prop("margin", PropValue::Float(4.0));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let l = w.base().style.layout;
        assert_eq!(l.gap, 8.0, "good property survives a bad neighbour");
        assert_eq!(l.margin, 4.0, "and so does one declared after the bad ones");
        assert_eq!(l.padding, 0.0, "bad value ignored — the default stands");
        assert_eq!(l.width, Length::Auto, "unparseable length ignored");
        assert_eq!(l.justify, Justify::Start, "unknown enum name ignored");
    }

    /// The merge lands ON TOP of the constructed widget. `ScrollRegion::new()` zeroes its min
    /// sizes and opts into shrinking so a viewport can be smaller than its content; rebuilding
    /// from `Layout::default()` would undo that and the region would silently stop scrolling.
    #[test]
    fn merging_preserves_layout_the_widget_set_in_its_constructor() {
        let node = ViewNode::new(WidgetKind::Scroll).prop("padding", PropValue::Int(4));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let l = w.base().style.layout;
        assert_eq!(l.padding, 4.0, "the property the node did carry");
        assert_eq!(l.min_height, Some(Length::Px(0.0)), "constructor value survives");
        assert_eq!(l.min_width, Some(Length::Px(0.0)));
        assert_eq!(l.flex_shrink, Some(1.0), "without this a scroll region cannot shrink");
        assert!(l.gap_spacing.is_some(), "theme spacing token survives");
    }

    /// **A described node can place itself at a fractional rect** — the declarative half of
    /// [`LayoutExt::at_rect`](heca_grid_ui::builders::LayoutExt::at_rect), which a plugin needs for
    /// the same reason the exposé does: a box whose position its parent cannot express.
    ///
    /// ⚠️ Written because **adding serde to a type is not the same as being able to author it**
    /// (AGENTS, F003/P011/T018). `Placement` groups four `Length`s, so it can only travel as a
    /// [`PropValue::Map`] — a scalar channel would have carried nothing and nothing would have
    /// failed. This is the check that the value channel is real.
    #[test]
    fn a_description_can_place_a_node_at_a_fractional_rect() {
        let node = ViewNode::new(WidgetKind::VStack).prop(
            "placement",
            PropValue::Map(
                [
                    ("left".to_string(), PropValue::Text("25%".into())),
                    ("top".to_string(), PropValue::Text("10%".into())),
                    ("width".to_string(), PropValue::Text("50%".into())),
                    ("height".to_string(), PropValue::Int(120)),
                ]
                .into_iter()
                .collect(),
            ),
        );

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        let placement = w.base().style.layout.placement.expect("the rect was authored");
        assert_eq!(placement.left, Length::Pct(0.25));
        assert_eq!(placement.top, Length::Pct(0.10));
        assert_eq!(placement.width, Length::Pct(0.5));
        assert_eq!(placement.height, Length::Px(120.0), "a bare number is pixels");
    }

    /// A node with no properties leaves the widget exactly as its constructor built it.
    #[test]
    fn a_node_with_no_properties_changes_nothing() {
        let bare = realize(
            &ViewNode::new(WidgetKind::Scroll),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        assert_eq!(bare.base().style.layout, ScrollRegion::new().base().style.layout);
    }

    // ── The generated surface, end to end (F003/P017/T3) ─────────────────────────────────
    // The two capabilities that started this phase: both existed in the widgets, and neither
    // could be set from a description while this file named properties by hand.

    /// An input's placeholder. `Input::placeholder` has existed all along and the showcase uses it
    /// twice, including the command palette — yet a description could not say it.
    #[test]
    fn a_description_can_now_set_an_inputs_placeholder() {
        let node = ViewNode::new(WidgetKind::Input)
            .text("current")
            .prop("placeholder", PropValue::Text("type to filter…".into()));

        let input = with_props(Input::new().value(text_of(&node)), &node, &Theme::default());
        assert_eq!(input.placeholder_str(), "type to filter…");
        assert_eq!(input.value_str(), "current", "the value still lands alongside it");
    }

    /// A scroll region's axes. `ScrollRegion` has supported both all along — the showcase's own
    /// root is `.both()` — but every declarative region was vertical, forever.
    #[test]
    fn a_description_can_now_ask_for_a_two_axis_scroll_region() {
        let both = ViewNode::new(WidgetKind::Scroll).prop("axes", PropValue::Text("both".into()));
        assert_eq!(
            with_props(ScrollRegion::new(), &both, &Theme::default()).clone_axes(),
            heca_grid_ui::ScrollAxes::Both,
        );
        assert_eq!(
            with_props(ScrollRegion::new(), &ViewNode::new(WidgetKind::Scroll), &Theme::default()).clone_axes(),
            heca_grid_ui::ScrollAxes::Vertical,
            "unset still means the widget's own default",
        );
    }

    /// Total for untrusted input: an unknown enum name and a property belonging to a different
    /// widget both leave the widget alone, and the good property on the same node still lands.
    #[test]
    fn the_generated_surface_ignores_what_it_cannot_use() {
        let node = ViewNode::new(WidgetKind::Scroll)
            .prop("axes", PropValue::Text("sideways".into()))
            .prop("placeholder", PropValue::Text("not a scroll property".into()))
            .prop("gap", PropValue::Int(6));

        assert_eq!(
            with_props(ScrollRegion::new(), &node, &Theme::default()).clone_axes(),
            heca_grid_ui::ScrollAxes::Vertical,
            "unknown variant name keeps the default",
        );
        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut FormBindings::default());
        assert_eq!(w.base().style.layout.gap, 6.0, "the good property still lands");
    }

    /// The two tests above read the widget through `with_props`, which is the surface but not the
    /// path a real description takes. These two go through **`realize` itself** and read the result
    /// the only way a `Box<dyn Component>` allows — by painting it — so the arm is proven to wire
    /// the surface, not just the surface proven to exist.
    ///
    /// An empty input paints its placeholder, so the text is in the scene when the arm passed it on
    /// and absent when it did not.
    #[test]
    fn a_realized_input_paints_the_placeholder_it_was_given() {
        use heca_core::layout::Size;
        use heca_grid_ui::{DrawCommand, LayoutEngine, PaintCx, Scene, Theme};

        let runs = |node: &ViewNode| {
            let mut widget = realize(
                node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            LayoutEngine::new().compute(widget.as_mut(), Size::new(240.0, 40.0));
            let theme = Theme::default();
            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                widget.paint(&mut cx);
            }
            scene
                .iter()
                .filter_map(|c| match c {
                    DrawCommand::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        // No value, so the placeholder is what shows.
        let with = ViewNode::new(WidgetKind::Input)
            .prop("placeholder", PropValue::Text("type to filter…".into()));
        assert!(
            runs(&with).contains(&"type to filter…".to_string()),
            "the realized input paints the placeholder it was described with",
        );
        assert!(
            !runs(&ViewNode::new(WidgetKind::Input)).contains(&"type to filter…".to_string()),
            "and it is the property that put it there, not the widget's own default",
        );
    }

    /// A described two-axis region **is** the native one: same widget, same content, same scene.
    /// The `axes` property is the only difference between the two calls, and it is what makes the
    /// horizontal bar appear — every declarative region was vertical forever before it.
    #[test]
    fn a_realized_two_axis_region_is_the_native_one() {
        use heca_core::layout::Size;
        use heca_grid_ui::{LayoutEngine, PaintCx, Parent, Scene, ScrollAxes, Theme};

        // Content wider AND taller than the viewport, so both axes overflow and both bars draw.
        let content = ViewNode::new(WidgetKind::VStack)
            .prop("width", PropValue::Int(400))
            .prop("height", PropValue::Int(300))
            .child(ViewNode::new(WidgetKind::Label).text("content"));
        let realized_content = || {
            realize(
                &content,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            )
        };

        let paint = |mut widget: Box<dyn Component>| {
            LayoutEngine::new().compute(widget.as_mut(), Size::new(120.0, 80.0));
            let theme = Theme::default();
            let mut scene = Scene::new();
            {
                let mut cx = PaintCx::new(&mut scene, &theme);
                widget.paint(&mut cx);
            }
            scene.iter().cloned().collect::<Vec<_>>()
        };

        let described = |axes: Option<&str>| {
            let mut node = ViewNode::new(WidgetKind::Scroll)
                .prop("width", PropValue::Int(120))
                .prop("height", PropValue::Int(80))
                .child(content.clone());
            if let Some(axes) = axes {
                node = node.prop("axes", PropValue::Text(axes.into()));
            }
            paint(realize(
                &node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            ))
        };

        let native = |axes: ScrollAxes| {
            paint(Box::new(
                ScrollRegion::new()
                    .axes(axes)
                    .width(Length::Px(120.0))
                    .height(Length::Px(80.0))
                    .child_boxed(realized_content()),
            ))
        };

        assert_eq!(described(Some("both")), native(ScrollAxes::Both), "same widget, same scene");
        assert_eq!(described(None), native(ScrollAxes::Vertical), "unset = the widget's default");
        assert_ne!(
            described(Some("both")),
            described(None),
            "the property is what adds the second axis (and its bar)",
        );
        assert_eq!(
            described(Some("sideways")),
            native(ScrollAxes::Vertical),
            "an unknown axis name keeps the default, through the whole path",
        );
    }

    /// And it **scrolls** both ways, not just paints a second bar: a described two-axis region
    /// consumes a horizontal wheel delta, where a described default region leaves it for the host.
    /// (The visible behaviour is confirmed in the running app; this pins the routing.)
    #[test]
    fn a_realized_two_axis_region_consumes_a_horizontal_wheel() {
        use heca_core::layout::{Point, Size};
        use heca_grid_ui::{Event, Handled, LayoutEngine};

        let horizontal_wheel = |axes: Option<&str>| {
            let mut node = ViewNode::new(WidgetKind::Scroll)
                .prop("width", PropValue::Int(120))
                .prop("height", PropValue::Int(80))
                .child(
                    ViewNode::new(WidgetKind::VStack)
                        .prop("width", PropValue::Int(400))
                        .prop("height", PropValue::Int(300))
                        .child(ViewNode::new(WidgetKind::Label).text("content")),
                );
            if let Some(axes) = axes {
                node = node.prop("axes", PropValue::Text(axes.into()));
            }
            let mut region = realize(
                &node,
                &Theme::default(),
                &noop_emitter(),
                &mut FormBindings::default(),
            );
            LayoutEngine::new().compute(region.as_mut(), Size::new(120.0, 80.0));
            // The wheel is hover-gated (`Event::Scroll` carries no position), so hover it first.
            heca_grid_ui::dispatch(region.as_mut(), &Event::pointer_moved(Point::new(60.0, 40.0)));
            heca_grid_ui::dispatch(region.as_mut(), &Event::wheel(Point::new(60.0, 40.0), -1.0, 0.0))
        };

        assert_eq!(
            horizontal_wheel(Some("both")),
            Handled::Yes,
            "a described two-axis region scrolls horizontally",
        );
        assert_eq!(
            horizontal_wheel(None),
            Handled::No,
            "a described default region still has no horizontal axis to scroll",
        );
    }
}
