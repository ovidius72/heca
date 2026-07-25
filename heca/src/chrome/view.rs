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
    /// One selectable **option**: a `value` plus arbitrary composed content. The children of a
    /// [`Select`](WidgetKind::Select) / [`Tabs`](WidgetKind::Tabs) — and usable on its own.
    Choice,

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

impl WidgetKind {
    /// Every variant — the closed vocabulary, enumerable.
    ///
    /// This exists so the host can check **coverage**: `realize` has a test that walks this list and
    /// asserts each kind maps to a real widget, which is what stops a newly-added kind from silently
    /// rendering an empty container. Keep it in sync with the enum — [`ordinal`](Self::ordinal)
    /// makes that mechanical rather than a matter of discipline (see its docs).
    pub const ALL: &'static [WidgetKind] = &[
        WidgetKind::Column,
        WidgetKind::Row,
        WidgetKind::Grid,
        WidgetKind::Card,
        WidgetKind::Scroll,
        WidgetKind::Panel,
        WidgetKind::Surface,
        WidgetKind::ItemGroup,
        WidgetKind::DockFrame,
        WidgetKind::MarkerGroup,
        WidgetKind::Tabs,
        WidgetKind::Choice,
        WidgetKind::Label,
        WidgetKind::Button,
        WidgetKind::IconButton,
        WidgetKind::Badge,
        WidgetKind::BadgeButton,
        WidgetKind::Tag,
        WidgetKind::Icon,
        WidgetKind::Input,
        WidgetKind::Select,
        WidgetKind::Toggle,
        WidgetKind::Checkbox,
        WidgetKind::StatusDot,
        WidgetKind::Gauge,
        WidgetKind::ScrollBar,
        WidgetKind::Alert,
        WidgetKind::Toast,
        WidgetKind::RailCell,
        WidgetKind::Item,
    ];

    /// This kind's position in [`ALL`](Self::ALL).
    ///
    /// The match is **exhaustive**, so adding a variant to the enum without adding it here is a
    /// *compile error*; the `all_lists_every_widget_kind` test then compares the two, so adding it
    /// here without adding it to [`ALL`](Self::ALL) is a *test failure*. Between them, the list
    /// cannot silently fall behind the vocabulary — which is the whole point, since the coverage
    /// guard is only as good as the list it walks.
    fn ordinal(self) -> usize {
        match self {
            WidgetKind::Column => 0,
            WidgetKind::Row => 1,
            WidgetKind::Grid => 2,
            WidgetKind::Card => 3,
            WidgetKind::Scroll => 4,
            WidgetKind::Panel => 5,
            WidgetKind::Surface => 6,
            WidgetKind::ItemGroup => 7,
            WidgetKind::DockFrame => 8,
            WidgetKind::MarkerGroup => 9,
            WidgetKind::Tabs => 10,
            WidgetKind::Choice => 11,
            WidgetKind::Label => 12,
            WidgetKind::Button => 13,
            WidgetKind::IconButton => 14,
            WidgetKind::Badge => 15,
            WidgetKind::BadgeButton => 16,
            WidgetKind::Tag => 17,
            WidgetKind::Icon => 18,
            WidgetKind::Input => 19,
            WidgetKind::Select => 20,
            WidgetKind::Toggle => 21,
            WidgetKind::Checkbox => 22,
            WidgetKind::StatusDot => 23,
            WidgetKind::Gauge => 24,
            WidgetKind::ScrollBar => 25,
            WidgetKind::Alert => 26,
            WidgetKind::Toast => 27,
            WidgetKind::RailCell => 28,
            WidgetKind::Item => 29,
        }
    }
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
    /// An ordered list of values.
    ///
    /// Deliberately rare. The option-shaped widgets (`Select` / `Tabs`) do **not** use it — their
    /// options are **children**, because an option is a node with a value and content, not a string
    /// (see the "Options are children" section). What is genuinely list-shaped is a
    /// [`Grid`](WidgetKind::Grid)'s **track templates**: `columns` / `rows` (CSS-like strings —
    /// `"1fr"`, `"22px"`, `"auto"`) and `areas` (one string per grid row). That is what this exists
    /// for.
    List(Vec<PropValue>),
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

/// A node's event → intent bindings. The canonical event names (per plan §2.6.2):
///
/// - **`"press"`** — activated (buttons / rows / a standalone `Choice`).
/// - **`"change"`** — the value changed (`Input` / `Toggle` / `Checkbox`; on a `Select`/`Tabs` the
///   intent carries the chosen option's `value`).
/// - **`"toggle"`** — a collapsible group folded or unfolded (`ItemGroup`); the intent carries the
///   new state in `args["expanded"]`.
///
/// Keyed, so a node can bind several.
pub type Events = BTreeMap<String, Intent>;

/// A declarative widget node — one element of the serializable UI tree that both native code and
/// plugins author, and that [`realize`](super::realize) turns into a retained grid-ui
/// [`Component`](heca_grid_ui::Component).
///
/// # The model (SwiftUI/Flutter-style)
/// A node is **four things, all owned by *this* node**:
/// - [`kind`](Self::kind) — which widget it is ([`WidgetKind`]).
/// - [`props`](Self::props) — its **own** styling/content values ([`PropMap`] = `name → PropValue`).
///   Props are **per node**: `.prop("gap", …)` on a `Column` styles *the column*, not its children.
///   (That's why the props next to `.child(…)` calls look like "sibling" props — they belong to the
///   node you called `.prop` on, i.e. the container.)
/// - [`events`](Self::events) — its **own** event → [`Intent`] bindings. Behaviour is an action
///   **id** (+ args), never a Rust closure, so the tree stays serializable across the plugin boundary.
/// - [`children`](Self::children) — a plain **`Vec<ViewNode>`**, each a full node with its *own*
///   props / events / children. Composition is recursive: a child is styled exactly like its parent,
///   by putting props on *that child*.
///
/// The builder just chains for ergonomics; the children are a vector underneath — `.child(n)` appends
/// one, `.children([a, b])` appends many, so `Column().child(a).child(b)` ≡ `Column().children([a,b])`.
///
/// ```ignore
/// ViewNode::new(WidgetKind::Column)
///     .prop("gap", PropValue::Int(8))                       // ← the COLUMN's prop
///     .child(ViewNode::new(WidgetKind::Label).text("New name"))
///     .child(
///         ViewNode::new(WidgetKind::Input)
///             .text("current")
///             .prop("name", PropValue::Text("name".into())),  // ← the INPUT's props
///     )
///     .child(
///         ViewNode::new(WidgetKind::Button)
///             .text("Rename")
///             .prop("variant", PropValue::Variant(ViewVariant::Primary)) // ← the BUTTON's prop
///             .on_press(Intent::new("rename")),                          // ← the BUTTON's event
///     );
/// ```
///
/// # Layout props — every kind, no list
/// **Any field of [`Layout`](heca_grid_ui::Layout) is a prop on any kind**, named exactly as the
/// field is: `padding`, `margin` (+ per-side), `gap`, `gap_spacing`, `align`, `align_self`,
/// `justify`, `justify_items`, `justify_self`, `direction`, `width`, `height`, min/max sizes,
/// `flex_grow`, `flex_shrink`, `hidden`, `grid_cell`, `size`.
///
/// `realize` does **not** enumerate them — it merges by name against `Layout`'s own fields, so a
/// field added there is settable from a description with no change to the mapper. The counterpart
/// is that `Visual` (fill, border, glow, radius, font_size, font_scale) is not serializable, so
/// appearance is unreachable from a description by construction, not by a rule someone enforces.
///
/// Values read the way an author would write them: enums by **name** (`"center"`,
/// `"space_between"`, `"small"`), and a `Length` as a bare number (px), `"auto"`, or `"50%"`.
/// The merge lands **on top of** the constructed widget, so a widget's own constructor settings
/// survive any property it does not mention.
///
/// # Props & events by kind (what `realize` reads today)
/// Missing/mistyped props are ignored (the widget keeps its default) — the model is untrusted input,
/// so `realize` is total, and a bad value costs only itself: its neighbours on the same node still
/// apply. Below are the props a kind reads **in addition to** the layout set above:
///
/// | Kind | Props it reads | Events |
/// |------|----------------|--------|
/// | `Column` / `Row` | (layout only — see above) | — |
/// | `Card` | `text` (title) + children | — |
/// | `Surface` / `Panel` / `Scroll` | (container — children only) | — |
/// | `Label` | `text`, `bold`, `italic`, `underline`, `strikethrough` (Bool) | — |
/// | `Badge` / `Tag` / `Alert` | `text` | — |
/// | `Button` / `BadgeButton` | `text`, `variant`, `size` | `press` |
/// | `Icon` / `IconButton` / `RailCell` | `icon` (Glyph **name**), `size` | `press` (button/rail) |
/// | `Input` | `text` (value), `name` | `change` |
/// | `Toggle` | `on` (Bool), `name` | `change` |
/// | `Checkbox` | `checked` (Bool), `text` (label), `name` | `change` |
/// | `Gauge` | `value` (Float) | — |
/// | `StatusDot` | — | — |
/// | `Item` | `text` (label); **slots**: `leading` / `trailing` (no default slot) | `press` |
/// | `DockFrame` | `text` (title), `expanded` / `frameless` / `active` / `nav_selected` (Bool); **slot**: `header`, else body (default) | `toggle` |
/// | `Toast` | `text` (title), `severity`, `icon`, `body`, `action_text`, `dismissible` | `press` · `dismiss` · `action` |
/// | `Choice` | `value` (Text/Int), `text` (childless sugar) + children | `press` (standalone only) |
/// | `Select` / `Tabs` | `selected` (Int) + `Choice` children | `change` (carries the chosen **value**) |
/// | `ItemGroup` | `text` (header), `expanded` (Bool) + children (the rows) | `toggle` (carries the new `expanded`) |
/// | `MarkerGroup` | `active` (Bool), `nav_selected` (Bool) + children | — (an indicator) |
/// | `Grid` | `columns` / `rows` / `areas` (List of CSS-like strings); per-**child**: `area`, or `col`/`row`/`col_span`/`row_span` | — |
///
/// A **`"name"` prop** on a value widget (`Input`/`Toggle`/`Checkbox`) opts it into a submitted
/// modal's returned data (`ModalResult::Action { data }`, see `OverlayHost::open_modal`).
///
/// # Options are children (`Select` / `Tabs` / `Choice`)
/// An option is **a node with a value and arbitrary content**, and the options of a picker are its
/// **children** — never a `props["options"]` list of strings. That is what lets a declarative option
/// compose an icon + a label exactly like a native one:
///
/// ```ignore
/// ViewNode::new(WidgetKind::Select)
///     .prop("selected", PropValue::Int(1))
///     .on("change", Intent::new("set_level"))
///     .child(ViewNode::new(WidgetKind::Choice)
///         .prop("value", PropValue::Text("high".into()))
///         .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("lightning".into())))
///         .child(ViewNode::new(WidgetKind::Label).text("HIGH")));
/// ```
///
/// The widgets track a selected **index**, but an index is meaningless to a plugin and breaks when
/// the options are reordered — so `realize` maps it back through the options' `value` props and
/// fires the bound intent with **`args["value"]`** set (`{"value": "high"}`). An option with no
/// `value` falls back to `args["index"]`. A child of a `Select`/`Tabs` that is not a `Choice` is
/// ignored (realize is total for untrusted input). A childless `Choice` desugars `text` to a `Label`
/// child — children win, the same precedence as `Button`.
///
/// # Named child slots
/// A widget with **several places for children** (a `DockFrame`'s header vs body, an `Item`'s
/// leading vs trailing) needs no change to this shape: `children` stays one flat `Vec`, and the
/// **child** says where it goes with a **`slot` prop**. A widget may declare a *default* slot
/// (`DockFrame`'s body) — an unslotted child lands there; `Item` has none, so an unslotted child is
/// ignored. An unknown slot name is debug-logged and falls back to the default (or is ignored),
/// never a panic.
///
/// **Coverage**: every kind realizes to its widget except `ScrollBar`, which is **host-only** by
/// design (its state is live host signals, which static data cannot drive — use `Scroll`). The same
/// applies to individual builders that bind a host signal, e.g. `DockFrame::rail(..)`.
///
/// > Human-facing catalog version: `docs/widgets.md` → "Declarative UI model (`ViewNode`)". Keep
/// > both this rustdoc and that section in sync when adding a `WidgetKind` or a `realize` arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewNode {
    /// Which widget this node is — selects the `realize` arm + the props it reads.
    pub kind: WidgetKind,
    /// This node's **own** styling/content values (`name → PropValue`), ordered for deterministic
    /// (de)serialisation. Per-node: never inherited by children. See the "Props & events by kind"
    /// table above for what each `kind` reads.
    #[serde(default, skip_serializing_if = "PropMap::is_empty")]
    pub props: PropMap,
    /// This node's **own** event → [`Intent`] bindings (`"press"` = activate, `"change"` = value
    /// changed). The *only* way a node carries behaviour — an action id, not a closure.
    #[serde(default, skip_serializing_if = "Events::is_empty")]
    pub events: Events,
    /// Child nodes, in order. A **vector**, not a fixed slot: containers (`Column`/`Row`/`Card`/…)
    /// render them; leaves leave it empty. Each child is a full `ViewNode` with its own props/events.
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

    /// Set a property **on this node** (per-node — not inherited by children). Which keys a node
    /// reads depends on its [`kind`](Self::kind); see the "Props & events by kind" table on
    /// [`ViewNode`]. Setting an irrelevant key is harmless (ignored at realize time).
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

    /// Append a child node to the [`children`](Self::children) vec. The child is a full `ViewNode`
    /// with its own props/events — style it by putting props on *it*, not on the parent.
    pub fn child(mut self, child: ViewNode) -> Self {
        self.children.push(child);
        self
    }

    /// Append several children at once — `Column().children([a, b])` ≡ `.child(a).child(b)`.
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

    /// `ALL` must list the whole vocabulary — the realize **coverage guard** walks it, and a guard is
    /// only as good as the list it walks. `ordinal`'s match is exhaustive (a new variant fails to
    /// compile there); this ties the two together, so a variant that reaches `ordinal` but not `ALL`
    /// fails here.
    #[test]
    fn all_lists_every_widget_kind() {
        for (i, kind) in WidgetKind::ALL.iter().enumerate() {
            assert_eq!(
                kind.ordinal(),
                i,
                "{kind:?} is out of order in ALL (or missing from it)",
            );
        }
        let highest = WidgetKind::ALL
            .iter()
            .map(|k| k.ordinal())
            .max()
            .expect("the vocabulary is not empty");
        assert_eq!(
            WidgetKind::ALL.len(),
            highest + 1,
            "a variant exists that ALL does not list",
        );
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

    /// The whole `PropValue` vocabulary — including the nested [`PropValue::List`] a `Grid`'s track
    /// templates ride on — must survive the WASM boundary intact.
    #[test]
    fn every_prop_value_round_trips_including_lists() {
        let node = ViewNode::new(WidgetKind::Grid)
            .prop(
                "columns",
                PropValue::List(vec![
                    PropValue::Text("auto".into()),
                    PropValue::Text("1fr".into()),
                ]),
            )
            .prop(
                "areas",
                PropValue::List(vec![PropValue::Text("icon title".into())]),
            )
            .prop("flag", PropValue::Bool(true))
            .prop("count", PropValue::Int(3))
            .prop("ratio", PropValue::Float(0.5))
            .prop("tint", PropValue::Color("#ff00ff".into()))
            .prop("icon", PropValue::Glyph("terminal".into()))
            .prop("size", PropValue::Size(ViewSize::Small))
            .prop("align", PropValue::Align(ViewAlign::Center))
            .prop("variant", PropValue::Variant(ViewVariant::Ghost));

        let json = serde_json::to_string(&node).expect("serialize");
        let back: ViewNode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(node, back, "every PropValue must round-trip through JSON");
        assert!(
            matches!(back.props.get("columns"), Some(PropValue::List(items)) if items.len() == 2),
            "the list survives as a list: {json}",
        );
    }

    #[test]
    fn empty_maps_are_omitted_in_json() {
        let json = serde_json::to_string(&ViewNode::new(WidgetKind::Label).text("x")).unwrap();
        assert!(!json.contains("events"), "empty events omitted: {json}");
        assert!(!json.contains("children"), "empty children omitted: {json}");
    }
}
