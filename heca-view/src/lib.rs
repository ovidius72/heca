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
//! A node that carries an `on_press`/`on_change` intent is *actionable*; `realize` makes every
//! actionable node pickable by `prefix+/` for free, and `on_hint` says what a pick does when that
//! differs from a click.
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
    /// **A grid of cards with a cursor** — the shape a picker surface is: an exposé, a palette of
    /// tiles, a plugin's chooser (F003/P097/T501).
    ///
    /// Each child is one card, in reading order, and its own `key` is what activation hands back.
    /// The cursor is the widget's — arrow keys move it, hovering moves it, Enter activates, Escape
    /// dismisses — and a described grid gets all of that with nothing declared but the cards.
    ///
    /// **The lit card is not something a description wires.** Natively a caller hands the grid each
    /// card's own state signal; a description cannot name another node's signal, so the realizer
    /// makes that connection itself — it is the one building both the card and the cell. That is
    /// why this can be described at all while a `ScrollBar` cannot.
    CardGrid,
    Card,
    Scroll,
    Panel,
    Surface,
    /// Grouped list of items (`ItemGroup`).
    ItemGroup,
    /// A titled, collapsible dock frame.
    DockFrame,
    /// **A surface over the page**: a scrim, a panel holding the children, and — the reason a
    /// description can raise one at all — how it **arrives and leaves** (`animation_named`).
    ///
    /// The panel is the children: one child is the panel, several are stacked into one. Native
    /// code hands the widget a live animation, including a type this model has never heard of; a
    /// description names a built-in and gets the same gesture (`heca_grid_ui::NamedAnimation`).
    Overlay,
    /// A row of column/pane markers.
    MarkerGroup,
    /// A tab strip + panel.
    Tabs,
    /// One selectable **option**: a `value` plus arbitrary composed content. The children of a
    /// [`Select`](WidgetKind::Select) / [`Tabs`](WidgetKind::Tabs) — and usable on its own.
    Choice,
    /// **A picker over its own children**: open it and every pickable node beneath wears a letter,
    /// typing one runs that node's `hint`. A transparent wrapper the rest of the time.
    ///
    /// `opens_on` names the verb that opens it (`"mypanel.pick"`), and config binds the key to that
    /// name — so a described surface owns a picker on the same terms the exposé does, instead of
    /// only contributing targets to heca's (`heca_grid_ui::widgets::KeyHintGroup`).
    KeyHintGroup,

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
    /// **Something is happening and nobody knows for how long** — an indeterminate ring
    /// (F003/P097/T501).
    ///
    /// It takes no properties of its own: it animates itself off the frame clock, and its diameter
    /// is `width`/`height` like any other node's. Reach for [`Progress`](WidgetKind::Progress)
    /// instead the moment you can say *how far along* — a spinner is what you show when you cannot.
    Spinner,
    /// **How far along something is**, `0.0..=1.0` in the `value` prop (F003/P097/T501).
    ///
    /// The fill eases toward whatever it is given, so a described tree re-sent with a new `value`
    /// animates rather than jumping, with nothing declared.
    Progress,
    /// **A keyboard glyph** from the embedded Nerd Font — ⇧ ⌃ ⌥ ⌘, Enter, Escape, the arrows
    /// (F003/P097/T501).
    ///
    /// Its own `glyph` vocabulary ([`ViewNfGlyph`]), because it is its own font. It is what lets a
    /// plugin draw a shortcut the way heca's own key hints do, rather than typing a character its
    /// user's font may not carry.
    NfIcon,
    /// **A row of actions that gets out of its own way** (F003/P097/T501).
    ///
    /// Its children are [`Button`](WidgetKind::Button) nodes. As the room runs out it shows icons
    /// instead of words, and whatever still does not fit collapses into a ⋮ menu that runs the same
    /// actions — none of which an author writes. A button's own text becomes its menu row and its
    /// words on hover, so it is written once.
    ///
    /// The group is why a tooltip and a hint placement had to stop being wrappers: it holds
    /// **typed** buttons, and wrapping one changes what it is.
    ButtonGroup,
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
        WidgetKind::Overlay,
        WidgetKind::MarkerGroup,
        WidgetKind::Tabs,
        WidgetKind::Choice,
        WidgetKind::KeyHintGroup,
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
        WidgetKind::CardGrid,
        WidgetKind::Spinner,
        WidgetKind::Progress,
        WidgetKind::NfIcon,
        WidgetKind::ButtonGroup,
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
            WidgetKind::Overlay => 10,
            WidgetKind::MarkerGroup => 11,
            WidgetKind::Tabs => 12,
            WidgetKind::Choice => 13,
            WidgetKind::KeyHintGroup => 14,
            WidgetKind::Label => 15,
            WidgetKind::Button => 16,
            WidgetKind::IconButton => 17,
            WidgetKind::Badge => 18,
            WidgetKind::BadgeButton => 19,
            WidgetKind::Tag => 20,
            WidgetKind::Icon => 21,
            WidgetKind::Input => 22,
            WidgetKind::Select => 23,
            WidgetKind::Toggle => 24,
            WidgetKind::Checkbox => 25,
            WidgetKind::StatusDot => 26,
            WidgetKind::Gauge => 27,
            WidgetKind::ScrollBar => 28,
            WidgetKind::Alert => 29,
            WidgetKind::Toast => 30,
            WidgetKind::RailCell => 31,
            WidgetKind::Item => 32,
            WidgetKind::Separator => 33,
            WidgetKind::CardGrid => 34,
            WidgetKind::Spinner => 35,
            WidgetKind::Progress => 36,
            WidgetKind::NfIcon => 37,
            WidgetKind::ButtonGroup => 38,
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

/// How a surface arrives and leaves — mirrors grid-ui `NamedAnimation` (`Overlay`).
///
/// The **built-ins**, which is all a description can name: a live animation is a Rust type, and a
/// plugin that writes its own reaches it from native code rather than from data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewAnimation {
    /// A cut: there, then gone.
    None,
    /// A dissolve, both ways.
    Fade,
    /// Growing in from smaller, shrinking away again.
    Zoom,
    /// The exposé's gesture: it shrinks away, and the dissolve rides the shrink.
    ZoomFade,
}

/// Where a scroll region puts the descendant it follows — mirrors grid-ui `RevealAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRevealAlign {
    /// Scroll the least that makes it visible.
    Minimal,
    /// Keep it at the centre of the viewport.
    Center,
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

/// A spacing step from the theme — mirrors grid-ui `Spacing`.
///
/// **Resolved from the inherited font at layout, never a pixel count**, so a described tree spaces
/// itself the way the rest of the app does and follows a font or theme change with nothing
/// rewritten. It is what an author reaches for instead of `padding(16.0)`: raw pixels are still
/// there for the rare case that genuinely needs one, and are the wrong default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSpacing {
    None,
    Xs,
    Sm,
    Md,
    Lg,
}

/// **A space: a number of pixels, or a step of the theme's rhythm** — mirrors grid-ui `Space`.
///
/// One authoring type, because spacing is one property. It was two builders on each axis — a px
/// `gap` beside a `gap_spacing` step, `padding` beside `pad_all` — and the docs told everyone to
/// prefer the step while the px name stayed the shorter, more obvious one.
///
/// ```ignore
/// VStack::new().gap(8)                 // eight pixels
/// VStack::new().gap(ViewSpacing::Sm)   // a step of the rhythm
/// VStack::new().gap("sm")              // the same step, said as JSON would
/// ```
///
/// **Prefer the step.** It is resolved from the inherited font at layout, so it follows a font,
/// size-variant or zoom change with nothing rewritten; a pixel count is tuned for one font size
/// and wrong at every other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewSpace {
    /// Logical pixels, fixed whatever the font does.
    Px(f32),
    /// A step of the theme's rhythm.
    Step(ViewSpacing),
}

impl From<ViewSpacing> for ViewSpace {
    fn from(s: ViewSpacing) -> Self {
        ViewSpace::Step(s)
    }
}

impl From<f32> for ViewSpace {
    fn from(v: f32) -> Self {
        ViewSpace::Px(v)
    }
}

impl From<f64> for ViewSpace {
    /// Rust reads a bare decimal as `f64`, so without this `.gap(8.0)` does not compile.
    fn from(v: f64) -> Self {
        ViewSpace::Px(v as f32)
    }
}

impl From<i32> for ViewSpace {
    /// …and a bare integer as `i32`.
    fn from(v: i32) -> Self {
        ViewSpace::Px(v as f32)
    }
}

impl From<&str> for ViewSpace {
    /// `"sm"` for a step, `"8"` / `"8px"` for pixels — the spellings the described side reads.
    ///
    /// An unrecognised step name is passed through as text rather than guessed at: the realizing
    /// side owns the vocabulary and degrades a value it cannot read, which is the rule every
    /// untrusted value here follows.
    fn from(t: &str) -> Self {
        match t.trim().to_ascii_lowercase().as_str() {
            "none" => ViewSpace::Step(ViewSpacing::None),
            "xs" => ViewSpace::Step(ViewSpacing::Xs),
            "sm" => ViewSpace::Step(ViewSpacing::Sm),
            "md" => ViewSpace::Step(ViewSpacing::Md),
            "lg" => ViewSpace::Step(ViewSpacing::Lg),
            other => ViewSpace::Px(
                other
                    .strip_suffix("px")
                    .map_or(other, str::trim_end)
                    .parse()
                    .unwrap_or(0.0),
            ),
        }
    }
}

impl From<ViewSpace> for PropValue {
    /// A step travels as its **name** and a length as a number — the two spellings the layout
    /// setting reads back, so one property name carries either.
    fn from(s: ViewSpace) -> Self {
        match s {
            ViewSpace::Px(v) => PropValue::Float(v as f64),
            ViewSpace::Step(step) => step.into(),
        }
    }
}

/// How a [`ButtonGroup`](WidgetKind::ButtonGroup) shows its actions — mirrors grid-ui `Display`.
///
/// Whatever does not fit collapses into a ⋮ menu whichever of these is chosen; this only decides
/// how wide each action is before that happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewDisplay {
    /// Words while there is room, icons once there is not.
    ///
    /// ⚠️ **Not settled** — at some widths the group alternates between the two on successive
    /// layouts, because taking the words off is what makes the row fit. Prefer the other two.
    Auto,
    /// Always icons, however much room there is. The labels still say what the hover bubble and the
    /// collapsed menu read. **The default**, because it is the one that is settled.
    IconOnly,
    /// Always words. The group collapses into the menu sooner, because each action is wider.
    Full,
}

/// Which side of a widget its tooltip anchors to — mirrors grid-ui `TooltipSide`.
///
/// A **preference, not a placement**: the framework flips it to the opposite side when there is no
/// room, so an author says where they would like the bubble and never where it must go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTooltipSide {
    Top,
    Bottom,
    Left,
    Right,
}

/// Where a node's hint letter sits over it — mirrors grid-ui `HintPlacement`.
///
/// The picker draws the cap itself; this only says where. `TopLeft` is what the picker drew for
/// every large target before letters became the widget's own to place, and it is kept as a variant
/// because that rule was right for a card or a content pane: out of the way of what the target
/// shows, and never on its border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewHintPlacement {
    TopCenter,
    Center,
    CenterRight,
    TopRight,
    TopLeft,
}

/// **What a keycap means** — mirrors grid-ui `HintTone`. The theme picks the colour, so a letter
/// follows a theme reload and a plugin never writes a hex.
///
/// `Accent` is a place to go; `Muted` a structural control — fold this, close that — which is not
/// somewhere to navigate; the rest are further classes so two kinds of target never read alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewHintTone {
    Accent,
    Muted,
    Warning,
    Success,
    Danger,
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

/// Which end of a label is cut when its text does not fit — mirrors grid-ui `Ellipsis` (`Label`).
///
/// Two, because the two kinds of text read from opposite ends: a label is identified by its
/// beginning, a path by its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewEllipsis {
    /// Keep the head, cut the tail.
    End,
    /// Keep the tail, cut the head — what a path needs.
    Start,
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
    ViewRevealAlign { Minimal => "minimal", Center => "center" }
    ViewAnimation { None => "none", Fade => "fade", Zoom => "zoom", ZoomFade => "zoom_fade" }
    ViewSeverity { Info => "info", Success => "success", Warning => "warning", Danger => "danger" }
    ViewLabelSide { Right => "right", Left => "left" }
    ViewTooltipSide { Top => "top", Bottom => "bottom", Left => "left", Right => "right" }
    ViewDisplay { Auto => "auto", IconOnly => "icon_only", Full => "full" }
    ViewSpacing { None => "none", Xs => "xs", Sm => "sm", Md => "md", Lg => "lg" }
    ViewHintTone { Accent => "accent", Muted => "muted", Warning => "warning", Success => "success", Danger => "danger" }
    ViewNfGlyph {
        Shift => "shift",
        Control => "control",
        Option => "option",
        Command => "command",
        CapsLock => "caps_lock",
        Enter => "enter",
        Escape => "escape",
        Tab => "tab",
        Space => "space",
        Backspace => "backspace",
        ArrowUp => "arrow_up",
        ArrowDown => "arrow_down",
        ArrowLeft => "arrow_left",
        ArrowRight => "arrow_right",
    }
    ViewHintPlacement {
        TopCenter => "top_center",
        Center => "center",
        CenterRight => "center_right",
        TopRight => "top_right",
        TopLeft => "top_left",
    }
    ViewMarker { None => "none", Bar => "bar", Check => "check" }
    ViewTextAlign { Start => "start", Center => "center", End => "end" }
    ViewEllipsis { End => "end", Start => "start" }
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
    StackPlus,
    StackMinus,
    ColumnsPlusLeft,
    ColumnsPlusRight,
    SquareHalf,
    SquareSplitHorizontal,
    SquareHalfBottom,
}

/// The **keyboard** glyphs, from the embedded Nerd Font — a separate vocabulary from
/// [`ViewGlyph`] because it is a separate font (F003/P097/T501).
///
/// These are the keys a shortcut is written with: ⇧ ⌃ ⌥ ⌘, Enter, Escape, Tab, Space, Backspace and
/// the four arrows. A description names one and the host resolves it against the font, exactly as
/// it does an icon name — so a plugin can render a keybinding the way heca's own key hints do
/// instead of typing a character that its user's font may not have.
///
/// Held honest by `every_glyph_name_has_a_mirror`, which compares both vocabularies against the
/// library's own in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewNfGlyph {
    Shift,
    Control,
    Option,
    Command,
    CapsLock,
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
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

    /// Bind an event to an intent (e.g. `.on("press", Intent::new("close"))`).
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
        self.on("press", intent)
    }

    /// Convenience: bind the `"hint"` event — **what a leader-key pick (`prefix+/`) does to this
    /// node**, when that is not simply what a click does.
    ///
    /// Unbound, a pick falls back to [`press`](Self::on_press), so every actionable node is
    /// reachable by letter for free. Bind it when the two genuinely differ: heca's sidebar row
    /// activates the pane and leaves the sidebar on a click, and stays in the sidebar on a hint pick.
    pub fn on_hint(self, intent: Intent) -> Self {
        self.on("hint", intent)
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
        self.events.contains_key("press")
    }

    /// The intent bound to `event`, if any.
    pub fn intent(&self, event: &str) -> Option<&Intent> {
        self.events.get(event)
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
            .intent("press")
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

/// One entry of a dropdown / context menu. The author supplies id/label/action; the host resolves
/// the icon from the action registry (`ActionCatalog::icon`) and wires the intent + quick-pick —
/// the same centralized path as [`ModalAction`], with **no hand-picked glyph and no `prefix+X`
/// label** (the leader doesn't work while the menu is open; a host-assigned single-letter quick-pick
/// that *does* work replaces it).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DropdownItem {
    /// Stable id returned in [`ModalResult::Action`]; also the **catalog name** the icon and label
    /// resolve from — the entry's visual identity (e.g. `"close"`).
    ///
    /// It is deliberately **not** the same thing as what the entry runs: a sidebar "Close pane"
    /// entry has id `close` (so it shows the close icon) but dispatches `close_pane_by_id` with the
    /// row's pane. Identity and behaviour are separate fields.
    pub id: String,
    pub label: String,
    /// What the entry dispatches when chosen: an [`Intent`] — an action **name + args** — routed
    /// through the one dispatch door, so the interaction policy and the confirm gate apply exactly
    /// as they would for a keypress.
    ///
    /// An `Intent` rather than a `WmAction` because `WmAction` is a **closed enum**: a plugin cannot
    /// add a variant, so a menu entry carrying one could only ever run actions heca already has —
    /// which is precisely what blocked plugin-contributed menus (context-menu-5). A name resolves to
    /// a built-in *or* to a plugin's own registered action, indifferently.
    pub intent: Intent,
    pub danger: bool,
    pub enabled: bool,
    /// **Where this entry sits among all the others**, or `None` to sit where its block sits.
    ///
    /// A menu is filled by several sources at once — heca's own entries and any plugin's — and each
    /// source declares a weight for its whole block. That is the right granularity most of the
    /// time: a plugin thinks in "my entries". It is not enough when one entry belongs at the very
    /// top and the rest belong at the bottom, because a block can only move whole.
    ///
    /// So an entry may say where it goes, and **one that says nothing takes its block's weight**.
    /// There is a single ordering rule rather than "sort the blocks, then sort inside them": every
    /// entry has a weight, most simply do not spell it, and the whole menu is one sorted list.
    ///
    /// A list of numbers rather than one, sorted ascending, so an entry can be slotted *between*
    /// two neighbours without renumbering either — `[1, 1, 1]` lands between `[1, 1]` and `[1, 2]`.
    /// Ties keep the order the entries were produced in.
    pub weight: Option<Vec<i64>>,
}

impl DropdownItem {
    /// An enabled, non-destructive entry whose id is also the action it runs (the common case: the
    /// entry's catalog identity and its behaviour coincide, e.g. `zoom_column`).
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id = id.into();
        let intent = Intent::new(id.clone());
        Self {
            id,
            label: label.into(),
            intent,
            danger: false,
            enabled: true,
            weight: None,
        }
    }

    /// An entry whose behaviour differs from its visual identity — the id keeps the icon/label
    /// (`close`), while the intent carries the action actually run, with its args
    /// (`close_pane_by_id` + `pane_id`).
    pub fn with_intent(id: impl Into<String>, label: impl Into<String>, intent: Intent) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            intent,
            danger: false,
            enabled: true,
            weight: None,
        }
    }
    /// Tint destructive (red) — the confirm gate still applies on dispatch.
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Enable/disable (a disabled entry is dimmed + unselectable).
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// **Put this entry somewhere other than where its block sits** — see
    /// [`weight`](DropdownItem::weight).
    ///
    /// ```ignore
    /// DropdownItem::new("myplugin.pin", "Pin this").weight(vec![0])   // above everything
    /// ```
    pub fn weight(mut self, weight: Vec<i64>) -> Self {
        self.weight = Some(weight);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── The identity rule, declarative half: a `press` in a collection needs a `key` ──────────

    fn row(action: &str) -> ViewNode {
        ViewNode::new(WidgetKind::Row).on_press(Intent::new(action))
    }

    /// The case this exists for: rows built by iterating, none of them keyed. Every one is
    /// reported, because every one loses its cursor position and its letter on the next rebuild.
    #[test]
    fn every_unkeyed_pressable_row_of_a_collection_is_reported() {
        let tree = ViewNode::new(WidgetKind::VStack)
            .child(row("focus_pane"))
            .child(row("focus_pane"))
            .child(row("focus_pane"));

        let found = unkeyed_collection_items(&tree);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].path, vec![0]);
        assert_eq!(found[2].path, vec![2]);
        assert_eq!(found[0].kind, WidgetKind::Row);
        assert_eq!(found[0].action, "focus_pane");
        assert_eq!(
            found[0].siblings, 3,
            "what the author has to look at to see the collection"
        );
    }

    /// Keying the items is the fix, and it is the only thing the report ever asks for.
    #[test]
    fn keyed_items_are_never_reported() {
        let tree = ViewNode::new(WidgetKind::VStack)
            .child(row("focus_pane").key("pane:7"))
            .child(row("focus_pane").key("pane:9"));

        assert_eq!(unkeyed_collection_items(&tree), vec![]);
    }

    /// A composed control is not a collection: `WidgetKind` says so by construction, with nothing
    /// to infer about what the author meant.
    #[test]
    fn a_composed_control_is_not_a_collection() {
        let tree = ViewNode::new(WidgetKind::Choice)
            .on_press(Intent::new("set_level"))
            .child(ViewNode::new(WidgetKind::Icon))
            .child(ViewNode::new(WidgetKind::Label).text("HIGH"));

        assert_eq!(unkeyed_collection_items(&tree), vec![]);
    }

    /// **Only an actionable node is reported.** Identity is what a cursor, a right-click, a drag and
    /// a remembered letter are kept on — decorative siblings have none of those to lose.
    #[test]
    fn a_collection_with_nothing_to_press_is_not_reported() {
        let tree = ViewNode::new(WidgetKind::VStack)
            .child(ViewNode::new(WidgetKind::Label).text("one"))
            .child(ViewNode::new(WidgetKind::Label).text("two"))
            .child(ViewNode::new(WidgetKind::Label).text("three"));

        assert_eq!(unkeyed_collection_items(&tree), vec![]);
    }

    /// A single pressable child is not a collection — the ordinary case of a button in a box, which
    /// needs no key and must never be asked for one.
    #[test]
    fn a_lone_pressable_child_is_not_a_collection() {
        let tree = ViewNode::new(WidgetKind::HStack)
            .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("OK")
                    .on_press(Intent::new("confirm_ok")),
            );

        assert_eq!(unkeyed_collection_items(&tree), vec![]);
    }

    /// Two buttons side by side **are** a collection of two, so an unkeyed pressable one is
    /// reported — a confirm dialog's Cancel/OK pair is the everyday example, and the everyday fix
    /// is a key naming the action.
    #[test]
    fn a_pair_of_buttons_is_a_collection_of_two() {
        let tree = ViewNode::new(WidgetKind::HStack)
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("Cancel")
                    .on_press(Intent::new("cancel")),
            )
            .child(
                ViewNode::new(WidgetKind::Button)
                    .text("OK")
                    .on_press(Intent::new("confirm_ok")),
            );

        let found = unkeyed_collection_items(&tree);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].action, "cancel");
        assert_eq!(found[1].action, "confirm_ok");
    }

    /// The report reaches the whole tree, not only its top: a list nested inside a card is still a
    /// list, and its path says where to look.
    #[test]
    fn a_nested_collection_is_reported_with_its_path() {
        let tree = ViewNode::new(WidgetKind::VStack).child(
            ViewNode::new(WidgetKind::Card)
                .child(row("open"))
                .child(row("open")),
        );

        let found = unkeyed_collection_items(&tree);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].path, vec![0, 0]);
        assert_eq!(found[1].path, vec![0, 1]);
    }

    /// Siblings of **different** kinds are not one collection, however many of them there are.
    #[test]
    fn different_kinds_are_not_one_collection() {
        let tree = ViewNode::new(WidgetKind::HStack)
            .child(ViewNode::new(WidgetKind::Button).on_press(Intent::new("a")))
            .child(ViewNode::new(WidgetKind::Row).on_press(Intent::new("b")))
            .child(ViewNode::new(WidgetKind::Item).on_press(Intent::new("c")));

        assert_eq!(unkeyed_collection_items(&tree), vec![]);
    }

    /// `realize` writes a described key into the very slot a native `.key(..)` writes — held on the
    /// realize side; here we only hold that the node carries it and reads it back.
    #[test]
    fn a_key_is_an_ordinary_prop_read_back_by_name() {
        let node = ViewNode::new(WidgetKind::Row).key("pane:7");
        assert_eq!(node.declared_key(), Some("pane:7"));
        assert_eq!(
            node.props.get("key"),
            Some(&PropValue::Text("pane:7".into()))
        );
        assert_eq!(ViewNode::new(WidgetKind::Row).declared_key(), None);
    }

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
        assert_eq!(
            tree, back,
            "ViewNode must round-trip through JSON (WASM boundary)"
        );
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
        fn check<T: Copy + Serialize + std::fmt::Debug>(
            all: &[T],
            name: impl Fn(T) -> &'static str,
        ) {
            for &v in all {
                let json = serde_json::to_value(v).unwrap();
                assert_eq!(
                    json.as_str(),
                    Some(name(v)),
                    "{v:?}: name() and serde disagree"
                );
            }
        }
        check(ViewOrientation::ALL, ViewOrientation::name);
        check(ViewScrollAxes::ALL, ViewScrollAxes::name);
        check(ViewRevealAlign::ALL, ViewRevealAlign::name);
        check(ViewAnimation::ALL, ViewAnimation::name);
        check(ViewSeverity::ALL, ViewSeverity::name);
        check(ViewLabelSide::ALL, ViewLabelSide::name);
        check(ViewTooltipSide::ALL, ViewTooltipSide::name);
        check(ViewHintPlacement::ALL, ViewHintPlacement::name);
        check(ViewNfGlyph::ALL, ViewNfGlyph::name);
        check(ViewDisplay::ALL, ViewDisplay::name);
        check(ViewSpacing::ALL, ViewSpacing::name);
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
        assert_eq!(
            PropValue::from(ViewSeverity::Danger),
            PropValue::Text("danger".into())
        );
        assert_eq!(
            PropValue::from(ViewGlyph::GitBranch),
            PropValue::Glyph("git_branch".into()),
        );
    }
}
