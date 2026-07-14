//! `realize` — the host mapper from the declarative [`ViewNode`] model to a retained
//! grid-ui [`Component`] tree (plugin-task-ui-3).
//!
//! [`ViewNode`](super::ViewNode) is the pure, serializable UI *description* (same model
//! authored in Rust or shipped by a WASM plugin). `realize` is the **host-side** binding
//! that turns it into live widgets: it owns the grid-ui dependency and the theme/registry
//! wiring, so the model stays free of both.
//!
//! Two host services are threaded in:
//! - `emit` — the chrome intent sink ([`ChromeIntentEmitter`](super::ChromeIntentEmitter)):
//!   a realized actionable widget fires its [`view::Intent`](super::Intent) wrapped as
//!   [`InteractionIntent::View`] on **click**.
//! - `hints` — the shared [`HintTargetRegistry`](super::HintTargetRegistry): every
//!   actionable node is registered so the universal picker (`prefix+/`) reaches it by
//!   letter, firing the *same* intent as a click. This is the "every clickable widget is
//!   also hintable" rule, for free (plan §2.7.2, "everything is an action").
//!
//! Adding a widget = one [`WidgetKind`](super::WidgetKind) arm here (+ its variant in the model).
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
//! walks [`WidgetKind::ALL`](super::WidgetKind::ALL) and fails if a kind produces neither children
//! nor paint — so a new kind added without an arm cannot silently render an empty container.
//!
//! Seam module (consumed by the OverlayHost/Modal body in ui-4 and plugin panels) — carries
//! `#![allow(dead_code)]` like the sibling chrome seam modules until those consumers land.
#![allow(dead_code)]

use heca_grid_ui::reactive::{Signal, SignalGet};
use heca_grid_ui::{
    Action, Alert, Align, Badge, BadgeButton, Button, ButtonVariant, Card, Checkbox, Choice,
    Component, DockFrame, Flex, Gauge, Glyph, Grid, HintExt, HintTargetId, Icon, IconButton, Input,
    Item, ItemGroup, Label, LayoutExt, MarkerGroup, RailCell, ScrollRegion, Select, SignalData,
    StatusDot, Surface, Tabs, Tag, Toast, ToastSeverity, Toggle, Track, WidgetSize,
};

use super::view::{PropMap, PropValue, ViewAlign, ViewNode, ViewSize, ViewVariant, WidgetKind};
use super::{ChromeIntentEmitter, HintTargetRegistry};
use crate::app::interaction::InteractionIntent;

/// Reads a form field's current value at submit time. Boxed because the concrete widget signal
/// type varies (String / bool / …). Native-side only (never crosses the plugin boundary).
type FieldReader = Box<dyn Fn() -> PropValue>;

/// The named value fields a realized tree exposes, collected into a [`PropMap`] when the overlay
/// is submitted (→ [`ModalResult::Action`](super::ModalResult)'s `data`). A value node opts in by
/// carrying a `"name"` prop; `realize` binds a reader over its live value signal. Order is
/// registration order (deterministic).
#[derive(Default)]
pub(crate) struct FormBindings {
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
    pub(crate) fn text_signal(&self, name: &str) -> Option<Signal<String>> {
        self.text_signals
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| *s)
    }

    /// Read every bound field's current value into a name→value map.
    pub(crate) fn collect(&self) -> PropMap {
        self.fields.iter().map(|(k, r)| (k.clone(), r())).collect()
    }
}

/// Realize a [`ViewNode`] (and its subtree) into a retained grid-ui component.
///
/// Returns a `Box<dyn Component>` because the produced widget type depends on the runtime
/// [`kind`](WidgetKind). NB: `Box<dyn Component>` is **not** itself `Component`, so a
/// container can't take it via `Parent::child` (which boxes an `impl Component`); realized
/// children are pushed straight onto `base_mut().children` (the `Vec<Box<dyn Component>>`).
pub(crate) fn realize(
    node: &ViewNode,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    let mut realized = realize_kind(node, emit, hints, forms);
    // Self-alignment is a property of the node *inside its parent*, so it applies to every kind —
    // read it once here rather than in each arm.
    if let Some(align) = align_prop(node, "align_self") {
        realized.base_mut().style.align_self = Some(align);
    }
    if let Some(justify) = align_prop(node, "justify_self") {
        realized.base_mut().style.justify_self = Some(justify);
    }
    realized
}

/// The per-kind mapping — see [`realize`], which wraps it with the props every node can carry.
fn realize_kind(
    node: &ViewNode,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    match node.kind {
        // ── Containers (attach realized children) ──
        WidgetKind::Column => realize_flex(node, Flex::column(), emit, hints, forms),
        WidgetKind::Row => realize_flex(node, Flex::row(), emit, hints, forms),
        WidgetKind::Card => {
            attach_children(Box::new(Card::new(text_of(node))), node, emit, hints, forms)
        }
        // No dedicated `Panel` widget — a bare panel is a plain `Surface`.
        WidgetKind::Surface | WidgetKind::Panel => {
            attach_children(Box::new(Surface::new()), node, emit, hints, forms)
        }
        WidgetKind::Scroll => {
            attach_children(Box::new(ScrollRegion::new()), node, emit, hints, forms)
        }

        // ── Leaves ──
        WidgetKind::Label => {
            // Weight + slant are font attributes (the shaper picks the glyphs); underline +
            // strikethrough are decorations the widget draws. Both are plain bools here.
            let label = Label::new(text_of(node))
                .bold(bool_prop(node, "bold").unwrap_or(false))
                .italic(bool_prop(node, "italic").unwrap_or(false))
                .underline(bool_prop(node, "underline").unwrap_or(false))
                .strikethrough(bool_prop(node, "strikethrough").unwrap_or(false));
            Box::new(label)
        }
        WidgetKind::Button => realize_button(node, emit, hints, forms),
        WidgetKind::Badge => Box::new(Badge::new(text_of(node))),
        WidgetKind::Tag => Box::new(Tag::new(text_of(node))),
        WidgetKind::Alert => Box::new(Alert::new(text_of(node))),
        WidgetKind::StatusDot => Box::new(StatusDot::online()),
        WidgetKind::Gauge => {
            let mut g = Gauge::new();
            if let Some(v) = f32_prop(node, "value") {
                g = g.value(v);
            }
            Box::new(g)
        }
        WidgetKind::Icon => match glyph_prop(node) {
            Some(glyph) => Box::new(Icon::new(glyph)),
            None => Box::new(Flex::empty()),
        },
        WidgetKind::Input => {
            let mut input = Input::new().value(text_of(node));
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
            let mut t = Toggle::new().on(bool_prop(node, "on").unwrap_or(false));
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
            let mut c = Checkbox::new()
                .checked(bool_prop(node, "checked").unwrap_or(false))
                .label(text_of(node));
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
                let realized = realize(child, emit, hints, forms);
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
            let mut choice = realize_choice(node, emit, hints, forms);
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
            for option in realize_options(node, emit, hints, forms) {
                select = select.option(option);
            }
            if let Some(i) = usize_prop(node, "selected") {
                select = select.selected(i);
            }
            if let Some(on_change) = option_change(node, emit) {
                select = select.on_change(on_change);
            }
            Box::new(select)
        }
        WidgetKind::Tabs => {
            let mut tabs = Tabs::empty();
            for option in realize_options(node, emit, hints, forms) {
                tabs = tabs.tab(option);
            }
            if let Some(i) = usize_prop(node, "selected") {
                tabs = tabs.selected(i);
            }
            if let Some(on_change) = option_change(node, emit) {
                tabs = tabs.on_change(on_change);
            }
            Box::new(tabs)
        }

        // ── Groups ──
        WidgetKind::ItemGroup => {
            let mut group =
                ItemGroup::new(text_of(node)).expanded(bool_prop(node, "expanded").unwrap_or(true));
            if let Some(on_toggle) = toggle_change(node, emit) {
                group = group.on_toggle(on_toggle);
            }
            // The group's own header is `children[0]`; the realized rows follow it.
            attach_children(Box::new(group), node, emit, hints, forms)
        }
        WidgetKind::MarkerGroup => {
            let mut markers = MarkerGroup::new();
            if let Some(active) = bool_prop(node, "active") {
                markers = markers.active(active);
            }
            if let Some(nav) = bool_prop(node, "nav_selected") {
                markers = markers.nav_selected(nav);
            }
            // An indicator: no events of its own — the rows inside carry their own intents.
            attach_children(Box::new(markers), node, emit, hints, forms)
        }

        // ── Layout ──
        WidgetKind::Grid => realize_grid(node, emit, hints, forms),

        WidgetKind::DockFrame => {
            let mut dock = DockFrame::new(text_of(node))
                .expanded(bool_prop(node, "expanded").unwrap_or(true))
                .active(bool_prop(node, "active").unwrap_or(false))
                .nav_selected(bool_prop(node, "nav_selected").unwrap_or(false));
            if bool_prop(node, "frameless").unwrap_or(false) {
                dock = dock.frameless();
            }
            if let Some(on_toggle) = toggle_change(node, emit) {
                dock = dock.on_toggle(on_toggle);
            }
            // Slots: `header` is the controls slot (a search field, a count badge); everything else
            // is body content — the body is the **default slot**, so an unslotted child lands there.
            for child in &node.children {
                let realized = realize(child, emit, hints, forms);
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
            if let Some(dismissible) = bool_prop(node, "dismissible") {
                toast = toast.dismissible(dismissible);
            }
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
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    for child in &node.children {
        container.base_mut().children.push(realize(child, emit, hints, forms));
    }
    container
}

/// Register the node's `"press"` (activation) intent as a hint target, returning the id + the
/// carrier intent to fire on click. `None` when the node isn't actionable.
fn press_intent(node: &ViewNode, hints: &mut HintTargetRegistry) -> Option<(HintTargetId, InteractionIntent)> {
    let intent = node.intent("press")?;
    let carrier = InteractionIntent::View(intent.clone());
    let id = hints.register(carrier.clone());
    Some((id, carrier))
}

/// The node's `"change"` intent as a carrier (value widgets — input/toggle/checkbox). No hint
/// target: a value change isn't a pick target. Data marshalling into the intent is a later step
/// (plugin-task-ui-4 remainder); today the change simply fires the bound intent.
fn change_intent(node: &ViewNode) -> Option<InteractionIntent> {
    let intent = node.intent("change")?;
    Some(InteractionIntent::View(intent.clone()))
}

/// Realize a container node onto a base [`Flex`] (row or column), applying layout props and
/// recursively realizing + attaching children.
fn realize_flex(
    node: &ViewNode,
    mut flex: Flex,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    if let Some(gap) = f32_prop(node, "gap") {
        flex = flex.gap(gap);
    }
    if let Some(align) = align_prop(node, "align") {
        flex = flex.align(align);
    }
    for child in &node.children {
        // `child()` takes an `impl Component` and boxes it; a `Box<dyn Component>` isn't
        // `Component`, so push the already-boxed child directly.
        flex.base_mut().children.push(realize(child, emit, hints, forms));
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
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
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
        let carrier = InteractionIntent::View(intent.clone());
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
            .push(realize(child, emit, hints, forms));
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
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
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
            .push(realize(child, emit, hints, forms));
    }
    choice
}

/// Realize the `Choice` children of a `Select`/`Tabs`, in order.
///
/// A child of another kind is **ignored** (with a debug log): `realize` is total for untrusted
/// input, and the options of an option-picker are options.
fn realize_options(
    node: &ViewNode,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
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
        .map(|child| realize_choice(child, emit, hints, forms))
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
fn option_change(node: &ViewNode, emit: &ChromeIntentEmitter) -> Option<impl Fn(Action) + 'static> {
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
        emit(InteractionIntent::View(intent));
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
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
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
    // How the items sit inside their cells: `align` vertically, `justify_items` horizontally. Both
    // default to `Stretch`, which pins an explicitly-sized item to the top-left of its cell — so a
    // row of mixed-height content needs `align: center` to share a centre line.
    if let Some(align) = align_prop(node, "align") {
        grid = grid.align(align);
    }
    if let Some(justify) = align_prop(node, "justify_items") {
        grid = grid.justify_items(justify);
    }
    for child in &node.children {
        let realized = realize(child, emit, hints, forms);
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
fn intent_carrier(node: &ViewNode, event: &str) -> Option<InteractionIntent> {
    node.intent(event)
        .map(|i| InteractionIntent::View(i.clone()))
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
fn toggle_change(node: &ViewNode, emit: &ChromeIntentEmitter) -> Option<impl Fn(Action) + 'static> {
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
        emit(InteractionIntent::View(intent));
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

/// A numeric prop (`Int` or `Float`) as `f32` — e.g. `"gap"`, `"value"`.
fn f32_prop(node: &ViewNode, key: &str) -> Option<f32> {
    match node.props.get(key)? {
        PropValue::Int(i) => Some(*i as f32),
        PropValue::Float(f) => Some(*f as f32),
        _ => None,
    }
}

/// A `"bool"`-typed prop (`"on"`, `"checked"`).
fn bool_prop(node: &ViewNode, key: &str) -> Option<bool> {
    node.props.get(key).and_then(PropValue::as_bool)
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

/// An alignment prop mapped to the grid-ui [`Align`] — `"align"` (a container's cross-axis
/// alignment of its children), `"align_self"` / `"justify_self"` (this node inside its parent), or
/// `"justify_items"` (a grid's horizontal placement of its items).
fn align_prop(node: &ViewNode, key: &str) -> Option<Align> {
    match node.props.get(key)? {
        PropValue::Align(a) => Some(map_align(*a)),
        _ => None,
    }
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

fn map_align(a: ViewAlign) -> Align {
    match a {
        ViewAlign::Start => Align::Start,
        ViewAlign::Center => Align::Center,
        ViewAlign::End => Align::End,
        ViewAlign::Stretch => Align::Stretch,
    }
}

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
    use crate::chrome::Intent;
    use std::rc::Rc;

    /// A confirm-dialog-shaped tree: a column with a message label + a row of two action
    /// buttons (Cancel / Delete), each carrying a `"press"` intent.
    fn confirm_tree() -> ViewNode {
        ViewNode::new(WidgetKind::Column)
            .prop("gap", PropValue::Int(8))
            .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
            .child(
                ViewNode::new(WidgetKind::Row)
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

    fn noop_emitter() -> ChromeIntentEmitter {
        Rc::new(|_| {})
    }

    /// The realized tree mirrors the model's structure: the column has 2 children (label +
    /// row) and the row has 2 children (the buttons). Verifies recursion + child attachment
    /// through `base_mut().children` (the `Box<dyn Component>` push path).
    #[test]
    fn realizes_nested_structure() {
        let mut hints = HintTargetRegistry::default();
        let root = realize(&confirm_tree(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(root.base().children.len(), 2, "column: label + row");
        let row = &root.base().children[1];
        assert_eq!(row.base().children.len(), 2, "row: two buttons");
    }

    /// Every actionable node (a `"press"` binding) registers exactly one hint target, and
    /// each carries `InteractionIntent::View` wrapping the node's own intent — so the picker
    /// fires the identical action a click would. Non-actionable nodes register nothing.
    #[test]
    fn actionable_nodes_register_view_intents() {
        let mut hints = HintTargetRegistry::default();
        let before = hints.checkpoint();
        let _ = realize(&confirm_tree(), &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(
            hints.checkpoint() - before,
            2,
            "only the two buttons are actionable (label + containers are not)",
        );
        // Realize order is depth-first: Cancel registers first, then Delete.
        let cancel = hints.get(heca_grid_ui::HintTargetId::new(before)).unwrap();
        let delete = hints.get(heca_grid_ui::HintTargetId::new(before + 1)).unwrap();
        assert!(
            matches!(cancel, InteractionIntent::View(i) if i.action == "confirm_cancel"),
            "first target = Cancel's View intent, got {cancel:?}",
        );
        assert!(
            matches!(delete, InteractionIntent::View(i) if i.action == "confirm_ok"),
            "second target = Delete's View intent, got {delete:?}",
        );
    }

    /// A `Button` node's **children are its content**: an arbitrary subtree is realized and mounted
    /// inside the button. Before the button composed its content, `realize` had nowhere to put them
    /// and dropped them silently — a declarative `Button(Icon + Label)` rendered as a bare button.
    #[test]
    fn button_children_are_realized_as_its_content() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Button)
            .prop("variant", PropValue::Variant(ViewVariant::Destructive))
            .on_press(Intent::new("confirm_ok"))
            .child(
                ViewNode::new(WidgetKind::Column)
                    .prop("gap", PropValue::Int(4))
                    .child(
                        ViewNode::new(WidgetKind::Row)
                            .child(
                                ViewNode::new(WidgetKind::Icon)
                                    .prop("icon", PropValue::Glyph("trash".into())),
                            )
                            .child(ViewNode::new(WidgetKind::Label).text("Delete")),
                    )
                    .child(ViewNode::new(WidgetKind::Label).text("Ctrl+D")),
            );

        let button = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Button)
            .text("Delete")
            .prop("icon", PropValue::Glyph("trash".into()));
        let button = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(
            button.base().children.len(),
            2,
            "text + icon props desugar into [Icon, Label] children",
        );
    }

    /// A still-deferred structured kind (needs track/slot model support — `Grid`, `choice-6`)
    /// realizes to an empty container instead of panicking — the tree stays total for untrusted
    /// plugin/RPC input.
    #[test]
    fn deferred_kind_is_empty_not_panic() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Grid);
        let realized = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Select)
            .prop("selected", PropValue::Int(1))
            .child(option_node("low", "LOW"))
            .child(option_node("high", "HIGH"));

        let select = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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

        let fired: Rc<RefCell<Vec<InteractionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: ChromeIntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Select)
            .on("change", Intent::new("set_level"))
            .child(option_node("low", "LOW"))
            .child(option_node("high", "HIGH"));
        let mut select = realize(&node, &emit, &mut HintTargetRegistry::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(select.as_mut(), Size::new(400.0, 300.0));

        // Open the dropdown, then click the second option where it actually is (its real bounds).
        let trigger = select.base().bounds;
        select.event(&Event::PointerPressed {
            pos: Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0),
        });
        let high = select.base().children[1].base().bounds;
        select.event(&Event::PointerPressed {
            pos: Point::new(high.loc.x + 5.0, high.loc.y + high.size.h / 2.0),
        });

        let fired = fired.borrow();
        let [InteractionIntent::View(intent)] = fired.as_slice() else {
            panic!("expected exactly one View intent, got {fired:?}");
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

        let fired: Rc<RefCell<Vec<InteractionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: ChromeIntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Tabs)
            .on("change", Intent::new("show_tab"))
            .child(option_node("files", "FILES"))
            .child(option_node("issues", "ISSUES"));
        let mut tabs = realize(&node, &emit, &mut HintTargetRegistry::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(tabs.as_mut(), Size::new(400.0, 100.0));
        assert_eq!(tabs.base().children.len(), 2, "one tab per Choice child");

        tabs.event(&Event::Widget(WidgetIntent::ItemNext));
        let fired = fired.borrow();
        let [InteractionIntent::View(intent)] = fired.as_slice() else {
            panic!("expected exactly one View intent, got {fired:?}");
        };
        assert_eq!(intent.action, "show_tab");
        assert_eq!(intent.args.get("value"), Some(&PropValue::Text("issues".into())));
    }

    /// A **childless** `Choice` falls back to the scalar sugar — `text` → one `Label` child, the very
    /// child the composed form would build. Same precedence rule as `Button`: children win.
    #[test]
    fn childless_choice_node_desugars_its_text_to_a_label_child() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text("high".into()))
            .text("HIGH");
        let choice = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(choice.base().children.len(), 1, "text desugars to a Label child");
        assert_eq!(choice.text_summary().as_deref(), Some("HIGH"));
    }

    /// A stray non-`Choice` child of an option picker is **ignored**, not realized into a broken
    /// option and not a panic: `realize` is total for untrusted plugin/RPC input.
    #[test]
    fn a_non_choice_child_of_a_select_is_ignored() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Select)
            .child(option_node("low", "LOW"))
            .child(ViewNode::new(WidgetKind::Button).text("I am not an option"))
            .child(option_node("high", "HIGH"));
        let select = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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

        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::ItemGroup)
            .text("EXPLORER")
            .prop("expanded", PropValue::Bool(false))
            .child(ViewNode::new(WidgetKind::Item).text("src"))
            .child(ViewNode::new(WidgetKind::Item).text("tests"));

        let mut group = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        // The group applies its expanded state during layout (`remeasure`), like the native widget.
        LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));
        assert_eq!(
            group.base().children.len(),
            3,
            "the group's own header, then the two realized rows",
        );
        // Collapsed: the rows leave layout (`display: none`), the header stays.
        assert!(!group.base().children[0].base().style.hidden, "the header stays");
        assert!(
            group.base().children[1..]
                .iter()
                .all(|row| row.base().style.hidden),
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

        let fired: Rc<RefCell<Vec<InteractionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: ChromeIntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::ItemGroup)
            .text("EXPLORER")
            .on("toggle", Intent::new("fold_group"))
            .child(ViewNode::new(WidgetKind::Item).text("src"));
        let mut group = realize(&node, &emit, &mut HintTargetRegistry::default(), &mut FormBindings::default());
        LayoutEngine::new().compute(group.as_mut(), Size::new(300.0, 200.0));

        // Click the header (it starts expanded) → it collapses.
        let header = group.base().children[0].base().bounds;
        group.event(&Event::PointerPressed {
            pos: Point::new(header.loc.x + 5.0, header.loc.y + header.size.h / 2.0),
        });

        let fired = fired.borrow();
        let [InteractionIntent::View(intent)] = fired.as_slice() else {
            panic!("expected exactly one View intent, got {fired:?}");
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
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::MarkerGroup)
            .prop("active", PropValue::Bool(true))
            .prop("nav_selected", PropValue::Bool(true))
            .child(ViewNode::new(WidgetKind::Item).text("pane 1"))
            .child(ViewNode::new(WidgetKind::Item).text("pane 2"));

        let markers = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = HintTargetRegistry::default();
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

        let grid = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        let children = &grid.base().children;
        assert_eq!(children.len(), 3);

        // The `icon` area spans both rows of column 1 (it appears twice in the template).
        assert_eq!(
            children[0].base().style.grid_cell,
            Some(heca_grid_ui::GridCell { col: 1, row: 1, col_span: 1, row_span: 2 }),
            "placed into the named area, spanning what the template gives it",
        );
        assert_eq!(
            children[1].base().style.grid_cell,
            Some(heca_grid_ui::GridCell { col: 2, row: 1, col_span: 2, row_span: 1 }),
            "placed by explicit cell; an omitted span defaults to 1",
        );
        assert_eq!(
            children[2].base().style.grid_cell,
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

        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("align", PropValue::Align(ViewAlign::Center))
            .prop("justify_items", PropValue::Align(ViewAlign::Center))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("pinned")
                    .prop("align_self", PropValue::Align(ViewAlign::End))
                    .prop("justify_self", PropValue::Align(ViewAlign::End)),
            );

        let grid = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        let style = grid.base().style;
        assert_eq!(style.align, Align::Center, "vertical: the items in their cells");
        assert_eq!(
            style.justify_items,
            Some(Align::Center),
            "horizontal: `justify_items`, not `justify` (which moves the track set)",
        );
        let child = grid.base().children[0].base().style;
        assert_eq!(child.align_self, Some(Align::End));
        assert_eq!(child.justify_self, Some(Align::End));
    }

    /// An unknown area name is not an error — the child simply auto-places (realize stays total).
    #[test]
    fn grid_child_in_an_unknown_area_auto_places() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Grid)
            .prop("areas", PropValue::List(vec![PropValue::Text("a b".into())]))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("x")
                    .prop("area", PropValue::Text("nope".into())),
            );
        let grid = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(grid.base().children[0].base().style.grid_cell, None);
    }

    /// A `Label`'s text attributes are authorable: weight + slant (font attributes) and underline +
    /// strikethrough (decorations the widget draws). Absent props keep the widget's default.
    #[test]
    fn label_text_attributes_are_authorable() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Label)
            .text("DONE")
            .prop("bold", PropValue::Bool(true))
            .prop("italic", PropValue::Bool(true))
            .prop("strikethrough", PropValue::Bool(true));

        let label = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = HintTargetRegistry::default();
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

        let dock = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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
        let mut hints = HintTargetRegistry::default();
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

        let item = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
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

        let fired: Rc<RefCell<Vec<InteractionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = fired.clone();
        let emit: ChromeIntentEmitter = Rc::new(move |i| sink.borrow_mut().push(i));

        let node = ViewNode::new(WidgetKind::Toast)
            .text("Build failed")
            .prop("severity", PropValue::Text("danger".into()))
            .prop("body", PropValue::Text("3 errors in heca-grid-ui".into()))
            .prop("action_text", PropValue::Text("RETRY".into()))
            .on("action", Intent::new("rebuild"))
            .on("dismiss", Intent::new("close_toast"));

        let toast = realize(&node, &emit, &mut HintTargetRegistry::default(), &mut FormBindings::default());
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
            WidgetKind::Column
            | WidgetKind::Row
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
                &noop_emitter(),
                &mut HintTargetRegistry::default(),
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
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Surface)
            .child(ViewNode::new(WidgetKind::Label).text("a"))
            .child(ViewNode::new(WidgetKind::Label).text("b"));
        let realized = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 2, "surface holds its two content children");

        // `Card` prepends a title child, so title + 2 content = 3.
        let card = realize(
            &ViewNode::new(WidgetKind::Card)
                .text("Title")
                .child(ViewNode::new(WidgetKind::Label).text("a")),
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
        let mut hints = HintTargetRegistry::default();
        let before = hints.checkpoint();
        let _ = realize(
            &ViewNode::new(WidgetKind::Input).on("change", Intent::new("q_changed")),
            &noop_emitter(),
            &mut hints,
            &mut FormBindings::default(),
        );
        assert_eq!(hints.checkpoint() - before, 0, "a change binding is not a hint target");

        let _ = realize(
            &ViewNode::new(WidgetKind::Item)
                .text("Row")
                .on_press(Intent::new("row_activated")),
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
        let mut hints = HintTargetRegistry::default();
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Column)
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
        let _ = realize(&node, &noop_emitter(), &mut hints, &mut forms);

        let data = forms.collect();
        assert_eq!(data.len(), 2, "only the two named widgets are collected");
        assert_eq!(data.get("q").and_then(PropValue::as_text), Some("hello"));
        assert_eq!(data.get("agree").and_then(PropValue::as_bool), Some(true));
    }

    /// A named text `Input` exposes its **live value signal** for reactive validation (used to
    /// disable a submit button while empty). Only text inputs register; other kinds / unnamed
    /// inputs do not.
    #[test]
    fn named_input_exposes_live_text_signal() {
        use heca_grid_ui::reactive::SignalUpdate;
        let mut hints = HintTargetRegistry::default();
        let mut forms = FormBindings::default();
        let node = ViewNode::new(WidgetKind::Input)
            .text("term")
            .prop("name", PropValue::Text("name".into()));
        let _ = realize(&node, &noop_emitter(), &mut hints, &mut forms);

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
}
