//! The per-kind mapping: one arm per [`WidgetKind`], building the live
//! widget it names.

use heca_grid_ui::{
    Action, Alert, Badge, BadgeButton, Button, Card, Checkbox, Component, DockFrame, Flex, Gauge,
    Glyph, Icon, IconButton, Input, Item, ItemGroup, KeyHintGroup, Label, LandingSlot, MarkerGroup,
    NfGlyph, NfIcon, Overlay, Panel, ProgressBar, RailCell, Row as GridRow, ScrollRegion, Select,
    Separator, Spinner, Splitter, StatusDot, Surface, Tabs, Tag, Theme, Toast, Toggle,
};
use heca_grid_ui::reactive::SignalGet;
use heca_view::{PropValue, ViewEvent, ViewNode, WidgetKind};

use crate::{FormBindings, IntentEmitter, realize};
use crate::intents::{change_intent, intent_carrier, press_intent, slot_of, warn_unknown_slot};
use crate::props::{glyph_prop, name_prop, option_values, text_of};
use crate::style::with_props;
use crate::vocab::{severity_prop, toast_position};
use crate::widgets::{
    button_of, option_change, realize_button, realize_card_grid, realize_choice, realize_flex,
    realize_grid, realize_options, toggle_change,
};

/// The per-kind mapping — see [`realize`], which wraps it with the props every node can carry.
pub(crate) fn realize_kind(
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
        WidgetKind::Surface => attach_children(Box::new(Surface::new()), node, theme, emit, forms),
        // `Panel` used to be an alias for `Surface`, which is why the published examples showed
        // `Panel::new().title(..)` against a widget that had no title. It is its
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
            let mut overlay = with_props(Overlay::new().panel(panel), node, theme);
            // **What the dismiss key means, declared like any other behaviour.** The overlay has
            // always held the keys; it answers this one, and this is the
            // described spelling — so a plugin's surface closes on Escape by saying what closing
            // means, and never by writing a key.
            if let Some(intent) = node.events.get(ViewEvent::Dismiss.name()) {
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
            Box::new(with_props(KeyHintGroup::new(subtree), node, theme))
        }
        WidgetKind::Scroll => {
            // `axes` reaches the widget through its own builder, so a declarative region can be
            // horizontal or two-axis — it was vertical-only for as long as this arm named its
            // properties by hand.
            let region = with_props(ScrollRegion::new(), node, theme);
            attach_children(Box::new(region), node, theme, emit, forms)
        }
        WidgetKind::Line => Box::new(with_props(heca_grid_ui::Line::new(), node, theme)),
        WidgetKind::Resizable => {
            let content: Box<dyn Component> = match node.children.len() {
                1 => realize(&node.children[0], theme, emit, forms),
                _ => attach_children(Box::new(Flex::column()), node, theme, emit, forms),
            };
            Box::new(with_props(heca_grid_ui::Resizable::new(content), node, theme))
        }
        WidgetKind::TreeLayout => {
            let mut children = node.children.iter();
            let parent = children.next().map(|child| realize(child, theme, emit, forms))
                .unwrap_or_else(|| Box::new(Flex::empty()));
            let mut tree = heca_grid_ui::TreeLayout::new(parent);
            for child in children { tree = tree.branch(realize(child, theme, emit, forms)); }
            Box::new(tree)
        }
        WidgetKind::MessageList => {
            let mut list = with_props(heca_grid_ui::MessageList::new(), node, theme);
            for child in &node.children { list = list.entry(realize(child, theme, emit, forms)); }
            Box::new(list)
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
        WidgetKind::LandingSlot => Box::new(with_props(LandingSlot::new(), node, theme)),
        // The drag is the widget's; what it means is the author's, so each report goes out as the
        // described intent with the distance travelled.
        WidgetKind::Splitter => {
            let mut splitter = with_props(Splitter::vertical(), node, theme);
            if let Some(intent) = node.intent(ViewEvent::Resize) {
                let (intent, emit) = (intent.clone(), emit.clone());
                splitter = splitter
                    .on_resize(move |delta| emit(intent.clone().arg("delta", delta.into())));
            }
            Box::new(splitter)
        }
        WidgetKind::Gauge => Box::new(with_props(Gauge::new(), node, theme)),
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
        // A tile only arranges, so every part is a child and says where it goes with `slot`.
        // A child with no slot is a line under the head — the default slot, since a tile may have
        // any number of lines and exactly one of each other part.
        WidgetKind::Tile => {
            let mut tile = with_props(heca_grid_ui::widgets::Tile::new(), node, theme);
            for child in &node.children {
                let realized = realize(child, theme, emit, forms);
                tile = match slot_of(child) {
                    Some("status") => tile.status(realized),
                    Some("icon") => tile.icon(realized),
                    Some("title") => tile.title(realized),
                    Some("suffix") => tile.suffix(realized),
                    None => tile.line(realized),
                    other => {
                        warn_unknown_slot(
                            node,
                            child,
                            other,
                            &["status", "icon", "title", "suffix"],
                        );
                        tile.line(realized)
                    }
                };
            }
            Box::new(tile)
        }
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
                forms.bind(
                    name.clone(),
                    Box::new(move || PropValue::Text(sig.get_untracked())),
                );
                // Also expose the live signal for reactive validation (disabled submit button).
                forms.bind_text_signal(name, sig);
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                input = input.on_change(move |_a: Action| emit(carrier.clone()));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Submit) {
                let emit = emit.clone();
                input = input.on_submit(move |action| {
                    if let heca_grid_ui::SignalData::String(value) = action.data {
                        emit(carrier.clone().arg("value", PropValue::Text(value)));
                    }
                });
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
                    Some("leading") => it = it.leading(realized),
                    Some("trailing") => it = it.trailing(realized),
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
                        PropValue::Text(
                            values.get(idx.get_untracked()).cloned().unwrap_or_default(),
                        )
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
                    Some("header") => dock = dock.header(realized),
                    None => dock = dock.child(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["header"]);
                        dock = dock.child(realized);
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
            // controls under it, one per child. The card once had no slots, because it
            // hand-drew itself and could not hold arbitrary content; it now lays out real children.
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
                        toast = toast.action(realized);
                    }
                    Some("body") | None => toast = toast.body(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["body", "actions"]);
                        toast = toast.body(realized);
                    }
                }
            }

            // **Children win over the text sugar**, the way a `Button`'s children win over its
            // `text`/`icon`: one content model, two spellings, never two paint paths. The text
            // props stay because they are the plain-data path a `ToastSpec` uses.
            if !described_actions
                && let Some(label) = node.props.get("action_text").and_then(PropValue::as_text)
                && let Some(carrier) = intent_carrier(node, ViewEvent::Action)
            {
                let emit = emit.clone();
                toast = toast.action(Button::new(label).on_click(move || emit(carrier.clone())));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Press) {
                let emit = emit.clone();
                toast = toast.on_click(move || emit(carrier.clone()));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Dismiss) {
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
pub(crate) fn attach_children(
    mut container: Box<dyn Component>,
    node: &ViewNode,
    theme: &Theme,
    emit: &IntentEmitter,
    forms: &mut FormBindings,
) -> Box<dyn Component> {
    for child in &node.children {
        container
            .base_mut()
            .children
            .push(realize(child, theme, emit, forms));
    }
    container
}
