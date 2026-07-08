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
//! Adding a widget = one [`WidgetKind`](super::WidgetKind) arm here (+ its variant in the
//! model). Arms are filled in incrementally, starting with what the confirm dialog needs
//! (Column / Row / Label / Button); unhandled kinds fall back to an empty container.
//!
//! Seam module (consumed by the OverlayHost/Modal body in ui-4 and plugin panels) — carries
//! `#![allow(dead_code)]` like the sibling chrome seam modules until those consumers land.
#![allow(dead_code)]

use heca_grid_ui::reactive::SignalGet;
use heca_grid_ui::{
    Action, Alert, Align, Badge, BadgeButton, Button, ButtonVariant, Card, Checkbox, Component,
    Flex, Gauge, Glyph, HintExt, HintTargetId, Icon, IconButton, Input, Item, Label, LayoutExt,
    RailCell, ScrollRegion, StatusDot, Surface, Tag, Toggle, WidgetSize,
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
}

impl FormBindings {
    fn bind(&mut self, name: String, reader: FieldReader) {
        self.fields.push((name, reader));
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
        WidgetKind::Label => Box::new(Label::new(text_of(node))),
        WidgetKind::Button => realize_button(node, emit, hints),
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
                forms.bind(name, Box::new(move || PropValue::Text(sig.get_untracked())));
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

        // Structured / host-driven kinds still need model support the scalar description
        // can't express yet — list props (Select options, Tabs labels), track config (Grid),
        // markers (MarkerGroup), or host wiring (ScrollBar, Toast). Tracked as `plugin-task-ui-9`
        // (likely folded into the composition-first pass `ui-7`). Until then these realize to an
        // empty container (total for untrusted input) rather than a wrong guess.
        WidgetKind::Grid
        | WidgetKind::ItemGroup
        | WidgetKind::DockFrame
        | WidgetKind::MarkerGroup
        | WidgetKind::Tabs
        | WidgetKind::Select
        | WidgetKind::ScrollBar
        | WidgetKind::Toast => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] realize: WidgetKind {:?} needs structured model support (ui-7) — empty",
                node.kind
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
    if let Some(align) = align_prop(node) {
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
fn realize_button(
    node: &ViewNode,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
) -> Box<dyn Component> {
    let mut button = Button::new(text_of(node));
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
    Box::new(button)
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
        _ => return None,
    };
    Some(g)
}

/// The `"align"` prop mapped to the grid-ui [`Align`].
fn align_prop(node: &ViewNode) -> Option<Align> {
    match node.props.get("align")? {
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

    /// A still-deferred structured kind (needs list/track model support) realizes to an empty
    /// container instead of panicking — the tree stays total for untrusted plugin/RPC input.
    #[test]
    fn deferred_kind_is_empty_not_panic() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Tabs);
        let realized = realize(&node, &noop_emitter(), &mut hints, &mut FormBindings::default());
        assert_eq!(realized.base().children.len(), 0);
        assert_eq!(hints.checkpoint(), 0, "an empty fallback registers no hints");
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
}
