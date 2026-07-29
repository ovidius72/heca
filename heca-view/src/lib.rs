//! `ViewNode` — the declarative, serializable widget-tree model (plugin-task-ui-1).
//!
//! This is the **app-wide** UI description: any UI — native chrome, overlays, and plugin
//! panels — can be expressed as a tree of `ViewNode`s and turned into retained grid-ui
//! `Component`s by the mapper `realize()` in `heca-view-realize` (plugin-task-ui-3).
//! It is **generic** (its [`WidgetKind`] covers the whole grid-ui vocabulary) and fully
//! **serializable** (serde), so the exact same model authored in Rust is what a WASM plugin
//! ships over the boundary.
//!
//! This crate carries **no dependency but serde** — no widget library, no renderer, no taffy.
//! That is the point of it living apart from the app (F003/P017/T009): a plugin can depend on
//! the vocabulary without compiling the thing that draws it.
//!
//! Behaviour is expressed **only** through [`Intent`]s (an action id + args), never Rust
//! closures — so the model stays serializable and uniform for native and plugin UI alike.
//! A node that carries an `on_press`/`on_change` intent is *actionable*; `realize` gives
//! every actionable node a KeyHint target automatically, so `prefix+/` reaches it for free.
//!
//! Adding a widget = one [`WidgetKind`] variant + one arm in `realize`. Nothing here holds
//! layout or paint logic — this is pure description.
//!
//! Consumed by `realize` (plugin-task-ui-3), the Modal `body` (ui-4) and plugins. Everything
//! here is `pub`: it is the published vocabulary, so there is nothing to mark dead.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod build;

/// The closed widget vocabulary. Covers the whole grid-ui set: **containers** hold
/// children, **leaves** are terminal. `realize` maps each to its grid-ui widget (arms are
/// filled in incrementally, starting with what the confirm dialog needs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    // ── Containers ──
    /// A vertical box. Plain arrangement — no focus, no hover, no activation.
    VStack,
    /// A horizontal box. Plain arrangement — see [`Row`](WidgetKind::Row) for the interactive one.
    HStack,
    /// A **clickable, selectable** container for arbitrary content: hover tint, active state with
    /// a marker, press flash, focus ring, activation by mouse and by Enter/Space.
    ///
    /// This is `heca_grid_ui::Row`. The name used to belong to the plain horizontal box, which is
    /// now [`HStack`](WidgetKind::HStack) — so the library's `Row` and this one finally mean the
    /// same thing. Before that rename the interactive row had no declarative spelling at all, and
    /// `docs/chrome-and-ui.md` shipped an example writing this widget's behaviour against the box
    /// that cannot do it.
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
    /// A thin themed divider line.
    Separator,
}

impl WidgetKind {
    /// Every variant — the closed vocabulary, enumerable.
    ///
    /// This exists so the host can check **coverage**: `realize` has a test that walks this list and
    /// asserts each kind maps to a real widget, which is what stops a newly-added kind from silently
    /// rendering an empty container. Keep it in sync with the enum — [`ordinal`](Self::ordinal)
    /// makes that mechanical rather than a matter of discipline (see its docs).
    pub const ALL: &'static [WidgetKind] = &[
        WidgetKind::VStack,
        WidgetKind::HStack,
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
        WidgetKind::Separator,
    ];

    /// This kind's position in [`ALL`](Self::ALL).
    ///
    /// The match is **exhaustive**, so adding a variant to the enum without adding it here is a
    /// *compile error*; the `all_lists_every_widget_kind` test then compares the two, so adding it
    /// here without adding it to [`ALL`](Self::ALL) is a *test failure*. Between them, the list
    /// cannot silently fall behind the vocabulary — which is the whole point, since the coverage
    /// guard is only as good as the list it walks.
    ///
    /// Only the coverage test reads it, but it is compiled in **every** build on purpose: an
    /// uncompiled match cannot be the compile error described above. (The app crate hid this
    /// behind a module-wide `allow(dead_code)`; here the allow is narrowed to the one item.)
    #[allow(dead_code)]
    fn ordinal(self) -> usize {
        match self {
            WidgetKind::VStack => 0,
            WidgetKind::HStack => 1,
            WidgetKind::Row => 2,
            WidgetKind::Grid => 3,
            WidgetKind::Card => 4,
            WidgetKind::Scroll => 5,
            WidgetKind::Panel => 6,
            WidgetKind::Surface => 7,
            WidgetKind::ItemGroup => 8,
            WidgetKind::DockFrame => 9,
            WidgetKind::MarkerGroup => 10,
            WidgetKind::Tabs => 11,
            WidgetKind::Choice => 12,
            WidgetKind::Label => 13,
            WidgetKind::Button => 14,
            WidgetKind::IconButton => 15,
            WidgetKind::Badge => 16,
            WidgetKind::BadgeButton => 17,
            WidgetKind::Tag => 18,
            WidgetKind::Icon => 19,
            WidgetKind::Input => 20,
            WidgetKind::Select => 21,
            WidgetKind::Toggle => 22,
            WidgetKind::Checkbox => 23,
            WidgetKind::StatusDot => 24,
            WidgetKind::Gauge => 25,
            WidgetKind::ScrollBar => 26,
            WidgetKind::Alert => 27,
            WidgetKind::Toast => 28,
            WidgetKind::RailCell => 29,
            WidgetKind::Item => 30,
            WidgetKind::Separator => 31,
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

// ── The remaining fixed value sets ────────────────────────────────────────────────────────
//
// A property that accepts only a fixed set of words gets a type here, so a misspelling cannot
// compile. Written as free text, `orientation: "vertcal"` is accepted, ignored, and never
// reported — the same silent failure F003/P010/T006 removed from actions, which was still live in
// this vocabulary until F003/P011/T019.
//
// **None of these gets a `PropValue` variant, and neither should the next one.** A fixed set
// travels as its NAME, so `PropValue::Text` already carries every one of them; the type belongs in
// the authoring layer, not in the wire format. `Size`/`Variant`/`Align` above predate that rule and
// are the closed shape `PropValue::Map` was added to escape — do not extend it.

/// Which way a rule runs — mirrors grid-ui `Orientation` (`Separator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewOrientation {
    Horizontal,
    Vertical,
}

/// Which way a scroll region scrolls — mirrors grid-ui `ScrollAxes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewScrollAxes {
    Vertical,
    Horizontal,
    Both,
}

/// How serious a message is — mirrors both grid-ui `ToastSeverity` **and** `AlertVariant`, which
/// carry the same four values. One mirror, because two would be the same list written twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

/// Which side a control's label sits on — mirrors grid-ui `LabelSide` (`Checkbox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewLabelSide {
    Right,
    Left,
}

/// How a selected row shows it — mirrors grid-ui `ActiveMarker` (`Row`, `Item`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMarker {
    None,
    Bar,
    Check,
}

/// How children are distributed along the main axis — mirrors grid-ui `Justify`.
///
/// Missed by F003/P011/T019, which took its list from the widgets' own properties: this one is a
/// `Layout` field, so it never appeared there. Found while building the authoring layer on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewJustify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// How text sits in its box — mirrors grid-ui `TextAlign` (`Label`).
///
/// Distinct from [`ViewAlign`], which is where a *widget* sits in its parent. The two read alike
/// and mean different things, which is why both names say what they align.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextAlign {
    Start,
    Center,
    End,
}

/// The snake_case name a fixed value set travels under, and the conversion into a property value.
///
/// One macro rather than six copies: the name is what crosses to the widget, and every mirror
/// converts the same way, so `.prop("orientation", ViewOrientation::Vertical)` works for all of
/// them without `PropValue` growing a variant per set.
macro_rules! value_set {
    ($($ty:ident { $($variant:ident => $name:literal),* $(,)? })*) => {
        $(
            impl $ty {
                /// Every value in this set, in declaration order.
                ///
                /// Emitted from the same list as [`name`](Self::name), so it cannot fall behind:
                /// adding a variant to the enum without adding it here fails to compile at the
                /// `name` match, which is exhaustive.
                pub const ALL: &'static [$ty] = &[$($ty::$variant),*];

                /// The name this value travels under — what the widget's own enum parses.
                pub fn name(self) -> &'static str {
                    match self {
                        $($ty::$variant => $name,)*
                    }
                }
            }

            impl From<$ty> for PropValue {
                fn from(v: $ty) -> Self {
                    PropValue::Text(v.name().to_string())
                }
            }
        )*
    };
}

value_set! {
    ViewOrientation { Horizontal => "horizontal", Vertical => "vertical" }
    ViewScrollAxes { Vertical => "vertical", Horizontal => "horizontal", Both => "both" }
    ViewSeverity { Info => "info", Success => "success", Warning => "warning", Danger => "danger" }
    ViewLabelSide { Right => "right", Left => "left" }
    ViewMarker { None => "none", Bar => "bar", Check => "check" }
    ViewTextAlign { Start => "start", Center => "center", End => "end" }
    ViewJustify {
        Start => "start",
        Center => "center",
        End => "end",
        SpaceBetween => "space_between",
        SpaceAround => "space_around",
        SpaceEvenly => "space_evenly",
    }
}

// ── Scalars into property values ──────────────────────────────────────────────────────────
//
// So the authoring layer can write `.gap(8)` and `.bordered(true)` without naming a variant at
// every call. A string becomes `Text`; a colour is NOT inferred from one, because a colour has to
// say it is one — `.fill("accent")` goes through a setter that wraps it, so a theme token is never
// mistaken for a caption.

// The three older sets keep their own variants — that is how they already travel, and changing it
// would be a wire-format break. New sets do not get one; see the note above `ViewOrientation`.

impl From<ViewAlign> for PropValue {
    fn from(v: ViewAlign) -> Self {
        PropValue::Align(v)
    }
}

impl From<ViewVariant> for PropValue {
    fn from(v: ViewVariant) -> Self {
        PropValue::Variant(v)
    }
}

impl From<ViewSize> for PropValue {
    fn from(v: ViewSize) -> Self {
        PropValue::Size(v)
    }
}

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

// ── GENERATED — do not edit by hand ────────────────────────────────────────────────────────
//
// The icon names, mirrored from `heca_grid_ui::Glyph`. This crate cannot depend on the widget
// library (that is the point of it), so the list is copied — and a copy of 52 names that grows is
// exactly the kind of hand-kept list this codebase keeps being bitten by. So it is **generated and
// guarded**: `every_glyph_name_has_a_mirror` in `heca-view-realize` compares this against
// `Glyph::VARIANT_NAMES` and fails, naming what is missing, the moment an icon is added there
// (F003/P011/T019).
//
// To regenerate: add the variant here with its snake_case name in the `value_set!` block below.

/// A Phosphor icon, by name — mirrors `heca_grid_ui::Glyph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewGlyph {
    Folder,
    FolderOpen,
    File,
    FileCode,
    GitBranch,
    GitCommit,
    GitMerge,
    GitPullRequest,
    Terminal,
    Gear,
    Search,
    Close,
    Check,
    CaretRight,
    CaretLeft,
    CaretDown,
    CaretUp,
    Play,
    Pause,
    Stop,
    Warning,
    WarningCircle,
    Info,
    Circle,
    Lightning,
    List,
    Sidebar,
    DotsThreeVertical,
    ArrowRight,
    ArrowLineLeft,
    ArrowLineRight,
    Plus,
    Minus,
    SquareSplitVertical,
    XSquare,
    FrameCorners,
    Cards,
    Pencil,
    NotePencil,
    Backspace,
    Trash,
    XCircle,
    PlusCircle,
    FolderSimpleMinus,
    FolderSimplePlus,
    PlusSquare,
    StackPlus,
    StackMinus,
    ColumnsPlusLeft,
    ColumnsPlusRight,
    SquareHalf,
    SquareSplitHorizontal,
    SquareHalfBottom,
}

/// Names and the conversion into a property value, from the same list as the enum.
///
/// A glyph becomes [`PropValue::Glyph`], not `Text`: that variant already exists and says what the
/// string is, so `realize` resolves it against the icon font rather than guessing.
macro_rules! glyph_set {
    ($($variant:ident => $name:literal),* $(,)?) => {
        impl ViewGlyph {
            /// Every glyph, in enum order.
            pub const ALL: &'static [ViewGlyph] = &[$(ViewGlyph::$variant),*];

            /// The name this glyph travels under.
            pub fn name(self) -> &'static str {
                match self {
                    $(ViewGlyph::$variant => $name,)*
                }
            }
        }

        impl From<ViewGlyph> for PropValue {
            fn from(g: ViewGlyph) -> Self {
                PropValue::Glyph(g.name().to_string())
            }
        }
    };
}

glyph_set! {
    Folder => "folder",
    FolderOpen => "folder_open",
    File => "file",
    FileCode => "file_code",
    GitBranch => "git_branch",
    GitCommit => "git_commit",
    GitMerge => "git_merge",
    GitPullRequest => "git_pull_request",
    Terminal => "terminal",
    Gear => "gear",
    Search => "search",
    Close => "close",
    Check => "check",
    CaretRight => "caret_right",
    CaretLeft => "caret_left",
    CaretDown => "caret_down",
    CaretUp => "caret_up",
    Play => "play",
    Pause => "pause",
    Stop => "stop",
    Warning => "warning",
    WarningCircle => "warning_circle",
    Info => "info",
    Circle => "circle",
    Lightning => "lightning",
    List => "list",
    Sidebar => "sidebar",
    DotsThreeVertical => "dots_three_vertical",
    ArrowRight => "arrow_right",
    ArrowLineLeft => "arrow_line_left",
    ArrowLineRight => "arrow_line_right",
    Plus => "plus",
    Minus => "minus",
    SquareSplitVertical => "square_split_vertical",
    XSquare => "x_square",
    FrameCorners => "frame_corners",
    Cards => "cards",
    Pencil => "pencil",
    NotePencil => "note_pencil",
    Backspace => "backspace",
    Trash => "trash",
    XCircle => "x_circle",
    PlusCircle => "plus_circle",
    FolderSimpleMinus => "folder_simple_minus",
    FolderSimplePlus => "folder_simple_plus",
    PlusSquare => "plus_square",
    StackPlus => "stack_plus",
    StackMinus => "stack_minus",
    ColumnsPlusLeft => "columns_plus_left",
    ColumnsPlusRight => "columns_plus_right",
    SquareHalf => "square_half",
    SquareSplitHorizontal => "square_split_horizontal",
    SquareHalfBottom => "square_half_bottom",
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
/// `width`, `height`, min/max sizes, `flex_grow`, `flex_shrink`, `hidden`, `grid_cell`, `size`.
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
        ViewNode::new(WidgetKind::VStack)
            .prop("gap", PropValue::Int(8))
            .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
            .child(
                ViewNode::new(WidgetKind::HStack)
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
        assert_eq!(back.kind, WidgetKind::VStack);
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
    /// A value set's `name()` and its serde spelling are the same word.
    ///
    /// Two things have to agree for a fixed value to survive the trip: `name()`, which the
    /// authoring layer writes into a property, and the serde `rename_all` spelling, which is what a
    /// serialized description carries. If they ever differed, a value written through the type
    /// would arrive as a word the widget does not know — silently, which is the failure these types
    /// exist to remove. `ALL` comes from the same list as `name`, so this cannot miss a variant.
    #[test]
    fn a_value_sets_name_matches_how_it_serializes() {
        fn check<T: Copy + Serialize + std::fmt::Debug>(all: &[T], name: impl Fn(T) -> &'static str) {
            for &v in all {
                let json = serde_json::to_value(v).unwrap();
                assert_eq!(json.as_str(), Some(name(v)), "{v:?}: name() and serde disagree");
            }
        }
        check(ViewOrientation::ALL, ViewOrientation::name);
        check(ViewScrollAxes::ALL, ViewScrollAxes::name);
        check(ViewSeverity::ALL, ViewSeverity::name);
        check(ViewLabelSide::ALL, ViewLabelSide::name);
        check(ViewMarker::ALL, ViewMarker::name);
        check(ViewTextAlign::ALL, ViewTextAlign::name);
        check(ViewGlyph::ALL, ViewGlyph::name);
    }

    /// A fixed value converts into a property as its name, and **not** as a new `PropValue`
    /// variant.
    ///
    /// The wire format stays open on purpose: a fixed set travels as text, so the next one added
    /// needs no change to `PropValue` at all. `Size`/`Variant`/`Align` predate that rule. A glyph
    /// is the exception that proves it — it uses the `Glyph` variant, which already existed and
    /// says what the string is.
    #[test]
    fn a_value_set_travels_as_text() {
        assert_eq!(
            PropValue::from(ViewOrientation::Vertical),
            PropValue::Text("vertical".into()),
        );
        assert_eq!(PropValue::from(ViewSeverity::Danger), PropValue::Text("danger".into()));
        assert_eq!(
            PropValue::from(ViewGlyph::GitBranch),
            PropValue::Glyph("git_branch".into()),
        );
    }
}
