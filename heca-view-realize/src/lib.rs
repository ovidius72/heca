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
//! Two host services are threaded in. Both are **narrow seams** over the model's own
//! [`Intent`], not app types, so this mapper can live and be called below `heca`
//! (F003/P017/T009):
//! - `emit` — an [`IntentEmitter`]: a realized actionable widget fires the node's `Intent` on
//!   **click**. What it means is the host's business; the app wraps it as
//!   `InteractionIntent::View`.
//! - `hints` — a [`HintTargets`] sink: every actionable node is registered so the universal
//!   picker (`prefix+/`) reaches it by letter, firing the *same* intent as a click. This is the
//!   "every clickable widget is also hintable" rule, for free (plan §2.7.2, "everything is an
//!   action"). The host keeps the real registry; realize only needs to put an intent into it.
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
use heca_grid_ui::{
    Action, Alert, Badge, BadgeButton, Button, ButtonVariant, Card, Checkbox, Choice,
    Component, DockFrame, Flex, Gauge, Glyph, Grid, HintExt, HintTargetId, Icon, IconButton, Input,
    Item, ItemGroup, Label, LayoutExt, MarkerGroup, Panel, PropInput, RailCell,
    Row as GridRow, ScrollRegion, Select, Separator, SetProp, SignalData, StatusDot, Surface, Tabs,
    Tag, Theme, Toast, ToastSeverity, Toggle, Track, WidgetSize,
};

use heca_view::{
    Intent, PropMap, PropValue, ViewNode, ViewSize, ViewVariant, WidgetKind,
};

/// Where a realized tree's intents go: a widget fires the node's own [`Intent`], and the host
/// wraps it in whatever it dispatches (the app wraps it as `InteractionIntent::View`).
///
/// `Rc` because every actionable widget clones the sink into its own callback.
pub type IntentEmitter = Rc<dyn Fn(Intent)>;

/// The host's KeyHint pick registry, narrowed to the one thing `realize` does with it: put an
/// intent in, get its id back.
///
/// A seam rather than the concrete registry because the real one is **shared** — the overlay
/// registers modal buttons carrying intents that are not view intents at all — so it has to keep
/// storing the app's own carrier type. This is the part realize needs, and nothing more.
pub trait HintTargets {
    /// Register an actionable node's intent as a pick target and return its freshly-allocated id.
    fn register(&mut self, intent: Intent) -> HintTargetId;
}

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
pub fn realize(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    hints: &mut dyn HintTargets,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    let mut realized = realize_kind(node, theme, emit, hints, forms);
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

/// The per-kind mapping — see [`realize`], which wraps it with the props every node can carry.
fn realize_kind(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    hints: &mut dyn HintTargets,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    match node.kind {
        // ── Containers (attach realized children) ──
        WidgetKind::VStack => realize_flex(node, theme, Flex::column(), emit, hints, forms),
        WidgetKind::HStack => realize_flex(node, theme, Flex::row(), emit, hints, forms),
        // The interactive row — a container that is also a control. `active` / `nav_selected` /
        // `marker` arrive through the generated surface; the press is wired to BOTH a click and a
        // hint target, like `Button`, so `prefix+/` reaches it. Without `on_activate` the widget
        // stays non-focusable and paints no hover, which is the right answer for a row with no
        // press intent — a described row that nothing can activate should not pretend otherwise.
        WidgetKind::Row => {
            let mut row = with_props(GridRow::new(), node, theme);
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                row = row
                    .hint_target(id)
                    .on_activate(move || emit(carrier.clone()));
            }
            attach_children(Box::new(row), node, theme, emit, hints, forms)
        }
        WidgetKind::Card => {
            attach_children(Box::new(Card::new(text_of(node))), node, theme, emit, hints, forms)
        }
        WidgetKind::Surface => {
            attach_children(Box::new(Surface::new()), node, theme, emit, hints, forms)
        }
        // `Panel` used to be an alias for `Surface`, which is why the published examples showed
        // `Panel::new().title(..)` against a widget that had no title (F003/P017/T008). It is its
        // own widget now; `text` is the heading, as it is for `Card` and `DockFrame`.
        WidgetKind::Panel => {
            let panel = with_props(Panel::new().title(text_of(node)), node, theme);
            attach_children(Box::new(panel), node, theme, emit, hints, forms)
        }
        WidgetKind::Scroll => {
            // `axes` reaches the widget through its own builder, so a declarative region can be
            // horizontal or two-axis — it was vertical-only for as long as this arm named its
            // properties by hand.
            let region = with_props(ScrollRegion::new(), node, theme);
            attach_children(Box::new(region), node, theme, emit, hints, forms)
        }

        // ── Leaves ──
        WidgetKind::Label => {
            // bold / italic / underline / strikethrough / align / font_size all arrive through
            // the generated surface — `Label`'s builders decide which, not a list here.
            Box::new(with_props(Label::new(text_of(node)), node, theme))
        }
        WidgetKind::Button => realize_button(node, theme, emit, hints, forms),
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
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                b = b.hint_target(id).on_click(move || emit(carrier.clone()));
            }
            Box::new(b)
        }
        WidgetKind::IconButton => {
            let icon = Icon::new(glyph_prop(node).unwrap_or(Glyph::Circle));
            let mut b = IconButton::new(icon);
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                b = b.hint_target(id).on_click(move || emit(carrier.clone()));
            }
            Box::new(b)
        }
        WidgetKind::Item => {
            let mut it = Item::new(text_of(node));
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                it = it.hint_target(id).on_activate(move || emit(carrier.clone()));
            }
            // Slots: the row's leading / trailing affordances. It has **no default slot** — its
            // middle is the label, which comes from `text` — so a child with neither slot name is
            // ignored rather than silently dropped somewhere it doesn't belong.
            for child in &node.children {
                let realized = realize(child, theme, emit, hints, forms);
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
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                cell = cell.hint_target(id).on_activate(move || emit(carrier.clone()));
            }
            Box::new(cell)
        }

        // ── Options ──
        // An option is a node with a **value** and arbitrary content — and options are **children**,
        // not a `props["options"]` list of strings. That is what lets a declarative option compose
        // an icon + a label exactly like a native one, and what carries the chosen *value* back to
        // the author (see `realize_options`).
        WidgetKind::Choice => {
            let mut choice = realize_choice(node, theme, emit, hints, forms);
            // A standalone `Choice` (outside a Select/Tabs) is activatable on its own. Inside a
            // container the container owns the click, so a `press` there is ignored, not half-wired.
            if let Some((id, carrier)) = press_intent(node, hints) {
                let emit = emit.clone();
                choice = choice
                    .hint_target(id)
                    .on_activate(move || emit(carrier.clone()));
            }
            Box::new(choice)
        }
        WidgetKind::Select => {
            let mut select = Select::empty();
            for option in realize_options(node, theme, emit, hints, forms) {
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
            for option in realize_options(node, theme, emit, hints, forms) {
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
            attach_children(Box::new(group), node, theme, emit, hints, forms)
        }
        WidgetKind::MarkerGroup => {
            let markers = with_props(MarkerGroup::new(), node, theme);
            // An indicator: no events of its own — the rows inside carry their own intents.
            attach_children(Box::new(markers), node, theme, emit, hints, forms)
        }

        // ── Layout ──
        WidgetKind::Grid => realize_grid(node, theme, emit, hints, forms),

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
                let realized = realize(child, theme, emit, hints, forms);
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
            if let Some(body) = node.props.get("body").and_then(PropValue::as_text) {
                toast = toast.body(body);
            }
            toast = with_props(toast, node, theme);
            // The inline action is a **labelled button**, not arbitrary content — so it is a prop
            // (`action_text`) plus an `action` intent, not a slot. A slot would have promised
            // composition the widget doesn't offer.
            if let Some(label) = node.props.get("action_text").and_then(PropValue::as_text)
                && let Some(carrier) = intent_carrier(node, "action")
            {
                let emit = emit.clone();
                toast = toast.action(label, move || emit(carrier.clone()));
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
    hints: &mut dyn HintTargets,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    for child in &node.children {
        container.base_mut().children.push(realize(child, theme, emit, hints, forms));
    }
    container
}

/// Register the node's `"press"` (activation) intent as a hint target, returning the id + the
/// carrier intent to fire on click. `None` when the node isn't actionable.
fn press_intent(node: &ViewNode, hints: &mut dyn HintTargets) -> Option<(HintTargetId, Intent)> {
    let intent = node.intent("press")?;
    let carrier = intent.clone();
    let id = hints.register(carrier.clone());
    Some((id, carrier))
}

/// The node's `"change"` intent as a carrier (value widgets — input/toggle/checkbox). No hint
/// target: a value change isn't a pick target. Data marshalling into the intent is a later step
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
    hints: &mut dyn HintTargets,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    // `gap`, `align` and every other layout property are applied generically by
    // `merge_layout_props` in `realize`, for every kind — not read per-arm here.
    for child in &node.children {
        // `child()` takes an `impl Component` and boxes it; a `Box<dyn Component>` isn't
        // `Component`, so push the already-boxed child directly.
        flex.base_mut().children.push(realize(child, theme, emit, hints, forms));
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
    hints: &mut dyn HintTargets,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
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
    if let Some(intent) = node.intent("press") {
        // One intent, two input paths: register it for the picker, emit the same on click.
        let carrier = intent.clone();
        let id = hints.register(carrier.clone());
        let emit = emit.clone();
        button = button
            .hint_target(id)
            .on_click(move || emit(carrier.clone()));
    }
    // Attach the composed content. (`Box<dyn Component>` isn't `Component`, so it can't go through
    // `Parent::child`; push it the way every other container here does.)
    let mut button: Box<dyn Component> = Box::new(button);
    for child in &node.children {
        button
            .base_mut()
            .children
            .push(realize(child, theme, emit, hints, forms));
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
    hints: &mut dyn HintTargets,
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
            .push(realize(child, theme, emit, hints, forms));
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
    hints: &mut dyn HintTargets,
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
        .map(|child| realize_choice(child, theme, emit, hints, forms))
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
fn realize_grid(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    hints: &mut dyn HintTargets,
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
        let realized = realize(child, theme, emit, hints, forms);
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
    let g = match name {
        "folder" => Glyph::Folder,
        "folder_open" => Glyph::FolderOpen,
        "file" => Glyph::File,
        "file_code" => Glyph::FileCode,
        "git_branch" => Glyph::GitBranch,
        "git_commit" => Glyph::GitCommit,
        "git_merge" => Glyph::GitMerge,
        "git_pull_request" => Glyph::GitPullRequest,
        "terminal" => Glyph::Terminal,
        "gear" => Glyph::Gear,
        "search" => Glyph::Search,
        "close" => Glyph::Close,
        "check" => Glyph::Check,
        "caret_right" => Glyph::CaretRight,
        "caret_down" => Glyph::CaretDown,
        "play" => Glyph::Play,
        "pause" => Glyph::Pause,
        "stop" => Glyph::Stop,
        "warning" => Glyph::Warning,
        "warning_circle" => Glyph::WarningCircle,
        "info" => Glyph::Info,
        "circle" => Glyph::Circle,
        "lightning" => Glyph::Lightning,
        "list" => Glyph::List,
        "sidebar" => Glyph::Sidebar,
        "dots_three_vertical" => Glyph::DotsThreeVertical,
        "arrow_right" => Glyph::ArrowRight,
        "arrow_line_left" => Glyph::ArrowLineLeft,
        "arrow_line_right" => Glyph::ArrowLineRight,
        "plus" => Glyph::Plus,
        "minus" => Glyph::Minus,
        "square_split_vertical" => Glyph::SquareSplitVertical,
        "x_square" => Glyph::XSquare,
        "frame_corners" => Glyph::FrameCorners,
        "cards" => Glyph::Cards,
        "trash" => Glyph::Trash,
        _ => return None,
    };
    Some(g)
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

    /// The tests' own pick registry. `realize` no longer knows the app's, so the assertions run
    /// against a sink of the same shape: ids are handed out by position, in registration order.
    #[derive(Default)]
    struct TestHints {
        registered: Vec<Intent>,
    }

    impl HintTargets for TestHints {
        fn register(&mut self, intent: Intent) -> HintTargetId {
            let id = HintTargetId::new(self.registered.len());
            self.registered.push(intent);
            id
        }
    }

    impl TestHints {
        /// The id the next `register` will hand out — same contract as the host registry's, so a
        /// test can take a checkpoint either side of a build and count what it registered.
        fn checkpoint(&self) -> usize {
            self.registered.len()
        }

        /// The intent registered under `id`.
        fn get(&self, id: HintTargetId) -> Option<&Intent> {
            self.registered.get(id.raw())
        }
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

    /// The realized tree mirrors the model's structure: the column has 2 children (label +
    /// row) and the row has 2 children (the buttons). Verifies recursion + child attachment
    /// through `base_mut().children` (the `Box<dyn Component>` push path).
    #[test]
    fn realizes_nested_structure() {
        let mut hints = TestHints::default();
        let root = realize(&confirm_tree(), &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(root.base().children.len(), 2, "column: label + row");
        let row = &root.base().children[1];
        assert_eq!(row.base().children.len(), 2, "row: two buttons");
    }

    /// Every actionable node (a `"press"` binding) registers exactly one hint target carrying the
    /// node's own intent — so the picker fires the identical action a click would. Non-actionable
    /// nodes register nothing.
    #[test]
    fn actionable_nodes_register_view_intents() {
        let mut hints = TestHints::default();
        let before = hints.checkpoint();
        let _ = realize(&confirm_tree(), &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(
            hints.checkpoint() - before,
            2,
            "only the two buttons are actionable (label + containers are not)",
        );
        // Realize order is depth-first: Cancel registers first, then Delete.
        let cancel = hints.get(heca_grid_ui::HintTargetId::new(before)).unwrap();
        let delete = hints.get(heca_grid_ui::HintTargetId::new(before + 1)).unwrap();
        assert_eq!(cancel.action, "confirm_cancel", "first target = Cancel's intent");
        assert_eq!(delete.action, "confirm_ok", "second target = Delete's intent");
    }

    /// A `Button` node's **children are its content**: an arbitrary subtree is realized and mounted
    /// inside the button. Before the button composed its content, `realize` had nowhere to put them
    /// and dropped them silently — a declarative `Button(Icon + Label)` rendered as a bare button.
    #[test]
    fn button_children_are_realized_as_its_content() {
        let mut hints = TestHints::default();
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

        let button = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        let column = &button.base().children;
        assert_eq!(column.len(), 1, "the button holds its composed subtree");
        let column = &column[0].base().children;
        assert_eq!(column.len(), 2, "column: the icon+label row, then the accelerator label");
        assert_eq!(column[0].base().children.len(), 2, "row: icon + label");

        // The button is still one actionable target, whatever it composes.
        assert_eq!(hints.checkpoint(), 1, "one hint target: the button itself, not its content");
    }

    /// A **childless** Button node falls back to the scalar sugar — `text` (+ an optional leading
    /// `icon`) — which builds the very same children the explicit form would. One content model,
    /// two spellings.
    #[test]
    fn childless_button_node_uses_the_scalar_sugar() {
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Button)
            .text("Delete")
            .prop("icon", PropValue::Glyph("trash".into()));
        let button = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
            &mut TestHints::default(),
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
            &mut TestHints::default(),
            &mut FormBindings::default(),
        );
        let one_field = realize(
            &ViewNode::new(WidgetKind::Surface).prop("radius", PropValue::Float(9.0)),
            &Theme::default(),
            &noop_emitter(),
            &mut TestHints::default(),
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
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
            &mut TestHints::default(),
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
        check("Label", <Label as SetProp>::PROP_NAMES, "Label");
        check("MarkerGroup", <MarkerGroup as SetProp>::PROP_NAMES, "MarkerGroup");
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

        assert!(
            missing.is_empty(),
            "these widget properties have no setter in heca-view/src/build.rs, so a description \
             cannot reach them through the SDK: {missing:#?}\n\nAdd a setter, or add the property \
             to NOT_IN_SDK with the reason.",
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
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
            &mut TestHints::default(),
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
            &mut TestHints::default(),
            &mut FormBindings::default(),
        );
        assert_eq!(w.base().style.visual.radius, 7.0, "the good one still applied");
        assert!(w.base().style.visual.fill.is_none(), "the bad one was dropped, not fatal");
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
        let mut hints = TestHints::default();

        let node = ViewNode::new(WidgetKind::Row)
            .prop("active", PropValue::Bool(true))
            .on_press(Intent::new("docker.select").arg("id", PropValue::Text("web".into())))
            .child(ViewNode::new(WidgetKind::Label).text("nginx"))
            .child(ViewNode::new(WidgetKind::Badge).text("UP"));

        let mut row = realize(&node, &Theme::default(), &emit, &mut hints, &mut FormBindings::default());
        LayoutEngine::new().compute(row.as_mut(), Size::new(400.0, 40.0));

        assert_eq!(row.base().children.len(), 2, "it holds its composed content");
        assert!(row.base().focusable, "an actionable row is focusable");
        assert_eq!(hints.checkpoint(), 1, "one hint target: the row itself");

        let b = row.base().bounds;
        heca_grid_ui::dispatch(row.as_mut(), &Event::PointerPressed {
            pos: Point::new(b.loc.x + 5.0, b.loc.y + b.size.h / 2.0),
        });
        heca_grid_ui::dispatch(row.as_mut(), &Event::Key { key: GridKey::Enter, pressed: true });

        let fired = fired.borrow();
        assert_eq!(fired.len(), 2, "a click and an Enter each fire it: {fired:?}");
        for intent in fired.iter() {
            assert_eq!(intent.action, "docker.select");
            assert_eq!(intent.args.get("id"), Some(&PropValue::Text("web".into())));
        }
    }

    /// A `Row` with no press intent stays inert — not focusable, no hint target. A described row
    /// that nothing can activate should not pretend to be a control.
    #[test]
    fn a_described_row_without_a_press_intent_is_inert() {
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Row)
            .child(ViewNode::new(WidgetKind::Label).text("just content"));
        let row = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert!(!row.base().focusable);
        assert_eq!(hints.checkpoint(), 0);
        assert_eq!(row.base().children.len(), 1, "it still holds its content");
    }

    /// A still-deferred structured kind (needs track/slot model support — `Grid`, `choice-6`)
    /// realizes to an empty container instead of panicking — the tree stays total for untrusted
    /// plugin/RPC input.
    #[test]
    fn deferred_kind_is_empty_not_panic() {
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Grid);
        let realized = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 0);
        assert_eq!(hints.checkpoint(), 0, "an empty fallback registers no hints");
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Select)
            .prop("selected", PropValue::Int(1))
            .child(option_node("low", "LOW"))
            .child(option_node("high", "HIGH"));

        let select = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut select = realize(&node, &Theme::default(), &emit, &mut TestHints::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(select.as_mut(), Size::new(400.0, 300.0));

        // Open the dropdown, then click the second option where it actually is (its real bounds).
        let trigger = select.base().bounds;
        heca_grid_ui::dispatch(select.as_mut(), &Event::PointerPressed {
            pos: Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0),
        });
        let high = select.base().children[1].base().bounds;
        heca_grid_ui::dispatch(select.as_mut(), &Event::PointerPressed {
            pos: Point::new(high.loc.x + 5.0, high.loc.y + high.size.h / 2.0),
        });

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
        let mut tabs = realize(&node, &Theme::default(), &emit, &mut TestHints::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(tabs.as_mut(), Size::new(400.0, 100.0));
        assert_eq!(tabs.base().children.len(), 2, "one tab per Choice child");

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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text("high".into()))
            .text("HIGH");
        let choice = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(choice.base().children.len(), 1, "text desugars to a Label child");
        assert_eq!(choice.text_summary().as_deref(), Some("HIGH"));
    }

    /// A stray non-`Choice` child of an option picker is **ignored**, not realized into a broken
    /// option and not a panic: `realize` is total for untrusted plugin/RPC input.
    #[test]
    fn a_non_choice_child_of_a_select_is_ignored() {
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Select)
            .child(option_node("low", "LOW"))
            .child(ViewNode::new(WidgetKind::Button).text("I am not an option"))
            .child(option_node("high", "HIGH"));
        let select = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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

        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::ItemGroup)
            .text("EXPLORER")
            .prop("expanded", PropValue::Bool(false))
            .child(ViewNode::new(WidgetKind::Item).text("src"))
            .child(ViewNode::new(WidgetKind::Item).text("tests"));

        let mut group = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut group = realize(&node, &Theme::default(), &emit, &mut TestHints::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));

        // Click the header (it starts expanded) → it collapses.
        let header = group.base().children[0].base().bounds;
        heca_grid_ui::dispatch(group.as_mut(), &Event::PointerPressed {
            pos: Point::new(header.loc.x + 5.0, header.loc.y + header.size.h / 2.0),
        });

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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::MarkerGroup)
            .prop("active", PropValue::Bool(true))
            .prop("nav_selected", PropValue::Bool(true))
            .child(ViewNode::new(WidgetKind::Item).text("pane 1"))
            .child(ViewNode::new(WidgetKind::Item).text("pane 2"));

        let markers = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(markers.base().children.len(), 2, "the two realized rows");
        assert_eq!(hints.checkpoint(), 0, "an indicator registers no hint target");
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
        let mut hints = TestHints::default();
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

        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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

        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("align", PropValue::Align(ViewAlign::Center))
            .prop("justify_items", PropValue::Align(ViewAlign::Center))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("pinned")
                    .prop("align_self", PropValue::Align(ViewAlign::End))
                    .prop("justify_self", PropValue::Align(ViewAlign::End)),
            );

        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("areas", PropValue::List(vec![PropValue::Text("a b".into())]))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("x")
                    .prop("area", PropValue::Text("nope".into())),
            );
        let grid = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(grid.base().children[0].base().style.layout.grid_cell, None);
    }

    /// A `Label`'s text attributes are authorable: weight + slant (font attributes) and underline +
    /// strikethrough (decorations the widget draws). Absent props keep the widget's default.
    #[test]
    fn label_text_attributes_are_authorable() {
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Label)
            .text("DONE")
            .prop("bold", PropValue::Bool(true))
            .prop("italic", PropValue::Bool(true))
            .prop("strikethrough", PropValue::Bool(true));

        let label = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = TestHints::default();
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

        let dock = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = TestHints::default();
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

        let item = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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

    /// `Toast` needs no slots: its inline action is a **labelled button**, not arbitrary content, so
    /// it is a prop (`action_text`) + an `action` intent. A slot would have promised a composition
    /// the widget does not offer.
    #[test]
    fn toast_node_realizes_its_props_and_three_intents() {
        use std::cell::RefCell;

        let fired: Rc<RefCell<Vec<Intent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: IntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Toast)
            .text("Build failed")
            .prop("severity", PropValue::Text("danger".into()))
            .prop("body", PropValue::Text("3 errors in heca-grid-ui".into()))
            .prop("action_text", PropValue::Text("RETRY".into()))
            .on("action", Intent::new("rebuild"))
            .on("dismiss", Intent::new("close_toast"));

        let toast = realize(&node, &Theme::default(), &emit, &mut TestHints::default(), &mut FormBindings::default());
        assert!(
            toast.base().children.is_empty(),
            "the Toast draws its own card — it takes no children",
        );

        // An unknown severity degrades to the widget's default rather than erroring.
        let bogus = ViewNode::new(WidgetKind::Toast)
            .text("x")
            .prop("severity", PropValue::Text("catastrophic".into()));
        assert_eq!(severity_prop(&bogus), heca_grid_ui::ToastSeverity::Info);
        assert_eq!(severity_prop(&node), heca_grid_ui::ToastSeverity::Danger);
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
            | WidgetKind::Card
            | WidgetKind::Scroll
            | WidgetKind::Panel
            | WidgetKind::Surface
            | WidgetKind::Grid
            | WidgetKind::MarkerGroup
            | WidgetKind::ItemGroup
            | WidgetKind::DockFrame => node
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
            WidgetKind::StatusDot => node,

            // A rule normally stretches to its container; as a root it has none, so give it a
            // span — and take the chance to drive both of its properties, in the order that would
            // have been wrong before the widget started recomputing from the pair.
            WidgetKind::Separator => node
                .prop("length", PropValue::Float(120.0))
                .prop("orientation", PropValue::Text("vertical".into())),

            // Host-only — see the coverage test.
            WidgetKind::ScrollBar => node,
        }
    }

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
                &mut TestHints::default(),
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Surface)
            .child(ViewNode::new(WidgetKind::Label).text("a"))
            .child(ViewNode::new(WidgetKind::Label).text("b"));
        let realized = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 2, "surface holds its two content children");

        // `Card` prepends a title child, so title + 2 content = 3.
        let card = realize(
            &ViewNode::new(WidgetKind::Card)
                .text("Title")
                .child(ViewNode::new(WidgetKind::Label).text("a")),
            &Theme::default(),
            &noop_emitter(),
            &mut hints,
            &mut FormBindings::default(),
        );
        assert_eq!(card.base().children.len(), 2, "card = title + 1 content");
    }

    /// A `"change"` binding (value widgets) fires the intent but does NOT register a hint target
    /// (a value change isn't a pick target); a `"press"` binding (Item) does register one.
    #[test]
    fn change_binding_adds_no_hint_but_press_does() {
        let mut hints = TestHints::default();
        let before = hints.checkpoint();
        let _ = realize(
            &ViewNode::new(WidgetKind::Input).on("change", Intent::new("q_changed")),
            &Theme::default(),
            &noop_emitter(),
            &mut hints,
            &mut FormBindings::default(),
        );
        assert_eq!(hints.checkpoint() - before, 0, "a change binding is not a hint target");

        let _ = realize(
            &ViewNode::new(WidgetKind::Item)
                .text("Row")
                .on_press(Intent::new("row_activated")),
            &Theme::default(),
            &noop_emitter(),
            &mut hints,
            &mut FormBindings::default(),
        );
        assert_eq!(hints.checkpoint() - before, 1, "an actionable Item registers one hint");
    }

    /// A value widget with a `"name"` prop is bound into the form; `collect()` reads its current
    /// value under that name. An unnamed value widget is not collected.
    #[test]
    fn named_value_widgets_are_collected() {
        let mut hints = TestHints::default();
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
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut forms);

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
        let mut hints = TestHints::default();
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Select)
            .prop("name", PropValue::Text("priority".into()))
            .prop("selected", PropValue::Int(2))
            .child(option("low", "Low"))
            .child(option("medium", "Medium"))
            .child(option("high", "High"));
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut forms);

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
        let mut hints = TestHints::default();
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
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut forms);

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
        let mut hints = TestHints::default();
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Input)
            .text("term")
            .prop("name", PropValue::Text("name".into()));
        let _ = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut forms);

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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::VStack)
            .prop("padding", PropValue::Int(12))
            .prop("width", PropValue::Int(240))
            .prop("justify", PropValue::Text("space_between".into()))
            .prop("flex_grow", PropValue::Float(1.0))
            .prop("margin", PropValue::Float(6.0));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = TestHints::default();
        let mut case = |p: PropValue| {
            let node = ViewNode::new(WidgetKind::Surface).prop("width", p);
            realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default())
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::VStack)
            .prop("gap", PropValue::Int(8))
            .prop("padding", PropValue::Text("not a number".into()))
            .prop("width", PropValue::Text("50 furlongs".into()))
            .prop("nonsense_key", PropValue::Int(3))
            .prop("justify", PropValue::Text("sideways".into()))
            .prop("margin", PropValue::Float(4.0));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Scroll).prop("padding", PropValue::Int(4));

        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        let l = w.base().style.layout;
        assert_eq!(l.padding, 4.0, "the property the node did carry");
        assert_eq!(l.min_height, Some(Length::Px(0.0)), "constructor value survives");
        assert_eq!(l.min_width, Some(Length::Px(0.0)));
        assert_eq!(l.flex_shrink, Some(1.0), "without this a scroll region cannot shrink");
        assert!(l.gap_spacing.is_some(), "theme spacing token survives");
    }

    /// A node with no properties leaves the widget exactly as its constructor built it.
    #[test]
    fn a_node_with_no_properties_changes_nothing() {
        let mut hints = TestHints::default();
        let bare = realize(
            &ViewNode::new(WidgetKind::Scroll),
            &Theme::default(),
            &noop_emitter(),
            &mut hints,
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
        let mut hints = TestHints::default();
        let node = ViewNode::new(WidgetKind::Scroll)
            .prop("axes", PropValue::Text("sideways".into()))
            .prop("placeholder", PropValue::Text("not a scroll property".into()))
            .prop("gap", PropValue::Int(6));

        assert_eq!(
            with_props(ScrollRegion::new(), &node, &Theme::default()).clone_axes(),
            heca_grid_ui::ScrollAxes::Vertical,
            "unknown variant name keeps the default",
        );
        let w = realize(&node, &Theme::default(), &noop_emitter(), &mut hints, &mut FormBindings::default());
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
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
                &mut TestHints::default(),
                &mut FormBindings::default(),
            );
            LayoutEngine::new().compute(region.as_mut(), Size::new(120.0, 80.0));
            // The wheel is hover-gated (`Event::Scroll` carries no position), so hover it first.
            heca_grid_ui::dispatch(region.as_mut(), &Event::PointerMoved { pos: Point::new(60.0, 40.0) });
            heca_grid_ui::dispatch(region.as_mut(), &Event::Scroll { delta_x: -1.0, delta_y: 0.0 })
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
