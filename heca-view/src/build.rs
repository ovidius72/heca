//! Typed builders for [`ViewNode`] — the authoring layer (plugin-task-ui-2, F003/P011/T006).
//!
//! A [`ViewNode`] is a uniform bag of properties, which is right for a *wire format* and wrong for
//! writing by hand: nothing stops `Label::new("x").prop("title", …)`, and nothing reports it either
//! — the property is simply ignored at realize time.
//!
//! ```
//! use heca_view::build::*;
//! use heca_view::{Intent, PropValue};
//!
//! let tree = VStack::new()
//!     .gap(8.0)
//!     .child(
//!         Panel::new().title("CONTAINERS").child(
//!             Row::new()
//!                 .padding(6.0)
//!                 .on_press(Intent::new("docker.select").arg("id", PropValue::Text("web".into())))
//!                 .child(Label::new("nginx"))
//!                 .child(Badge::new("UP")),
//!         ),
//!     )
//!     .child(Separator::new())
//!     .child(Input::new().placeholder("filter…"))
//!     .into_node();
//! ```
//!
//! **A type per kind**, so a method exists only where the widget really has that property.
//! `Label::new("x").title("y")` does not compile. The shared arrangement and appearance properties
//! live on [`Style`], written once, because they apply to every kind alike (F003/P017/T007).
//! Children live on [`Parent`], implemented only for the kinds `realize` attaches children to — a
//! `Toast` takes none, and now cannot be given any.
//!
//! **This is a convenience, not a gate.** `ViewNode` stays public and `.prop(..)` still works: a
//! plugin shipping a serialized tree never passes through here, so the builders cannot be the only
//! way in, and pretending otherwise would be a lie about where the boundary is.
//!
//! # Staying honest
//!
//! `every_widget_property_is_reachable_from_the_sdk` in `heca-view-realize` walks
//! [`WidgetKind::ALL`] against each widget's generated `PROP_NAMES` and fails when a property has
//! no builder here. It fails **closed**: a new property must reach the SDK, or someone must name an
//! exception and say why. Without it this file becomes the hand-written list that falls behind,
//! which is the defect this whole phase exists to remove.

use crate::{
    Intent, PropMap, PropValue, ViewAlign, ViewEllipsis, ViewGlyph, ViewJustify, ViewLabelSide, ViewMarker,
    ViewNode, ViewOrientation, ViewScrollAxes, ViewSeverity, ViewSize, ViewTextAlign, ViewVariant,
    WidgetKind,
};

/// The properties every kind shares — arrangement (`Layout`) and appearance (`Visual`).
///
/// One trait rather than a copy per builder: since F003/P017/T007 both halves are merged
/// generically by name for every kind, so there is nothing kind-specific to say about them.
///
/// # What is deliberately absent
///
/// - **`direction`** — the kind already says it. A [`HStack`] with `direction: column` is a
///   contradiction the author should not be able to write.
/// - **`size_explicit`** — set by the layout pass to record that a size was chosen, not by an
///   author.
/// - **`grid_cell`** — grid placement is authored as `area` / `col` / `row` on the child, which is
///   what the `Grid` arm reads.
/// - **`pad_spacing_x` / `pad_spacing_y` / `gap_spacing`** — theme `Spacing` tokens, which have no
///   mirror in this crate yet. Use the px setters, or add the mirror when a caller needs it.
pub trait Style: Sized {
    /// The node being built. Public so the trait's defaults can reach it; not the way to author.
    #[doc(hidden)]
    fn node_mut(&mut self) -> &mut ViewNode;

    /// Finish, yielding the node itself — what `realize` and serialisation take.
    fn into_node(self) -> ViewNode;

    /// Set a property by name. The escape hatch for anything this trait does not cover; prefer a
    /// named setter, which is the whole point of the SDK.
    fn prop(mut self, key: &str, value: impl Into<PropValue>) -> Self {
        self.node_mut().props.insert(key.to_string(), value.into());
        self
    }

    // ── Arrangement ──
    /// Space between children, in px.
    fn gap(self, px: f32) -> Self {
        self.prop("gap", px)
    }
    /// Space inside the box on every side, in px.
    fn padding(self, px: f32) -> Self {
        self.prop("padding", px)
    }
    /// Horizontal padding (left + right), in px.
    fn padding_x(self, px: f32) -> Self {
        self.prop("padding_x", px)
    }
    /// Vertical padding (top + bottom), in px.
    fn padding_y(self, px: f32) -> Self {
        self.prop("padding_y", px)
    }
    /// Left padding, in px.
    fn padding_left(self, px: f32) -> Self {
        self.prop("padding_left", px)
    }
    /// Right padding, in px.
    fn padding_right(self, px: f32) -> Self {
        self.prop("padding_right", px)
    }
    /// Top padding, in px.
    fn padding_top(self, px: f32) -> Self {
        self.prop("padding_top", px)
    }
    /// Bottom padding, in px.
    fn padding_bottom(self, px: f32) -> Self {
        self.prop("padding_bottom", px)
    }
    /// Space outside the box on every side, in px.
    fn margin(self, px: f32) -> Self {
        self.prop("margin", px)
    }
    /// Horizontal margin (left + right), in px.
    fn margin_x(self, px: f32) -> Self {
        self.prop("margin_x", px)
    }
    /// Vertical margin (top + bottom), in px — a rule breathing away from what it separates.
    fn margin_y(self, px: f32) -> Self {
        self.prop("margin_y", px)
    }
    /// Left margin, in px.
    fn margin_left(self, px: f32) -> Self {
        self.prop("margin_left", px)
    }
    /// Right margin, in px.
    fn margin_right(self, px: f32) -> Self {
        self.prop("margin_right", px)
    }
    /// Top margin, in px.
    fn margin_top(self, px: f32) -> Self {
        self.prop("margin_top", px)
    }
    /// Bottom margin, in px.
    fn margin_bottom(self, px: f32) -> Self {
        self.prop("margin_bottom", px)
    }

    /// A fixed width in px.
    fn width(self, px: f32) -> Self {
        self.prop("width", px)
    }
    /// A width as a fraction of the parent — `0.5` is half.
    fn width_pct(self, fraction: f32) -> Self {
        self.prop("width", pct(fraction))
    }
    /// Width decided by content and flex rules (the default).
    fn width_auto(self) -> Self {
        self.prop("width", "auto")
    }
    /// A fixed height in px.
    fn height(self, px: f32) -> Self {
        self.prop("height", px)
    }
    /// A height as a fraction of the parent.
    fn height_pct(self, fraction: f32) -> Self {
        self.prop("height", pct(fraction))
    }
    /// Height decided by content and flex rules (the default).
    fn height_auto(self) -> Self {
        self.prop("height", "auto")
    }
    /// Smallest width, in px.
    fn min_width(self, px: f32) -> Self {
        self.prop("min_width", px)
    }
    /// Smallest height, in px.
    fn min_height(self, px: f32) -> Self {
        self.prop("min_height", px)
    }
    /// Largest width, in px.
    fn max_width(self, px: f32) -> Self {
        self.prop("max_width", px)
    }
    /// Largest height, in px.
    fn max_height(self, px: f32) -> Self {
        self.prop("max_height", px)
    }

    /// Where children sit across the main axis.
    fn align(self, a: ViewAlign) -> Self {
        self.prop("align", a)
    }
    /// Where this node sits across its parent's main axis, overriding the parent's `align`.
    fn align_self(self, a: ViewAlign) -> Self {
        self.prop("align_self", a)
    }
    /// How children are distributed along the main axis.
    fn justify(self, j: ViewJustify) -> Self {
        self.prop("justify", j)
    }
    /// Where children sit within their grid cell, across the inline axis.
    fn justify_items(self, a: ViewAlign) -> Self {
        self.prop("justify_items", a)
    }
    /// Where this node sits within its grid cell, across the inline axis.
    fn justify_self(self, a: ViewAlign) -> Self {
        self.prop("justify_self", a)
    }

    /// Share of leftover space this node takes.
    fn flex_grow(self, factor: f32) -> Self {
        self.prop("flex_grow", factor)
    }
    /// Willingness to shrink below the natural size.
    fn flex_shrink(self, factor: f32) -> Self {
        self.prop("flex_shrink", factor)
    }
    /// The size variant, which scales font and padding together and cascades to children.
    fn size(self, s: ViewSize) -> Self {
        self.prop("size", s)
    }
    /// Take the node out of layout and paint entirely.
    fn hidden(self, hidden: bool) -> Self {
        self.prop("hidden", hidden)
    }

    // ── Appearance ──
    /// Background colour: a theme token name (`"accent"`) or a literal (`"#ff8800"`).
    ///
    /// Prefer a token — it follows a theme reload, a literal does not.
    fn fill(self, colour: &str) -> Self {
        self.prop("fill", PropValue::Color(colour.to_string()))
    }
    /// A border of `colour` and `width` px. Colour is a token name or a literal.
    fn border(self, colour: &str, width: f32) -> Self {
        self.prop(
            "border",
            PropValue::Map(PropMap::from([
                ("color".to_string(), PropValue::Color(colour.to_string())),
                ("width".to_string(), PropValue::Float(width as f64)),
            ])),
        )
    }
    /// A glow halo of `colour`, spreading `radius` px at `intensity` (0..=1).
    fn glow(self, colour: &str, radius: f32, intensity: f32) -> Self {
        self.prop(
            "glow",
            PropValue::Map(PropMap::from([
                ("color".to_string(), PropValue::Color(colour.to_string())),
                ("radius".to_string(), PropValue::Float(radius as f64)),
                ("intensity".to_string(), PropValue::Float(intensity as f64)),
            ])),
        )
    }
    /// Corner radius in px.
    fn radius(self, px: f32) -> Self {
        self.prop("radius", px)
    }
    /// Font size in px, overriding the theme's.
    fn font_size(self, px: f32) -> Self {
        self.prop("font_size", px)
    }
    /// Font size as a multiple of the inherited one.
    fn font_scale(self, factor: f32) -> Self {
        self.prop("font_scale", factor)
    }
}

/// A kind that holds children — implemented only for the kinds `realize` attaches them to.
///
/// A `Toast` draws its own card and takes none, so it has no `child`. That is the same rule as the
/// properties: if the widget cannot do it, the builder cannot say it.
pub trait Parent: Style {
    /// Append a child.
    fn child(mut self, child: impl Into<ViewNode>) -> Self {
        self.node_mut().children.push(child.into());
        self
    }

    /// Append several children.
    fn children<C: Into<ViewNode>>(mut self, children: impl IntoIterator<Item = C>) -> Self {
        let node = self.node_mut();
        node.children.extend(children.into_iter().map(Into::into));
        self
    }
}

/// A `Length` percentage, in the spelling its deserializer accepts.
fn pct(fraction: f32) -> PropValue {
    PropValue::Text(format!("{}%", fraction * 100.0))
}

macro_rules! builder {
    ($(#[$m:meta])* $name:ident => $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(ViewNode);

        impl $name {
            /// A new node of this kind.
            pub fn new() -> Self {
                Self(ViewNode::new(WidgetKind::$kind))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Style for $name {
            fn node_mut(&mut self) -> &mut ViewNode {
                &mut self.0
            }
            fn into_node(self) -> ViewNode {
                self.0
            }
        }

        impl From<$name> for ViewNode {
            fn from(b: $name) -> ViewNode {
                b.0
            }
        }
    };
}

/// The same, for kinds whose **whole content is the scalar text** — the text goes in the
/// constructor, because a `Label` with no text is not a thing anyone means to write.
macro_rules! builder_text {
    ($(#[$m:meta])* $name:ident => $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(ViewNode);

        impl $name {
            /// A new node of this kind, showing `text`.
            pub fn new(text: impl Into<String>) -> Self {
                Self(ViewNode::new(WidgetKind::$kind).text(text))
            }
        }

        impl Style for $name {
            fn node_mut(&mut self) -> &mut ViewNode {
                &mut self.0
            }
            fn into_node(self) -> ViewNode {
                self.0
            }
        }

        impl From<$name> for ViewNode {
            fn from(b: $name) -> ViewNode {
                b.0
            }
        }
    };
}

/// Give a builder the scalar `text` sugar, for the kinds whose arm reads it.
macro_rules! with_text {
    ($($name:ident),* $(,)?) => {
        $(
            impl $name {
                /// The scalar text this kind shows.
                pub fn text(mut self, text: impl Into<String>) -> Self {
                    self.0.props.insert("text".into(), PropValue::Text(text.into()));
                    self
                }
            }
        )*
    };
}

/// Give a builder an event binding, for the events `realize` reads on that kind.
macro_rules! with_event {
    ($($name:ident { $($method:ident => $event:literal),* $(,)? })*) => {
        $(
            impl $name {
                $(
                    #[doc = concat!("Bind the `", $event, "` event to an intent.")]
                    pub fn $method(mut self, intent: Intent) -> Self {
                        self.0.events.insert($event.to_string(), intent);
                        self
                    }
                )*
            }
        )*
    };
}

// ── Containers ────────────────────────────────────────────────────────────────────────────
builder!(
    /// A vertical box. Plain arrangement — no focus, no hover, no activation.
    VStack => VStack
);
builder!(
    /// A horizontal box. For the clickable one, see [`Row`].
    HStack => HStack
);
builder!(
    /// A **clickable, selectable** container: hover tint, active state, press flash, focus ring,
    /// activation by mouse and by Enter/Space.
    Row => Row
);
builder!(
    /// A CSS-like grid. Tracks are `columns` / `rows` / `areas`; placement is a property on the
    /// child (`area`, or `col`/`row`).
    Grid => Grid
);
builder_text!(
    /// A titled card.
    Card => Card
);
builder!(
    /// A scroll viewport.
    Scroll => Scroll
);
builder!(
    /// A titled panel: heading, a rule under it, then body.
    Panel => Panel
);
builder!(
    /// A bare styled box.
    Surface => Surface
);
builder!(
    /// A grouped, collapsible list of items.
    ItemGroup => ItemGroup
);
builder!(
    /// A titled, collapsible dock frame.
    DockFrame => DockFrame
);
builder!(
    /// A row of column/pane markers.
    MarkerGroup => MarkerGroup
);
builder!(
    /// A tab strip plus panel. Its tabs are [`Choice`] children.
    Tabs => Tabs
);
builder!(
    /// One selectable option: a `value` plus composed content. The child of a [`Select`] or
    /// [`Tabs`], and usable alone.
    Choice => Choice
);

impl Parent for VStack {}
impl Parent for HStack {}
impl Parent for Row {}
impl Parent for Grid {}
impl Parent for Card {}
impl Parent for Scroll {}
impl Parent for Panel {}
impl Parent for Surface {}
impl Parent for ItemGroup {}
impl Parent for DockFrame {}
impl Parent for MarkerGroup {}
impl Parent for Tabs {}
impl Parent for Choice {}

// ── Leaves ────────────────────────────────────────────────────────────────────────────────
builder_text!(
    /// A run of text.
    Label => Label
);
builder!(
    /// A button. Its content is its children, or the `text`/`icon` sugar when it has none.
    Button => Button
);
builder!(
    /// An icon-only button.
    IconButton => IconButton
);
builder_text!(
    /// A small status chip.
    Badge => Badge
);
builder_text!(
    /// A clickable status chip.
    BadgeButton => BadgeButton
);
builder_text!(
    /// A coloured tag.
    Tag => Tag
);
builder!(
    /// An icon.
    Icon => Icon
);
builder!(
    /// A text field.
    Input => Input
);
builder!(
    /// A dropdown. Its options are [`Choice`] children.
    Select => Select
);
builder!(
    /// An on/off switch.
    Toggle => Toggle
);
builder!(
    /// A checkbox with a label.
    Checkbox => Checkbox
);
builder!(
    /// A small state dot.
    StatusDot => StatusDot
);
builder!(
    /// A value meter.
    Gauge => Gauge
);
builder!(
    /// A message banner.
    Alert => Alert
);
builder!(
    /// A transient notification card. It draws its own card and takes no children.
    Toast => Toast
);
builder!(
    /// One cell of an icon rail.
    RailCell => RailCell
);
builder!(
    /// A single selectable list row.
    Item => Item
);
builder!(
    /// A thin themed divider.
    Separator => Separator
);

// `Button`'s content is its children, and `Item`'s slots are children too.
impl Parent for Button {}
impl Parent for Item {}
impl Parent for Select {}

with_text!(
    Button, Panel, Input, Choice, Item, Toast, Alert, ItemGroup, DockFrame,
);

with_event!(
    Row { on_press => "press" }
    Button { on_press => "press" }
    IconButton { on_press => "press" }
    BadgeButton { on_press => "press" }
    Item { on_press => "press" }
    RailCell { on_press => "press" }
    Choice { on_press => "press" }
    Input { on_change => "change" }
    Toggle { on_change => "change" }
    Checkbox { on_change => "change" }
    Select { on_change => "change" }
    Tabs { on_change => "change" }
    ItemGroup { on_toggle => "toggle" }
    DockFrame { on_toggle => "toggle" }
    Toast { on_action => "action", on_dismiss => "dismiss" }
);

// ── The per-kind properties ───────────────────────────────────────────────────────────────
// Each block mirrors that widget's generated `PROP_NAMES`, and the drift guard in
// `heca-view-realize` fails when one falls behind.

impl Label {
    /// How the text sits in its box.
    pub fn align_text(self, a: ViewTextAlign) -> Self {
        self.prop("align", a)
    }
    /// Cut the text to fit its box, with the ellipsis at this end. Unset ⇒ the text keeps its
    /// natural width and a container too small for it overflows.
    pub fn truncate(self, mode: ViewEllipsis) -> Self {
        self.prop("truncate", mode)
    }
    /// Reflow the text onto as many lines as its width needs, breaking on word boundaries.
    /// Mutually exclusive with [`truncate`](Self::truncate) — a label either cuts or wraps.
    pub fn wrap(self, on: bool) -> Self {
        self.prop("wrap", on)
    }
    /// Text colour: a theme token name or a literal.
    pub fn color(self, colour: &str) -> Self {
        self.prop("color", PropValue::Color(colour.to_string()))
    }
    /// Draw in the theme's muted colour — secondary text, like a description under a title.
    pub fn muted(self, on: bool) -> Self {
        self.prop("muted", on)
    }
    /// The colour matched characters are drawn in. Unset ⇒ the theme accent.
    pub fn mark_color(self, colour: &str) -> Self {
        self.prop("mark_color", PropValue::Color(colour.to_string()))
    }
    /// Heavier weight.
    pub fn bold(self, on: bool) -> Self {
        self.prop("bold", on)
    }
    /// Slanted.
    pub fn italic(self, on: bool) -> Self {
        self.prop("italic", on)
    }
    /// A line under the text.
    pub fn underline(self, on: bool) -> Self {
        self.prop("underline", on)
    }
    /// A line through the text.
    pub fn strikethrough(self, on: bool) -> Self {
        self.prop("strikethrough", on)
    }
}

impl Button {
    /// A leading icon.
    pub fn icon(self, glyph: ViewGlyph) -> Self {
        self.prop("icon", glyph)
    }
    /// Which visual variant — primary, destructive, and so on.
    pub fn variant(self, v: ViewVariant) -> Self {
        self.prop("variant", v)
    }
    /// A glow halo at rest.
    pub fn glowing(self, on: bool) -> Self {
        self.prop("glow", on)
    }
    /// Draw a border.
    pub fn bordered(self, on: bool) -> Self {
        self.prop("bordered", on)
    }
}

impl IconButton {
    /// The cell's square size in px.
    pub fn cell(self, px: f32) -> Self {
        self.prop("cell", px)
    }
    /// Icon colour: a theme token name or a literal.
    pub fn tone(self, colour: &str) -> Self {
        self.prop("tone", PropValue::Color(colour.to_string()))
    }
    /// A glow halo.
    pub fn glowing(self, on: bool) -> Self {
        self.prop("glow", on)
    }
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
}

impl Badge {
    /// Which visual variant.
    pub fn variant(self, v: ViewVariant) -> Self {
        self.prop("variant", v)
    }
}

impl BadgeButton {
    /// Which visual variant.
    pub fn variant(self, v: ViewVariant) -> Self {
        self.prop("variant", v)
    }
}

impl Tag {
    /// Tag colour: a theme token name or a literal.
    pub fn color(self, colour: &str) -> Self {
        self.prop("color", PropValue::Color(colour.to_string()))
    }
}

impl Icon {
    /// Which icon.
    pub fn glyph(self, glyph: ViewGlyph) -> Self {
        self.prop("glyph", glyph)
    }
    /// Size in px.
    pub fn size_px(self, px: f32) -> Self {
        self.prop("size", px)
    }
    /// Primary colour: a theme token name or a literal.
    pub fn color(self, colour: &str) -> Self {
        self.prop("color", PropValue::Color(colour.to_string()))
    }
    /// The second layer's colour, for duotone icons.
    pub fn secondary_color(self, colour: &str) -> Self {
        self.prop("secondary_color", PropValue::Color(colour.to_string()))
    }
    /// A glow halo.
    pub fn glowing(self, on: bool) -> Self {
        self.prop("glow", on)
    }
}

impl Input {
    /// The text shown when the field is empty.
    pub fn placeholder(self, text: impl Into<String>) -> Self {
        self.prop("placeholder", PropValue::Text(text.into()))
    }
    /// The current value.
    pub fn value(self, text: impl Into<String>) -> Self {
        self.prop("value", PropValue::Text(text.into()))
    }
    /// The name this field reports under when the form is submitted.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.prop("name", PropValue::Text(name.into()))
    }
}

impl Select {
    /// Which option is chosen, by position.
    pub fn selected(self, index: usize) -> Self {
        self.prop("selected", index)
    }
}

impl Tabs {
    /// Which tab is chosen, by position.
    pub fn selected(self, index: usize) -> Self {
        self.prop("selected", index)
    }
}

impl Choice {
    /// What this option means, independently of what it shows.
    pub fn value(self, value: impl Into<String>) -> Self {
        self.prop("value", PropValue::Text(value.into()))
    }
    /// Show as chosen.
    pub fn selected(self, on: bool) -> Self {
        self.prop("selected", on)
    }
}

impl Toggle {
    /// Whether it is on.
    pub fn on(self, on: bool) -> Self {
        self.prop("on", on)
    }
}

impl Checkbox {
    /// Whether it is ticked.
    pub fn checked(self, on: bool) -> Self {
        self.prop("checked", on)
    }
    /// The label beside the box.
    pub fn label(self, text: impl Into<String>) -> Self {
        self.prop("label", PropValue::Text(text.into()))
    }
    /// Which side the label sits on.
    pub fn label_side(self, side: ViewLabelSide) -> Self {
        self.prop("label_side", side)
    }
}

impl Gauge {
    /// The value, 0..=1.
    pub fn value(self, value: f32) -> Self {
        self.prop("value", value)
    }
}

impl Alert {
    /// How serious the message is.
    pub fn severity(self, s: ViewSeverity) -> Self {
        self.prop("variant", s)
    }
    /// The supporting line under the title.
    pub fn body(self, text: impl Into<String>) -> Self {
        self.prop("body", PropValue::Text(text.into()))
    }
}

impl Toast {
    /// How serious the message is.
    pub fn severity(self, s: ViewSeverity) -> Self {
        self.prop("severity", s)
    }
    /// A leading icon.
    pub fn icon(self, glyph: ViewGlyph) -> Self {
        self.prop("icon", glyph)
    }
    /// The supporting line under the title.
    pub fn body(self, text: impl Into<String>) -> Self {
        self.prop("body", PropValue::Text(text.into()))
    }
    /// Whether it can be dismissed.
    pub fn dismissible(self, on: bool) -> Self {
        self.prop("dismissible", on)
    }
    /// The label on its inline action.
    pub fn action_text(self, text: impl Into<String>) -> Self {
        self.prop("action_text", PropValue::Text(text.into()))
    }
}

impl Row {
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// How the active state is shown.
    pub fn marker(self, m: ViewMarker) -> Self {
        self.prop("marker", m)
    }
    /// Show the navigation cursor on this row.
    pub fn nav_selected(self, on: bool) -> Self {
        self.prop("nav_selected", on)
    }
    /// The hover/active highlight colour: a theme token name or a literal.
    pub fn highlight(self, colour: &str) -> Self {
        self.prop("highlight", PropValue::Color(colour.to_string()))
    }
    /// The colour of the attention pulse.
    pub fn attention_color(self, colour: &str) -> Self {
        self.prop("attention_color", PropValue::Color(colour.to_string()))
    }
}

impl Item {
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// How the active state is shown.
    pub fn marker(self, m: ViewMarker) -> Self {
        self.prop("marker", m)
    }
    /// Dim the row.
    pub fn muted(self, on: bool) -> Self {
        self.prop("muted", on)
    }
    /// Draw a border around the leading slot.
    pub fn leading_bordered(self, on: bool) -> Self {
        self.prop("leading_bordered", on)
    }
    /// Draw a border around the trailing slot.
    pub fn trailing_bordered(self, on: bool) -> Self {
        self.prop("trailing_bordered", on)
    }
}

impl ItemGroup {
    /// Whether the group is open.
    pub fn expanded(self, on: bool) -> Self {
        self.prop("expanded", on)
    }
}

impl DockFrame {
    /// Whether the frame is open.
    pub fn expanded(self, on: bool) -> Self {
        self.prop("expanded", on)
    }
    /// Drop the frame's own border.
    pub fn frameless(self, on: bool) -> Self {
        self.prop("frameless", on)
    }
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// Show the navigation cursor.
    pub fn nav_selected(self, on: bool) -> Self {
        self.prop("nav_selected", on)
    }
}

impl MarkerGroup {
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// Show the navigation cursor.
    pub fn nav_selected(self, on: bool) -> Self {
        self.prop("nav_selected", on)
    }
}

impl RailCell {
    /// The cell's square size in px.
    pub fn cell_size(self, px: f32) -> Self {
        self.prop("cell_size", px)
    }
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
}

impl Panel {
    /// The heading above the rule.
    pub fn title(self, text: impl Into<String>) -> Self {
        self.prop("title", PropValue::Text(text.into()))
    }
}

impl Scroll {
    /// Which way it scrolls.
    pub fn axes(self, axes: ViewScrollAxes) -> Self {
        self.prop("axes", axes)
    }
}

impl Separator {
    /// Which way the rule runs.
    pub fn orientation(self, o: ViewOrientation) -> Self {
        self.prop("orientation", o)
    }
    /// How long the rule is, in px. Unset, it stretches to its container.
    pub fn length(self, px: f32) -> Self {
        self.prop("length", px)
    }
}

impl Grid {
    /// The column track template — CSS-like strings (`"1fr"`, `"22px"`, `"auto"`).
    pub fn columns<S: Into<String>>(self, tracks: impl IntoIterator<Item = S>) -> Self {
        self.prop("columns", track_list(tracks))
    }
    /// The row track template.
    pub fn rows<S: Into<String>>(self, tracks: impl IntoIterator<Item = S>) -> Self {
        self.prop("rows", track_list(tracks))
    }
    /// The named areas, one string per grid row.
    pub fn areas<S: Into<String>>(self, areas: impl IntoIterator<Item = S>) -> Self {
        self.prop("areas", track_list(areas))
    }
}

/// A list of CSS-like track strings as a property value.
fn track_list<S: Into<String>>(items: impl IntoIterator<Item = S>) -> PropValue {
    PropValue::List(
        items
            .into_iter()
            .map(|t| PropValue::Text(t.into()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The builders produce the same node hand-authoring does — they are sugar, not a second model.
    #[test]
    fn a_built_tree_is_an_ordinary_view_node() {
        let built = VStack::new()
            .gap(8.0)
            .child(Label::new("nginx").bold(true))
            .into_node();

        let by_hand = ViewNode::new(WidgetKind::VStack)
            .prop("gap", PropValue::Float(8.0))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("nginx")
                    .prop("bold", PropValue::Bool(true)),
            );

        assert_eq!(built, by_hand);
    }

    /// Appearance goes through the builders too, including the struct-shaped half that only became
    /// authorable in F003/P011/T018.
    #[test]
    fn appearance_including_border_and_glow_is_authorable() {
        let node = Surface::new()
            .fill("accent")
            .border("muted", 2.0)
            .glow("accent", 12.0, 0.4)
            .radius(6.0)
            .into_node();

        assert_eq!(node.props.get("fill"), Some(&PropValue::Color("accent".into())));
        assert!(matches!(node.props.get("border"), Some(PropValue::Map(_))));
        assert!(matches!(node.props.get("glow"), Some(PropValue::Map(_))));
    }

    /// A percentage width travels in the spelling `Length`'s deserializer accepts.
    #[test]
    fn a_percentage_width_is_written_the_way_length_reads_it() {
        let node = Surface::new().width_pct(0.5).into_node();
        assert_eq!(node.props.get("width"), Some(&PropValue::Text("50%".into())));
    }

    /// An event lands under the name `realize` looks for.
    #[test]
    fn an_event_is_bound_under_its_canonical_name() {
        let node = Row::new().on_press(Intent::new("docker.select")).into_node();
        assert_eq!(node.events.get("press").map(|i| i.action.as_str()), Some("docker.select"));
    }
}
