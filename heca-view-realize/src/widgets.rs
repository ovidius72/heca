//! The widgets that take more than props: flex containers, buttons, options, card grids, grids and
//! toggles.

use heca_grid_ui::{
    Action, Button, Choice, Component, Flex, Grid, LayoutExt, Row as GridRow, SignalData, Theme,
    Track,
};
use heca_grid_ui::widgets::{CardGrid, GridCell};
use heca_view::{PropValue, ViewEvent, ViewNode, WidgetKind};

use crate::{FormBindings, IntentEmitter, realize};
use crate::intents::press_intent;
use crate::props::{
    glyph_prop, size_prop, text_of, usize_prop, value_prop, value_string, variant_prop,
};
use crate::style::with_props;

/// Realize a container node onto a base [`Flex`] (row or column), applying layout props and
/// recursively realizing + attaching children.
pub(crate) fn realize_flex(
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
        flex.base_mut()
            .children
            .push(realize(child, theme, emit, forms));
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
pub(crate) fn realize_button(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    Box::new(button_of(node, theme, emit, forms))
}

/// The same button, **typed** — what a container holding `Button`s needs.
///
/// [`ButtonGroup::child`](heca_grid_ui::widgets::ButtonGroup::child) takes a `Button`, not a boxed component,
/// because it reads the button's label, glyph and click out of it to build the overflow menu — a
/// box would have erased all three. Same reason [`realize_choice`] is typed.
///
/// Split out rather than written twice: a second button-building path is exactly the drift this
/// project forbids, and the group's buttons must be the buttons a description asked for, down to
/// the press intent and the composed-children precedence.
pub(crate) fn button_of(
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
pub(crate) fn realize_choice(
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
pub(crate) fn realize_options(
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
pub(crate) fn option_change(node: &ViewNode, emit: &IntentEmitter) -> Option<impl Fn(Action) + 'static> {
    let intent = node.intent(ViewEvent::Change)?.clone();
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
/// **A grid of cards with a cursor**, described.
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
pub(crate) fn realize_card_grid(
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
        let card = GridRow::new().child(body);
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
    if let Some(intent) = node.events.get(ViewEvent::Activate.name()) {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid.on_activate(move |key| {
            emit(intent.clone().arg("key", PropValue::Text(key.to_string())))
        });
    }
    if let Some(intent) = node.events.get(ViewEvent::Move.name()) {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid
            .on_move(move |key| emit(intent.clone().arg("key", PropValue::Text(key.to_string()))));
    }
    if let Some(intent) = node.events.get(ViewEvent::Dismiss.name()) {
        let (intent, emit) = (intent.clone(), emit.clone());
        grid = grid.on_dismiss(move || emit(intent.clone()));
    }
    Box::new(grid)
}

pub(crate) fn realize_grid(
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    let mut grid = Grid::new();
    if let Some(columns) = track_list(node, "columns") {
        grid = grid.template_column(columns);
    }
    if let Some(rows) = track_list(node, "rows") {
        grid = grid.template_row(rows);
    }
    // Areas must be defined before a child can be placed into one by name.
    if let Some(areas) = string_list(node, "areas") {
        grid = grid.template_area(areas);
    }
    // How the items sit inside their cells — `align` vertically, `justify_items` horizontally —
    // arrives through the generic layout merge in `realize`, like every other layout property.
    // Both default to `Stretch`, which pins an explicitly-sized item to the top-left of its cell,
    // so a row of mixed-height content needs `align: center` to share a centre line.
    // **A described child places itself, exactly as a native one does.** `area`, `col`/`row` and
    // the spans are properties of the CHILD — CSS's `grid-area` and `grid-column`/`grid-row` — so
    // the grid only holds them. A name is resolved during layout, against whichever template ends
    // up holding the child.
    for child in &node.children {
        let mut realized = realize(child, theme, emit, forms);
        if let Some(area) = child.props.get("area").and_then(PropValue::as_text) {
            realized.base_mut().grid_area = Some(area.to_string());
        } else if let (Some(col), Some(row)) = (usize_prop(child, "col"), usize_prop(child, "row"))
        {
            realized.base_mut().style.layout.grid_cell = Some(heca_grid_ui::style::GridCell {
                col: col as u16,
                row: row as u16,
                col_span: usize_prop(child, "col_span").unwrap_or(1) as u16,
                row_span: usize_prop(child, "row_span").unwrap_or(1) as u16,
            });
        }
        grid.base_mut().children.push(realized);
    }
    Box::new(grid)
}

/// A [`PropValue::List`] of track strings, parsed to [`Track`]s (`"columns"` / `"rows"`).
pub(crate) fn track_list(node: &ViewNode, key: &str) -> Option<Vec<Track>> {
    let PropValue::List(items) = node.props.get(key)? else {
        return None;
    };
    Some(items.iter().map(parse_track).collect())
}

/// A [`PropValue::List`] of strings (`"areas"`). Non-text items are skipped.
pub(crate) fn string_list(node: &ViewNode, key: &str) -> Option<Vec<String>> {
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

/// Read one grid track from a described value — **through [`Track`]'s own parser**, never a second
/// copy of it.
///
/// A string goes to [`Track::from_str`](std::str::FromStr::from_str) and a bare number is pixels.
/// This function used to hold the whole vocabulary itself, which meant native Rust could not reach
/// any of it: a plugin wrote `"1fr"` while a `Grid` in the app had to write `Track::Fr(1.0)` by
/// hand, one variant per track. The parser lives on the type now, so both sides read the same word
/// the same way.
///
/// **Anything unrecognised degrades to [`Track::Auto`]** — never a panic, never an error. The model
/// is untrusted input (a plugin, an RPC caller), so a typo costs that author a differently-sized
/// track, not a broken host.
pub(crate) fn parse_track(value: &PropValue) -> Track {
    match value {
        PropValue::Text(s) => Track::from(s.as_str()),
        PropValue::Int(i) => Track::Px(*i as f32),
        PropValue::Float(f) => Track::Px(*f as f32),
        _ => Track::Auto,
    }
}

/// The handler for a **`"toggle"`** binding (a collapsible group) — the third canonical event name,
/// alongside `press` and `change`.
///
/// A toggle is only meaningful with its **new state**, so the bound intent is dispatched with
/// `args["expanded"]` set: an author binds one action and learns which way it went, instead of
/// having to track the group's state on their side.
pub(crate) fn toggle_change(node: &ViewNode, emit: &IntentEmitter) -> Option<impl Fn(Action) + 'static> {
    let intent = node.intent(ViewEvent::Toggle)?.clone();
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
