//! What a property, an event and an intent are made of.

use serde::{Deserialize, Serialize};

use crate::{ViewAlign, ViewSize, ViewVariant};
use std::collections::BTreeMap;

// ── Scalars into property values ──────────────────────────────────────────────────────────
//
// So the authoring layer can write `.gap(8)` and `.bordered(true)` without naming a variant at
// every call. A string becomes `Text`; a colour is NOT inferred from one, because a colour has to
// say it is one — `.fill("accent")` goes through a setter that wraps it, so a theme token is never
// mistaken for a caption.

impl From<f32> for PropValue {
    fn from(v: f32) -> Self {
        PropValue::Float(v as f64)
    }
}

impl From<f64> for PropValue {
    fn from(v: f64) -> Self {
        PropValue::Float(v)
    }
}

impl From<i32> for PropValue {
    fn from(v: i32) -> Self {
        PropValue::Int(v as i64)
    }
}

impl From<i64> for PropValue {
    fn from(v: i64) -> Self {
        PropValue::Int(v)
    }
}

impl From<usize> for PropValue {
    fn from(v: usize) -> Self {
        PropValue::Int(v as i64)
    }
}

impl From<bool> for PropValue {
    fn from(v: bool) -> Self {
        PropValue::Bool(v)
    }
}

impl From<&str> for PropValue {
    fn from(v: &str) -> Self {
        PropValue::Text(v.to_string())
    }
}

impl From<String> for PropValue {
    fn from(v: String) -> Self {
        PropValue::Text(v)
    }
}

/// A serializable property value: scalars plus the semantic enums. Colors and glyphs are
/// carried as names/strings and resolved against the `Theme`/icon font at realize time, so
/// the model never embeds resolved pixels or theme state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Size(ViewSize),
    Variant(ViewVariant),
    Align(ViewAlign),
    /// `#rrggbb` / `#rrggbbaa`, or a theme color token name.
    Color(String),
    /// A Phosphor glyph name.
    Glyph(String),
    /// An ordered list of values.
    ///
    /// Deliberately rare. The option-shaped widgets (`Select` / `Tabs`) do **not** use it — their
    /// options are **children**, because an option is a node with a value and content, not a string
    /// (see the "Options are children" section). What is genuinely list-shaped is a
    /// [`Grid`](WidgetKind::Grid)'s **track templates**: `columns` / `rows` (CSS-like strings —
    /// `"1fr"`, `"22px"`, `"auto"`) and `areas` (one string per grid row). That is what this exists
    /// for.
    List(Vec<PropValue>),
    /// A **named group of values** — the shape a struct-valued property needs.
    ///
    /// This is the extension point for anything that is not a scalar. A widget property whose type
    /// has fields (`border`, `glow`) is written as a map of its field names, and the realize side
    /// hands it to the field's own deserializer:
    ///
    /// ```text
    /// border = { color: "accent", width: 2 }
    /// glow   = { color: "accent", radius: 12, intensity: 0.4 }
    /// ```
    ///
    /// It exists because the alternative was a new variant per struct. `border` and `glow` were
    /// **unreachable from a description for as long as this was missing** — F003/P017/T007 gave both
    /// types serde derives and its commit claimed "the whole of `Visual`", but serde on the type is
    /// not a value that can carry it, and nothing tested the two, so nothing failed. A future
    /// struct-shaped property needs no change here (F003/P011/T018).
    ///
    /// Values nest: a [`Color`](PropValue::Color) inside a map is still a theme token name and is
    /// still resolved against the live theme.
    Map(PropMap),
}

impl PropValue {
    /// Borrow the string if this is [`Text`](PropValue::Text).
    pub fn as_text(&self) -> Option<&str> {
        match self {
            PropValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The bool if this is [`Bool`](PropValue::Bool).
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The number if this is [`Float`](PropValue::Float) **or** [`Int`](PropValue::Int).
    ///
    /// Both, because a description is written by hand and over the wire: `0.5` and `1` are the same
    /// number of seconds to whoever wrote them, and JSON does not keep the two apart the way Rust
    /// does. Refusing the integer would fail a correct description for a reason its author cannot
    /// see — the scalar channel already merges them the same way (`prop_to_input`).
    pub fn as_float(&self) -> Option<f64> {
        match self {
            PropValue::Float(f) => Some(*f),
            PropValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

/// A named, ordered property bag (ordered for deterministic (de)serialisation).
pub type PropMap = BTreeMap<String, PropValue>;

/// An action a node emits: a routing **id** (a `WmAction` name for built-ins, or a plugin
/// action id) plus optional args resolved at dispatch time. This is the *only* way a
/// `ViewNode` carries behaviour — there are no closures — which keeps it serializable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub action: String,
    #[serde(default, skip_serializing_if = "PropMap::is_empty")]
    pub args: PropMap,
}

impl Intent {
    /// An intent that dispatches `action` with no args.
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            args: PropMap::new(),
        }
    }

    /// Add an argument.
    ///
    /// The value is an explicit [`PropValue`] rather than an `impl Into<PropValue>`, deliberately:
    /// an argument crosses to RPC and to a WASM plugin as data, so what it *is* should be visible
    /// at the call site rather than inferred from whatever integer type happened to be in scope.
    ///
    /// ```
    /// # use heca_view::{Intent, PropValue};
    /// Intent::new("focus_pane").arg("pane_id", PropValue::Int(7));
    /// Intent::new("rename").arg("name", PropValue::Text("scratch".into()));
    /// Intent::new("expand").arg("open", PropValue::Bool(true));
    /// ```
    ///
    /// # Integers are `i64` — and pane ids are `u64`
    ///
    /// [`PropValue::Int`] is an `i64`, because that is what JSON and the RPC wire carry. Most ids in
    /// this codebase (`ToastSpec::id`, a pane id, a notification id) are **`u64`**, and there is
    /// deliberately **no `From<u64>`**: the conversion is lossy above `i64::MAX`, and a silently
    /// wrapped id would arrive as a negative number that nothing could diagnose from the UI. Cast at
    /// the call site, where the choice is visible:
    ///
    /// ```
    /// # use heca_view::{Intent, PropValue};
    /// # let pane_id: u64 = 7;
    /// Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane_id as i64));
    /// ```
    ///
    /// # What may go in one
    ///
    /// Anything [`PropValue`] can hold — `Bool`, `Int`, `Float`, `Text`, a colour or glyph **name**,
    /// a `List`, or a `Map` for a named group of values. It may **not** hold a closure or a widget:
    /// an intent is the one form behaviour takes when it has to survive being sent by RPC, named in
    /// a keybinding, or raised by a plugin. That is the whole reason it exists — see [`Intent`].
    pub fn arg(mut self, key: impl Into<String>, value: PropValue) -> Self {
        self.args.insert(key.into(), value);
        self
    }
}

/// A node's event → intent bindings, keyed by the names [`ViewEvent`](crate::ViewEvent) lists — the
/// one place they are spelled and documented.
///
/// Keyed, so a node can bind several.
pub type Events = BTreeMap<String, Intent>;
