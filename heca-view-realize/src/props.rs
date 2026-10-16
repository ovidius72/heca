//! Reading a node's props: text, numbers, values, names, glyphs, variants and sizes.

use heca_grid_ui::{ButtonVariant, Glyph, WidgetSize};
use heca_view::{PropValue, ViewNode, WidgetKind};

use crate::vocab::{map_size, map_variant};

// ── Prop readers ──────────────────────────────────────────────────────────────────────
// Small helpers that pull a typed value out of the node's prop bag. Missing/mistyped props
// are simply absent (the widget keeps its default), never an error — the model is untrusted
// input (plugins/RPC) so realize is total.

/// The `"text"` prop as an owned string (empty if absent) — labels, button captions, tags.
pub(crate) fn text_of(node: &ViewNode) -> String {
    node.props
        .get("text")
        .and_then(PropValue::as_text)
        .unwrap_or("")
        .to_string()
}

/// An index prop (`"selected"`) as a `usize`. Negative values are ignored (the widget keeps its
/// default) rather than wrapping — the model is untrusted input.
pub(crate) fn usize_prop(node: &ViewNode, key: &str) -> Option<usize> {
    match node.props.get(key)? {
        PropValue::Int(i) => usize::try_from(*i).ok(),
        _ => None,
    }
}

/// An option's `"value"` prop — what the option *means*, as opposed to what it shows. `Text` or
/// `Int`; any other type is ignored. Kept as a [`PropValue`] so the type survives the round trip
/// into the change intent's args.
pub(crate) fn value_prop(node: &ViewNode) -> Option<PropValue> {
    match node.props.get("value")? {
        v @ (PropValue::Text(_) | PropValue::Int(_)) => Some(v.clone()),
        _ => None,
    }
}

/// An option value as the plain string the widget stores (`heca-grid-ui` never depends on the app,
/// so a `Choice` carries a `String`; the richer `PropValue` is re-attached at the boundary — see
/// [`option_change`](crate::widgets::option_change)).
pub(crate) fn value_string(value: &PropValue) -> String {
    match value {
        PropValue::Text(s) => s.clone(),
        PropValue::Int(i) => i.to_string(),
        _ => String::new(),
    }
}

/// The option **values** of a `Select`/`Tabs` node, in child order — the strings its `Choice`
/// children carry. A named `Select` marshals the value at the chosen index into the form `data`
/// (mirrors [`realize_choice`](crate::widgets::realize_choice)'s own value derivation: explicit `value` prop,
/// else the label).
pub(crate) fn option_values(node: &ViewNode) -> Vec<String> {
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
pub(crate) fn name_prop(node: &ViewNode) -> Option<String> {
    node.props
        .get("name")
        .and_then(PropValue::as_text)
        .map(str::to_string)
}

/// The icon glyph from the `"icon"` (or `"glyph"`) prop, resolved from its Phosphor name.
pub(crate) fn glyph_prop(node: &ViewNode) -> Option<Glyph> {
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
/// NB: this is a **curated stopgap of ~35 icons**. The complete icon set and a generated
/// name↔`Glyph` mapping (so this cannot drift from the enum) are planned.
pub(crate) fn glyph_from_name(name: &str) -> Option<Glyph> {
    // **Looked up, not listed.** This was a hand-written `match` of 36 arms beside a `Glyph::ALL`
    // of 52, so sixteen glyphs — `caret_left`, `pencil`, `x_circle`, `folder_simple_plus` and the
    // rest — could not be named from a description at all and silently rendered nothing. A second
    // copy of a mapping drifts; `Glyph::name` is the one source and a new glyph cannot compile
    // without joining it.
    Glyph::ALL.iter().copied().find(|g| g.name() == name)
}

/// The `"variant"` prop mapped to the grid-ui [`ButtonVariant`].
pub(crate) fn variant_prop(node: &ViewNode) -> Option<ButtonVariant> {
    match node.props.get("variant")? {
        PropValue::Variant(v) => Some(map_variant(*v)),
        _ => None,
    }
}

/// The `"size"` prop mapped to the grid-ui [`WidgetSize`].
pub(crate) fn size_prop(node: &ViewNode) -> Option<WidgetSize> {
    match node.props.get("size")? {
        PropValue::Size(s) => Some(map_size(*s)),
        _ => None,
    }
}
