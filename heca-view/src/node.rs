//! The node, its builders and accessors.

use serde::{Deserialize, Serialize};

use crate::{DropdownItem, Events, Intent, PropMap, PropValue, ViewEvent, WidgetKind};

/// A declarative widget node — one element of the serializable UI tree that both native code and
/// plugins author, and that [`realize`](super::realize) turns into a retained grid-ui
/// [`Component`](heca_grid_ui::Component).
///
/// # The model (SwiftUI/Flutter-style)
/// A node is **five things, all owned by *this* node**:
/// - [`kind`](Self::kind) — which widget it is ([`WidgetKind`]).
/// - [`props`](Self::props) — its **own** styling/content values ([`PropMap`] = `name → PropValue`).
///   Props are **per node**: `.prop("gap", …)` on a `Column` styles *the column*, not its children.
///   (That's why the props next to `.child(…)` calls look like "sibling" props — they belong to the
///   node you called `.prop` on, i.e. the container.)
/// - [`events`](Self::events) — its **own** event → [`Intent`] bindings. Behaviour is an action
///   **id** (+ args), never a Rust closure, so the tree stays serializable across the plugin boundary.
/// - [`actions`](Self::actions) — **verbs it answers to by name**, each bound to an [`Intent`]. An
///   event is fired *at* a node by what the user did to it; an action is a name a key binding, the
///   palette or a script says out loud, and the node on screen that declares it is the one that
///   runs (`heca_grid_ui::fire_action`).
/// - [`children`](Self::children) — a plain **`Vec<ViewNode>`**, each a full node with its *own*
///   props / events / children. Composition is recursive: a child is styled exactly like its parent,
///   by putting props on *that child*.
///
/// The builder just chains for ergonomics; the children are a vector underneath — `.child(n)` appends
/// one and `.child([a, b])` appends many, so `Column().child(a).child(b)` ≡ `Column().child([a, b])`.
///
/// ```ignore
/// ViewNode::new(WidgetKind::VStack)
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
/// # Style props — every kind, no list
/// **Any field of [`Layout`](heca_grid_ui::Layout) or [`Visual`](heca_grid_ui::Visual) is a prop on
/// any kind**, named exactly as the field is. Layout: `padding`, `margin` (+ per-side), `gap`,
/// `gap_spacing`, `align`, `align_self`, `justify`, `justify_items`, `justify_self`, `direction`,
/// `width`, `height`, min/max sizes, `flex`, `order`, `flex_grow`, `flex_shrink`, `hidden`, `grid_cell`, `size`.
/// Appearance: `fill`, `border`, `glow`, `radius`, `font_size`, `font_scale`.
///
/// `realize` does **not** enumerate them — it merges by name against each half's own fields, so a
/// field added to either is settable from a description with no change to the mapper.
///
/// **Appearance became settable 2026-07-27 (F003/P017/T7).** `Visual` used to be unserializable on
/// purpose, so appearance was unreachable by construction. The theme is the default now, not a
/// wall: set nothing and you follow the theme, which is what most widgets should do.
///
/// A **colour** is a hex literal (`"#ff8800"`, `"#ff8800cc"`) or a **theme token name**
/// (`"accent"`, `"muted"`, `"danger"` — the theme's own colour fields, so the vocabulary is not a
/// list anyone maintains). A token resolves against the theme the tree is built with, and a theme
/// reload rebuilds the trees, so a token-named override follows the new theme. A hex literal does
/// not — it is exactly the colour it says. **Prefer a token name.**
///
/// Values read the way an author would write them: enums by **name** (`"center"`,
/// `"space_between"`, `"small"`), and a `Length` as a bare number (px), `"auto"`, or `"50%"`.
/// The merge lands **on top of** the constructed widget, so a widget's own constructor settings
/// survive any property it does not mention.
///
/// # Widget props — the widget's own builders decide, not a list here
/// `realize` names no widget property. Each widget generates its property surface from its own
/// builders (`#[prop]` in `heca-grid-ui`), so a capability added to a widget is settable from a
/// description the same day. Every builder must be classified `#[prop]` or `#[host_only("why")]`
/// — the build fails otherwise, which is what stops a capability going quietly missing the way
/// `Input::placeholder` and `ScrollRegion`'s second axis did.
///
/// **Properties are order-independent.** They are applied after children are attached, so a
/// builder that clamps against its children (`Select`/`Tabs` `selected`) sees the real ones.
/// Nothing an author, caller or agent has to think about.
///
/// Deliberately NOT properties, with the reason recorded on each builder: closures (behaviour
/// crosses as an [`Intent`]), composed content (use `children`), and builders bound to live host
/// signals. Appearance **used to be** in this group; it left on 2026-07-27 (F003/P017/T7).
///
/// # Props & events by kind
/// Missing/mistyped props are ignored (the widget keeps its default) — the model is untrusted input,
/// so `realize` is total, and a bad value costs only itself: its neighbours on the same node still
/// apply. The table below is a **reader's summary**; the widget's builders are the authority:
///
/// | Kind | Props it reads | Events |
/// |------|----------------|--------|
/// | `Column` / `Row` | (layout only — see above) | — |
/// | `Card` | `text` (title) + children | — |
/// | `Surface` | (container — children only) | — |
/// | `Panel` | `text` (the heading; omit it and no header row is drawn) + children | — |
/// | `Scroll` | `axes` (`vertical` \| `horizontal` \| `both`, default vertical) + children | — |
/// | `Label` | `text`, `bold`, `italic`, `underline`, `strikethrough` (Bool) | — |
/// | `Badge` / `Tag` / `Alert` | `text` | — |
/// | `Button` / `BadgeButton` | `text`, `variant`, `size` | `press` |
/// | `Icon` / `IconButton` / `RailCell` | `icon` (Glyph **name**), `size` | `press` (button/rail) |
/// | `Input` | `text` (the **value**), `placeholder`, `name` | `change` |
/// | `Toggle` | `on` (Bool), `name` | `change` |
/// | `Checkbox` | `checked` (Bool), `text` (label), `name` | `change` |
/// | `Gauge` | `value` (Float) | — |
/// | `StatusDot` | — | — |
/// | `Separator` | `orientation` (`horizontal` \| `vertical`, default horizontal), `length` (Float px; omit to stretch) | — |
/// | `Splitter` | `orientation` (`vertical` \| `horizontal`, default vertical), `line` (Bool) | `resize` (arg `delta`, Float px) |
/// | `LandingSlot` | `label` (the letter), `filled` (Bool), `while_dragging` (the drag kind it appears for), `accepting` (the drag kind it takes), `edge` (Bool: a line, not a place) | — |
/// | `Item` | `text` (label); **slots**: `leading` / `trailing` (no default slot) | `press` |
/// | `DockFrame` | `text` (title), `expanded` / `frameless` / `active` / `nav_selected` (Bool); **slot**: `header`, else body (default) | `toggle` |
/// | `Toast` | `text` (title), `severity`, `icon`, `body`, `action_text`, `dismissible` | `press` · `dismiss` · `action` |
/// | `Choice` | `value` (Text/Int), `text` (childless sugar) + children | `press` (standalone only) |
/// | `Select` / `Tabs` | `selected` (Int) + `Choice` children | `change` (carries the chosen **value**) |
/// | `ItemGroup` | `text` (header), `expanded` (Bool) + children (the rows) | `toggle` (carries the new `expanded`) |
/// | `MarkerGroup` | `active` (Bool), `nav_selected` (Bool) + children | — (an indicator) |
/// | `Grid` | `columns` / `rows` / `areas` (List of CSS-like strings); per-**child**: `area`, or `col`/`row`/`col_span`/`row_span` | — |
/// | `KeyHintGroup` | `opens_on` (the **verb** that opens the picker) + children | — |
///
/// Every kind also takes a **`hint`** event (what a `prefix+/` pick does to it) and an **`actions`**
/// map (verbs it answers to by name) — both universal, both read once for every kind.
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
///     .on(ViewEvent::Change, Intent::new("set_level"))
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
    /// **The menu this node opens on a right-click** — a declaration, exactly as `press` is
    /// (F003/P097/T501).
    ///
    /// ⚠️ **A menu is not a widget kind, and must not become one.** Natively it is one builder on
    /// *any* widget (`ComponentExt::context_menu`) — no row identity, no path string, no registered
    /// builder, no anchor: the framework takes the anchor from whatever triggered it, and owns the
    /// dismissal and the keyboard half. A described node says the same thing the same way, so the
    /// two authoring paths converge instead of drifting. A plugin made to assemble a menu out of
    /// parts is writing the second path by hand, and will get the anchor, the dismissal and the
    /// keys only approximately right (⭐⭐ RULE ZERO — one door, never two).
    ///
    /// Each entry carries an [`Intent`], so a plugin's menu dispatches **its own** registered
    /// actions and not only heca's — and every entry goes through the one dispatch door, so the
    /// interaction policy and the confirm gate apply exactly as they would for a keypress.
    ///
    /// Empty means no menu, which is also what "nothing declared" means natively: a right-click
    /// with nothing declared opens nothing, and bubbling stops at the nearest declaration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu: Vec<DropdownItem>,
    /// This node's **own** named verbs (`name` → [`Intent`]) — the declarative spelling of
    /// `ComponentExt::on_action`, and how a described surface owns a verb of its own instead of
    /// borrowing one the app already compiled in (F003/P082/T436).
    ///
    /// **Not the same thing as an event.** An event is fired *at* this node by something the user
    /// did to it (`press`, `change`); an action is a name said out loud — by a key binding
    /// (`[[keys.surface]]`), by the palette, over RPC — and answered by whichever node on screen
    /// declares it. Reachability is the whole of the gate: a verb whose surface is not up resolves
    /// to nothing.
    ///
    /// Namespace it the way a provider's actions are (`mypanel.reload`), because the binding names
    /// exactly this string.
    #[serde(default, skip_serializing_if = "Events::is_empty")]
    pub actions: Events,
    /// Child nodes, in order. A **vector**, not a fixed slot: containers (`Column`/`Row`/`Card`/…)
    /// render them; leaves leave it empty. Each child is a full `ViewNode` with its own props/events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewNode>,
}

/// **One node or many** — what every `child` builder takes, so a caller hands over whichever shape
/// they happen to hold and never goes looking for a plural spelling.
pub trait IntoNodes {
    /// The nodes.
    fn into_nodes(self) -> Vec<ViewNode>;
}

impl<N: Into<ViewNode>> IntoNodes for N {
    fn into_nodes(self) -> Vec<ViewNode> {
        vec![self.into()]
    }
}

impl<N: Into<ViewNode>> IntoNodes for Vec<N> {
    fn into_nodes(self) -> Vec<ViewNode> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<N: Into<ViewNode>, const K: usize> IntoNodes for [N; K] {
    fn into_nodes(self) -> Vec<ViewNode> {
        self.into_iter().map(Into::into).collect()
    }
}

impl ViewNode {
    /// **Open this menu when the node is right-clicked** — the described spelling of
    /// `ComponentExt::context_menu`, and available on every kind for the same reason it is on every
    /// widget (F003/P097/T501).
    ///
    /// ```ignore
    /// ViewNode::new(WidgetKind::Row)
    ///     .menu([
    ///         DropdownItem::with_intent("close", "Close", Intent::new("docker.stop").arg("id", id)),
    ///         DropdownItem::new("rename", "Rename").danger(false),
    ///     ])
    /// ```
    ///
    /// The framework anchors it where the click landed, dismisses it, and gives it the keyboard —
    /// an author writes none of that, exactly as a native caller does not.
    pub fn menu(mut self, items: impl IntoIterator<Item = DropdownItem>) -> Self {
        self.menu = items.into_iter().collect();
        self
    }

    /// A new node of `kind` with no props/events/children. (The ergonomic SwiftUI-style
    /// builder is a separate task, plugin-task-ui-2; these are the minimal constructors.)
    pub fn new(kind: WidgetKind) -> Self {
        Self {
            kind,
            props: PropMap::new(),
            events: Events::new(),
            menu: Vec::new(),
            actions: Events::new(),
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

    /// Convenience: set the `"key"` prop — **this node's identity, when it is one of a collection
    /// you are iterating**.
    ///
    /// The same `key` a native tree declares with `ComponentExt::key`, and it means React's `key`:
    /// the identity of the *thing this node represents*, taken from your data, so the framework can
    /// tell "this row again" from "a different row" after the tree is rebuilt. `realize` writes it
    /// into the widget's own slot, so a described row and a native row are identified alike.
    ///
    /// **Where it is required: in a collection, and nowhere else.** An ordinary node — a button, an
    /// icon, a card — needs nothing; its identity is derived from its content. What derivation
    /// cannot do is tell apart several nodes that read the same, which is exactly what iterating
    /// produces.
    ///
    /// **You never count.** A key is never a position and never a counter — an index is precisely
    /// the thing that changes when the list changes, which is what identity exists to survive.
    ///
    /// ```ignore
    /// for pane in panes {
    ///     Row::new().key(pane.id).on_press(Intent::new("focus_pane"))
    /// }
    /// ```
    pub fn key(self, k: impl Into<String>) -> Self {
        self.prop("key", PropValue::Text(k.into()))
    }

    /// This node's declared identity, if it carries one — see [`key`](Self::key).
    pub fn declared_key(&self) -> Option<&str> {
        match self.props.get("key") {
            Some(PropValue::Text(k)) => Some(k.as_str()),
            _ => None,
        }
    }

    /// Bind an event to an intent (e.g. `.on(ViewEvent::Press, Intent::new("close"))`).
    pub fn on(mut self, event: impl Into<String>, intent: Intent) -> Self {
        self.events.insert(event.into(), intent);
        self
    }

    /// **Declare a verb this node answers to**, by name — `.on_action("mypanel.reload", …)`.
    ///
    /// The declarative `ComponentExt::on_action`: the node names the verb, config names the key.
    ///
    /// ```ignore
    /// // [[keys.surface]] name = "mypanel" / reload = "r"   →   mypanel.reload
    /// Panel::new().on_action("mypanel.reload", Intent::new("docker.refresh"))
    /// ```
    pub fn on_action(mut self, name: impl Into<String>, intent: Intent) -> Self {
        self.actions.insert(name.into(), intent);
        self
    }

    /// Convenience: bind the `"press"` (activation) event.
    pub fn on_press(self, intent: Intent) -> Self {
        self.on(ViewEvent::Press, intent)
    }

    /// Convenience: bind the `"hint"` event — **what a leader-key pick (`prefix+/`) does to this
    /// node**, when that is not simply what a click does.
    ///
    /// Unbound, a pick falls back to [`press`](Self::on_press), so every actionable node is
    /// reachable by letter for free. Bind it when the two genuinely differ: heca's sidebar row
    /// activates the pane and leaves the sidebar on a click, and stays in the sidebar on a hint pick.
    pub fn on_hint(self, intent: Intent) -> Self {
        self.on(ViewEvent::Hint, intent)
    }

    /// **Append a child, or several** — one `ViewNode`, or a `Vec`/array of them.
    ///
    /// The child is a full `ViewNode` with its own props/events — style it by putting props on
    /// *it*, not on the parent.
    ///
    /// One door, as on the native side: a `child` / `children` pair is two names for one idea, and
    /// a caller reaches for whichever they saw first.
    pub fn child(mut self, children: impl IntoNodes) -> Self {
        self.children.extend(children.into_nodes());
        self
    }

    /// Whether this node emits an activation intent — an actionable target. `realize` makes such a
    /// node pickable by `prefix+/` even when it binds no `hint` of its own.
    pub fn is_actionable(&self) -> bool {
        self.events.contains_key(ViewEvent::Press.name())
    }

    /// The intent bound to `event`, if any.
    pub fn intent(&self, event: impl AsRef<str>) -> Option<&Intent> {
        self.events.get(event.as_ref())
    }
}

/// A described node that carries a `press` while sitting in a **collection** with no
/// [`key`](ViewNode::key) — the identity rule broken in the one place it is required.
///
/// Reported by [`unkeyed_collection_items`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnkeyedItem {
    /// Where it is: the child indices from the root down to it. A description has no file and no
    /// line, so this is the only way to point at one node in it.
    pub path: Vec<usize>,
    /// Which widget it is — and, because the siblings it clashes with are the same kind, what the
    /// collection is made of.
    pub kind: WidgetKind,
    /// The action its `press` names. An author recognises their own tree by this long before they
    /// recognise a path.
    pub action: String,
    /// How many siblings of this kind are in the collection, this one included.
    pub siblings: usize,
}

/// Every node in `root` that binds a `press` inside a collection and declares no
/// [`key`](ViewNode::key).
///
/// **This is the enforcement that reaches a plugin author** — the only one of the identity rule's
/// three that does. The other two are for us: a warning over a live widget tree
/// (`heca_grid_ui::nav::ambiguous_identities`) and a test over heca's own chrome. Someone whose UI
/// is JSON or a WASM module never meets the Rust compiler and never reads our test suite, so
/// without this their rows silently lose their cursor position and their hint letter on every
/// rebuild, and nothing anywhere says why.
///
/// **A collection is two or more direct children of one container with the same
/// [`WidgetKind`].** That mirrors the native condition — two or more unkeyed children deriving the
/// same name — with the stronger signal a description happens to carry: `WidgetKind` is real type
/// information, so `[Icon, Label]` is a composed control by construction and `[Row, Row, Row]` is a
/// list by construction, with nothing to infer about the author's intent.
///
/// **Only an actionable node is reported.** Identity is what a cursor, a right-click, a drag and a
/// remembered hint letter are kept *on*, and all four need something to act on. Three decorative
/// labels in a row lose nothing by being anonymous; three rows you can press lose all four.
///
/// A **keyed** node is skipped — it said who it is. So the fix is always the same one line, on the
/// item, from the data already being iterated:
///
/// ```ignore
/// for pane in panes {
///     Row::new().key(pane.id).on_press(Intent::new("focus_pane"))   // ← .key(…)
/// }
/// ```
///
/// It is **pure data, and it lives in this crate on purpose**: `heca-view` compiles without
/// anything that draws, so a plugin's own build can run this check against its own tree, long
/// before a host ever realizes it. The host runs it too — once per description, at the bridge —
/// which is also why it is not folded into `realize`: a description is realized again on every
/// theme reload and every plugin update, and a diagnostic that repeats on each of those is one
/// nobody reads.
pub fn unkeyed_collection_items(root: &ViewNode) -> Vec<UnkeyedItem> {
    let mut out = Vec::new();
    walk_unkeyed(root, &mut Vec::new(), &mut out);
    out
}

fn walk_unkeyed(node: &ViewNode, path: &mut Vec<usize>, out: &mut Vec<UnkeyedItem>) {
    // How many direct children share each kind — the collections this container holds.
    let mut counts: Vec<(WidgetKind, usize)> = Vec::new();
    for child in &node.children {
        match counts.iter_mut().find(|(k, _)| *k == child.kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((child.kind, 1)),
        }
    }

    for (i, child) in node.children.iter().enumerate() {
        let siblings = counts
            .iter()
            .find(|(k, _)| *k == child.kind)
            .map(|(_, n)| *n)
            .unwrap_or(1);
        path.push(i);
        if let Some(intent) = child
            .intent(ViewEvent::Press)
            .filter(|_| siblings >= 2 && child.declared_key().is_none())
        {
            out.push(UnkeyedItem {
                path: path.clone(),
                kind: child.kind,
                action: intent.action.clone(),
                siblings,
            });
        }
        walk_unkeyed(child, path, out);
        path.pop();
    }
}
