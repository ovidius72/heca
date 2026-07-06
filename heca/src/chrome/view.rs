//! `ViewNode` — the declarative, serializable widget-tree model (plugin-task-ui-1).
//!
//! This is the **app-wide** UI description: any UI — native chrome, overlays, and plugin
//! panels — can be expressed as a tree of `ViewNode`s and turned into retained grid-ui
//! [`Component`](heca_grid_ui::Component)s by the host mapper `realize()` (plugin-task-ui-3).
//! It is **generic** (its [`WidgetKind`] covers the whole grid-ui vocabulary) and fully
//! **serializable** (serde), so the exact same model authored in Rust is what a WASM plugin
//! ships over the boundary.
//!
//! Behaviour is expressed **only** through [`Intent`]s (an action id + args), never Rust
//! closures — so the model stays serializable and uniform for native and plugin UI alike.
//! A node that carries an `on_press`/`on_change` intent is *actionable*; `realize` gives
//! every actionable node a KeyHint target automatically, so `prefix+/` reaches it for free.
//!
//! Adding a widget = one [`WidgetKind`] variant + one arm in `realize`. Nothing here holds
//! layout or paint logic — this is pure description.
//!
//! Seam module: consumed by `realize` (plugin-task-ui-3), the Modal `body` (ui-4) and
//! plugins — carries `#![allow(dead_code)]` like the other chrome seam modules until then.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The closed widget vocabulary. Covers the whole grid-ui set: **containers** hold
/// children, **leaves** are terminal. `realize` maps each to its grid-ui widget (arms are
/// filled in incrementally, starting with what the confirm dialog needs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    // ── Containers ──
    Column,
    Row,
    Grid,
    Card,
    Scroll,
    Panel,
    Surface,
    /// Grouped list of items (`ItemGroup`).
    ItemGroup,
    /// A titled, collapsible dock frame.
    DockFrame,
    /// A row of column/pane markers.
    MarkerGroup,
    /// A tab strip + panel.
    Tabs,

    // ── Leaves ──
    Label,
    Button,
    IconButton,
    Badge,
    BadgeButton,
    Tag,
    Icon,
    Input,
    Select,
    Toggle,
    Checkbox,
    StatusDot,
    Gauge,
    ScrollBar,
    Alert,
    Toast,
    RailCell,
    /// A single selectable list row.
    Item,
}

/// Semantic size variant — mirrors grid-ui `WidgetSize`; `realize` maps it across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSize {
    Small,
    Normal,
    Large,
    Header,
}

/// Semantic visual variant — mirrors grid-ui `ButtonVariant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewVariant {
    Primary,
    Secondary,
    Destructive,
    Outline,
    Ghost,
    Link,
}

/// Semantic cross-axis alignment — mirrors grid-ui `Align` (the plan's `align` prop enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewAlign {
    Start,
    Center,
    End,
    Stretch,
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
    pub fn arg(mut self, key: impl Into<String>, value: PropValue) -> Self {
        self.args.insert(key.into(), value);
        self
    }
}

/// A node's event → intent bindings. Canonical event names (per plan §2.6.2): `"press"`
/// (activate — buttons/rows), `"change"` (value changed — input/toggle/select). Keyed so a
/// node can bind several.
pub type Events = BTreeMap<String, Intent>;

/// A declarative widget node: a [`kind`](WidgetKind), its `props`, its `events`, and its
/// `children`. Fully serializable; realized to a retained `Component` by `realize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewNode {
    pub kind: WidgetKind,
    #[serde(default, skip_serializing_if = "PropMap::is_empty")]
    pub props: PropMap,
    #[serde(default, skip_serializing_if = "Events::is_empty")]
    pub events: Events,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewNode>,
}

impl ViewNode {
    /// A new node of `kind` with no props/events/children. (The ergonomic SwiftUI-style
    /// builder is a separate task, plugin-task-ui-2; these are the minimal constructors.)
    pub fn new(kind: WidgetKind) -> Self {
        Self {
            kind,
            props: PropMap::new(),
            events: Events::new(),
            children: Vec::new(),
        }
    }

    /// Set a property.
    pub fn prop(mut self, key: impl Into<String>, value: PropValue) -> Self {
        self.props.insert(key.into(), value);
        self
    }

    /// Convenience: set the `"text"` prop (labels, buttons, tags…).
    pub fn text(self, s: impl Into<String>) -> Self {
        self.prop("text", PropValue::Text(s.into()))
    }

    /// Bind an event to an intent (e.g. `.on("press", Intent::new("close"))`).
    pub fn on(mut self, event: impl Into<String>, intent: Intent) -> Self {
        self.events.insert(event.into(), intent);
        self
    }

    /// Convenience: bind the `"press"` (activation) event.
    pub fn on_press(self, intent: Intent) -> Self {
        self.on("press", intent)
    }

    /// Append a child.
    pub fn child(mut self, child: ViewNode) -> Self {
        self.children.push(child);
        self
    }

    /// Append several children.
    pub fn children(mut self, children: impl IntoIterator<Item = ViewNode>) -> Self {
        self.children.extend(children);
        self
    }

    /// Whether this node emits an activation intent — an actionable target. `realize` uses
    /// this to attach a KeyHint target so `prefix+/` can reach it.
    pub fn is_actionable(&self) -> bool {
        self.events.contains_key("press")
    }

    /// The intent bound to `event`, if any.
    pub fn intent(&self, event: &str) -> Option<&Intent> {
        self.events.get(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small confirm-dialog-shaped tree: a column with a message + two action buttons.
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
                            .prop("size", PropValue::Size(ViewSize::Normal))
                            .on_press(Intent::new("confirm_ok")),
                    ),
            )
    }

    #[test]
    fn actionable_reflects_click_binding() {
        let btn = ViewNode::new(WidgetKind::Button)
            .text("OK")
            .on_press(Intent::new("ok"));
        assert!(btn.is_actionable());
        assert_eq!(btn.intent("press").unwrap().action, "ok");
        assert!(!ViewNode::new(WidgetKind::Label).text("hi").is_actionable());
    }

    #[test]
    fn json_round_trips_wasm_ready() {
        let tree = confirm_tree();
        let json = serde_json::to_string(&tree).expect("serialize");
        let back: ViewNode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tree, back, "ViewNode must round-trip through JSON (WASM boundary)");
        // Spot-check the shape survived.
        assert_eq!(back.kind, WidgetKind::Column);
        assert_eq!(back.children.len(), 2);
        assert!(back.children[1].children[1].is_actionable());
    }

    #[test]
    fn empty_maps_are_omitted_in_json() {
        let json = serde_json::to_string(&ViewNode::new(WidgetKind::Label).text("x")).unwrap();
        assert!(!json.contains("events"), "empty events omitted: {json}");
        assert!(!json.contains("children"), "empty children omitted: {json}");
    }
}
