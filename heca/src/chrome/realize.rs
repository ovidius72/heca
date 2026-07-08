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

use heca_grid_ui::{
    Align, Button, ButtonVariant, Component, Flex, HintExt, Label, LayoutExt, WidgetSize,
};

use super::view::{PropValue, ViewAlign, ViewNode, ViewSize, ViewVariant, WidgetKind};
use super::{ChromeIntentEmitter, HintTargetRegistry};
use crate::app::interaction::InteractionIntent;

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
) -> Box<dyn Component> {
    match node.kind {
        WidgetKind::Column => realize_flex(node, Flex::column(), emit, hints),
        WidgetKind::Row => realize_flex(node, Flex::row(), emit, hints),
        WidgetKind::Label => Box::new(Label::new(text_of(node))),
        WidgetKind::Button => realize_button(node, emit, hints),
        // Remaining kinds map to their grid-ui widget as the model grows (ui-4+). Until
        // then an unhandled kind is an empty container rather than a panic — the tree still
        // realizes, it just renders nothing for that node.
        other => {
            #[cfg(debug_assertions)]
            eprintln!("[heca] realize: unimplemented WidgetKind {other:?}");
            let _ = other;
            Box::new(Flex::empty())
        }
    }
}

/// Realize a container node onto a base [`Flex`] (row or column), applying layout props and
/// recursively realizing + attaching children.
fn realize_flex(
    node: &ViewNode,
    mut flex: Flex,
    emit: &ChromeIntentEmitter,
    hints: &mut HintTargetRegistry,
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
        flex.base_mut().children.push(realize(child, emit, hints));
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

/// A numeric prop (`Int` or `Float`) as `f32` — e.g. `"gap"`.
fn f32_prop(node: &ViewNode, key: &str) -> Option<f32> {
    match node.props.get(key)? {
        PropValue::Int(i) => Some(*i as f32),
        PropValue::Float(f) => Some(*f as f32),
        _ => None,
    }
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
        let root = realize(&confirm_tree(), &noop_emitter(), &mut hints);
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
        let _ = realize(&confirm_tree(), &noop_emitter(), &mut hints);
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

    /// An unhandled kind realizes to an empty container instead of panicking (the tree stays
    /// total for untrusted plugin/RPC input).
    #[test]
    fn unhandled_kind_is_empty_not_panic() {
        let mut hints = HintTargetRegistry::default();
        let node = ViewNode::new(WidgetKind::Gauge);
        let realized = realize(&node, &noop_emitter(), &mut hints);
        assert_eq!(realized.base().children.len(), 0);
        assert_eq!(hints.checkpoint(), 0, "an empty fallback registers no hints");
    }
}
