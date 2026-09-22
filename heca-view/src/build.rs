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
    Intent, PropMap, PropValue, ViewAlign, ViewAnimation, ViewEllipsis, ViewGlyph, ViewJustify,
    ViewLabelSide, ViewMarker, ViewNode, ViewOrientation, ViewRevealAlign, ViewScrollAxes,
    ViewSeverity, ViewSize, ViewTextAlign, ViewVariant, WidgetKind,
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
/// - **`pad_spacing_x` / `pad_spacing_y` / `gap_spacing`** — the retired half of the old spacing
///   pair. [`gap`](Style::gap) and [`padding`](Style::padding) take a step directly now; the old
///   names are still read on the wire so no existing tree breaks.
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
    /// **Space between children** — a number of pixels, a theme step, or either said as a string.
    ///
    /// ```ignore
    /// VStack::new().gap(8)                 // eight pixels
    /// VStack::new().gap(ViewSpacing::Sm)   // a step of the rhythm
    /// VStack::new().gap("sm")              // the same step, said as JSON would
    /// ```
    ///
    /// **Prefer the step.** It is resolved from the inherited font at layout, so a described tree
    /// spaces itself the way the rest of the app does and follows a font, size-variant or theme
    /// change with nothing rewritten; a pixel count is tuned for one font size and wrong at every
    /// other. Use the steps to group: a tight `Xs` inside a label-and-control couple, a roomier
    /// `Md` between couples — no arithmetic, and no new widget.
    ///
    /// It was two builders, `gap` and `gap_spacing`, writing two different properties. The docs
    /// said prefer the step and the step was the one nobody reached for, because it had the longer
    /// name. `"gap_spacing"` is still read on the wire, so no existing tree breaks.
    fn gap(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("gap", v.into())
    }

    /// **Space inside the box on every side** — pixels or a step, exactly as [`gap`](Style::gap).
    fn padding(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding", v.into())
    }
    /// Horizontal padding (left + right) — see [`padding`](Style::padding).
    fn padding_x(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_x", v.into())
    }
    /// Vertical padding (top + bottom) — see [`padding`](Style::padding).
    fn padding_y(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_y", v.into())
    }
    /// Left padding — see [`padding`](Style::padding).
    fn padding_left(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_left", v.into())
    }
    /// Right padding — see [`padding`](Style::padding).
    fn padding_right(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_right", v.into())
    }
    /// Top padding — see [`padding`](Style::padding).
    fn padding_top(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_top", v.into())
    }
    /// Bottom padding — see [`padding`](Style::padding).
    fn padding_bottom(self, v: impl Into<crate::ViewSpace>) -> Self {
        self.prop("padding_bottom", v.into())
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

    // ── Identity ──
    /// **This node's identity, when it is one of a collection you are iterating** — React's `key`,
    /// meaning exactly what it means there.
    ///
    /// It is on this trait, not on one builder, for the reason `ComponentExt::key` is on every
    /// widget: a collection can be built from any kind, so the identity of an item cannot belong to
    /// a particular one. `realize` reads it once for every kind and writes it into the widget's own
    /// slot, so a described row is identified exactly as a native row is — the cursor, the
    /// right-click target, the drag identity and the picker's remembered letter all read that one
    /// string.
    ///
    /// **Two cases, and only two:**
    ///
    /// | what you are building | what you write |
    /// |---|---|
    /// | anything at all — a button, an icon, a card | **nothing** |
    /// | an item in a collection you are iterating | **`.key(…)`** — the item's own id, from your data |
    ///
    /// Everything else gets an identity anyway, derived from its content. What derivation cannot do
    /// is tell apart several nodes that read the same, and that is what iterating produces — so
    /// this is required in a collection and nowhere else.
    ///
    /// **You never count.** A key is never a position and never a counter: an index is the one
    /// thing that changes when the list changes, which is what identity exists to survive. If you
    /// are reaching for a counter, the key is wrong.
    ///
    /// ```ignore
    /// for pane in &column.panes {
    ///     Row::new().key(pane.id).on_press(Intent::new("focus_pane").arg("pane_id", pane.id))
    /// }
    /// ```
    fn key(self, key: impl Into<String>) -> Self {
        self.prop("key", PropValue::Text(key.into()))
    }

    /// **What this node says on hover.**
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::ViewGlyph;
    ///
    /// let close = Button::new().icon(ViewGlyph::Minus).tooltip("Close the pane");
    /// ```
    ///
    /// Universal, like [`key`](Style::key), because `ComponentExt::tooltip` is on every widget
    /// natively — so a described row says it the same way a native one does, and the framework owns
    /// the rest: the reveal delay is timed off the hover clock the pointer router already keeps,
    /// and the bubble is drawn in the one place every widget passes through.
    ///
    /// ⚠️ **There is no `Tooltip` widget kind, and there must not be one.** The wrapper it used to
    /// be survives only for regions that are not widgets you can put a builder on. A plugin made to
    /// wrap and anchor its own bubble is writing the second path by hand (⭐⭐ RULE ZERO).
    fn tooltip(self, text: impl Into<String>) -> Self {
        self.prop("tooltip", PropValue::Text(text.into()))
    }

    /// Which side the tooltip anchors to (default [`Top`](crate::ViewTooltipSide::Top)).
    ///
    /// A preference: the framework flips it when there is no room on that side. Says nothing on a
    /// node that declared no [`tooltip`](Style::tooltip) — the side is part of the tip, not a style
    /// of its own.
    fn tooltip_side(self, side: crate::ViewTooltipSide) -> Self {
        self.prop("tooltip_side", side)
    }

    /// Seconds the pointer must rest before the tooltip appears (default `0.5`).
    ///
    /// Says nothing on a node that declared no [`tooltip`](Style::tooltip).
    fn tooltip_delay(self, seconds: f32) -> Self {
        self.prop("tooltip_delay", seconds)
    }

    /// **Whether this node's ink is drawn**, keeping its box either way — CSS `visibility`.
    ///
    /// ```
    /// use heca_view::build::*;
    ///
    /// // The row does not shift when this dot appears.
    /// let quiet = StatusDot::new().visible(false);
    /// ```
    ///
    /// **The other one is `hidden`** — CSS `display: none`, a [`Style`] property like any other,
    /// which takes the node out of the layout so its neighbours close up. This keeps the box.
    ///
    /// Between them there is nothing left for a `Visibility` wrapper to do in a described tree, and
    /// so there is no `WidgetKind` for one.
    fn visible(self, visible: bool) -> Self {
        self.prop("visible", visible)
    }

    /// **Where this node's hint letter sits over it** (default
    /// [`TopCenter`](crate::ViewHintPlacement::TopCenter)).
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::ViewHintPlacement;
    ///
    /// // A wide list row: the cap on the right keeps the row's own label readable.
    /// let row = Row::new().hint_placement(ViewHintPlacement::CenterRight);
    /// ```
    ///
    /// Universal, like [`tooltip`](Style::tooltip), because the slot it writes is on every widget.
    /// **Being pickable is not what this turns on** — anything actionable already wears a letter
    /// with nothing declared; this only says where the letter goes.
    fn hint_placement(self, placement: crate::ViewHintPlacement) -> Self {
        self.prop("hint_placement", placement)
    }

    /// **Which pickers letter this target.** Unset — the default — means the ordinary one, which
    /// letters everything actionable.
    ///
    /// One surface can mean more than one thing by a letter: a card that means *go there* and a ⊠
    /// beside it that means *remove that*. A picker that letters both hands out twice the letters
    /// and half of them do the wrong one, so a target names the sets it answers to and a picker
    /// names the set it letters.
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::Intent;
    ///
    /// // Lettered only by a picker that asked for "close" — never by the ordinary one.
    /// let close = IconButton::new().on_press(Intent::new("card.remove")).hint_scope(["close"]);
    /// ```
    ///
    /// **Naming a scope takes the target OUT of the ordinary picker** — that is the point, and the
    /// direction matters: a ⊠ that deletes something must not wear a letter in the picker you use
    /// to move around. Name several and it belongs to each of those pickers.
    ///
    /// Universal, like [`hint_placement`](Style::hint_placement), because the slot is on every
    /// widget.
    /// **What this node's keycap means** — the theme picks the colour, so it follows a reload and
    /// a description never carries a hex.
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::{Intent, ViewHintTone};
    ///
    /// // A fold control: a real act, but not somewhere to navigate to.
    /// let fold = IconButton::new().on_press(Intent::new("panel.fold")).hint_tone(ViewHintTone::Muted);
    /// ```
    fn hint_tone(self, tone: crate::ViewHintTone) -> Self {
        self.prop("hint_tone", tone)
    }

    fn hint_scope(self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let list: Vec<PropValue> = scopes
            .into_iter()
            .map(|s| PropValue::Text(s.into()))
            .collect();
        self.prop("hint_scope", PropValue::List(list))
    }

    /// The keycap's font size in logical px. Unset = derived from the node's resolved font, which
    /// is what keeps a letter proportional to the thing it captions.
    fn hint_size(self, px: f32) -> Self {
        self.prop("hint_size", px)
    }

    /// The keycap's colour **by theme token name** (`"accent"`, `"danger"`), glow included. Unset =
    /// the theme's `accent`.
    ///
    /// A token, not a hex literal, for the same reason every other colour here is one: a letter
    /// should follow a theme change with nothing rewritten.
    fn hint_color(self, colour: &str) -> Self {
        self.prop("hint_color", PropValue::Color(colour.to_string()))
    }

    /// A vertical nudge applied **after** placement — positive moves the cap down. What drops a
    /// [`TopRight`](crate::ViewHintPlacement::TopRight) cap onto a dock's header line.
    fn hint_offset_y(self, px: f64) -> Self {
        self.prop("hint_offset_y", px)
    }

    /// Keep this node **out of the picker**, however actionable it is. Default `true`.
    ///
    /// The declarative spelling of `ComponentExt::hintable`. Being pickable is not opt-in — a node
    /// with a `press` wears a letter with nothing written — so the only thing left to say is "not
    /// me". Letters are scarce (52, one keystroke each), and a close button on every row of a long
    /// list would spend one apiece.
    fn hintable(self, hintable: bool) -> Self {
        self.prop("hintable", hintable)
    }

    /// **Declare a verb this node answers to**, by name — the declarative
    /// `heca_grid_ui::ComponentExt::on_action`.
    ///
    /// The node names the verb; config names the key. That division is the whole of it: a key
    /// written into a description would be unrebindable, absent from the palette and unreachable
    /// over RPC.
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::Intent;
    ///
    /// // [[keys.surface]] name = "mypanel" / reload = "r"   →   mypanel.reload
    /// let panel = Panel::new().on_action("mypanel.reload", Intent::new("docker.refresh"));
    /// ```
    ///
    /// Universal, like [`key`](Style::key), because `on_action` is on every widget natively — this
    /// is what lets a described **surface** own a verb instead of borrowing one the app compiled in
    /// (F003/P082/T436). Whichever node on screen declares the name is the one that runs it, so a
    /// verb whose surface is not up resolves to nothing, exactly as an unmounted provider's does.
    ///
    /// ⚠️ [`Toast`] has an inherent `on_action` of its own — the **event** its action button fires,
    /// which takes an intent alone. Its arity differs, so the compiler says which you reached.
    fn on_action(mut self, name: impl Into<String>, intent: Intent) -> Self {
        self.node_mut().actions.insert(name.into(), intent);
        self
    }

    // ── Appearance ──
    /// **Override the accent — for this node and everything inside it.**
    ///
    /// Focus rings, hover and press fills, selected washes, scrollbar thumbs, a caret: everything
    /// that would otherwise be the theme's accent. A panel with a hue of its own says it once and
    /// every control inside follows, with nothing told twice.
    ///
    /// ```ignore
    /// Surface::new().accent("danger").child(Button::new().text("Delete"))
    /// ```
    ///
    /// **The theme is always first.** Set nothing and everything reads the theme's accent. The
    /// order is: this node's own → the nearest ancestor that set one → the theme.
    ///
    /// A theme token name (`"danger"`) or a literal (`"#ff8800"`) — prefer a token, which follows a
    /// theme reload where a literal does not.
    ///
    /// ⚠️ It does **not** redefine a declared meaning: a badge's `accent` variant, an alert's
    /// `info`, a destructive button. Those keep the colour their variant names.
    fn accent(self, colour: &str) -> Self {
        self.prop("accent", PropValue::Color(colour.to_string()))
    }
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
    /// **Append a child, or several** — one node, or a `Vec`/array of them. One door, the same as
    /// `ViewNode::child` and the native `Parent::child`.
    fn child(mut self, children: impl crate::IntoNodes) -> Self {
        self.node_mut().children.extend(children.into_nodes());
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
builder!(
    /// A grid of cards with a cursor: arrow keys move it, hovering moves it, Enter activates and
    /// Escape dismisses. Each child is a card, and its own `key` is what activation hands back.
    CardGrid => CardGrid
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
    /// **A surface over the page**: a scrim, a panel holding the children, and how it arrives and
    /// leaves ([`animation`](Overlay::animation)).
    ///
    /// ```ignore
    /// Overlay::new()
    ///     .animation(ViewAnimation::ZoomFade)
    ///     .child(Panel::new().title("MAP").child(Label::new("…")))
    /// ```
    Overlay => Overlay
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

builder!(
    /// **A picker over its own children.** Open it and everything pickable beneath wears a letter;
    /// typing one runs that node's `hint`. A transparent wrapper the rest of the time.
    ///
    /// [`opens_on`](KeyHintGroup::opens_on) is the whole of it — see there for why a picker needs
    /// no key of its own and no state from the host.
    KeyHintGroup => KeyHintGroup
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
impl Parent for Overlay {}
impl Parent for MarkerGroup {}
impl Parent for Tabs {}
impl Parent for Choice {}
impl Parent for KeyHintGroup {}
impl Parent for ButtonGroup {}

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
    /// An indeterminate loading ring — *something is happening, and nobody knows for how long*.
    ///
    /// It takes no properties of its own: it animates itself off the frame clock, and its diameter
    /// is `width`/`height` like any other node's. Reach for [`Progress`] the moment you can say how
    /// far along you are.
    Spinner => Spinner
);
builder!(
    /// A determinate progress bar, `0.0..=1.0`.
    ///
    /// The fill **eases** toward whatever value it is given, so a tree re-sent with a new one
    /// animates rather than jumping — with nothing declared.
    Progress => Progress
);
builder!(
    /// **A row of actions that gets out of its own way.** Its children are [`Button`]s.
    ///
    /// As the room runs out it shows icons instead of words, and whatever still does not fit
    /// collapses into a ⋮ menu running the same actions — none of which you write. Give each button
    /// **both** `text` and `icon`: the text is its menu row and its words on hover, the icon is what
    /// it shows once there is no room for words.
    ButtonGroup => ButtonGroup
);
builder!(
    /// A **keyboard glyph** from the embedded Nerd Font — ⇧ ⌃ ⌥ ⌘, Enter, Escape, the arrows.
    ///
    /// Its own vocabulary ([`ViewNfGlyph`](crate::ViewNfGlyph)), because it is its own font. Draw a
    /// shortcut with it rather than typing a character your user's font may not carry.
    NfIcon => NfIcon
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
    // **`hint` is on every kind, because `on_hint` is on every widget.**
    //
    // Natively a hint is `ComponentExt::on_hint` — universal, on `Base`, no widget opts in
    // (F003/P082/T432) — and `realize` writes a node's `hint` event into that same slot for every
    // kind, in the common path rather than a per-kind arm. The SDK offered it on seven kinds, so an
    // author writing a `Label`, a `Card` or a `Panel` could draw the thing and never make it
    // pickable, while the very same tree written as a raw `ViewNode` could. That is the drift
    // `every_kind_can_be_given_a_hint_from_the_sdk` was written red to catch (F003/P082/T434).
    //
    // A hint is enough on its own: a node carrying one **is** a pick target, whether or not it can
    // be clicked (`heca_grid_ui::hint::is_target` — `hintable && (hint.is_some() || actionable)`).
    // So this is not "hint beside press"; it is a capability of every node, listed here because
    // this table is where a builder gets its event setters.
    //
    // `press` stays on the kinds `realize` actually wires a click for — that is a per-kind arm, and
    // a `press` on a `Separator` would be a setter that silently does nothing.
    //
    // `ScrollBar` is absent from this table and from the SDK entirely: host-only, its state is live
    // host signals, and `realize` refuses it outright rather than render a dead control.
    Row { on_press => "press", on_hint => "hint" }
    Button { on_press => "press", on_hint => "hint" }
    IconButton { on_press => "press", on_hint => "hint" }
    BadgeButton { on_press => "press", on_hint => "hint" }
    Item { on_press => "press", on_hint => "hint" }
    RailCell { on_press => "press", on_hint => "hint" }
    Choice { on_press => "press", on_hint => "hint" }
    Input { on_change => "change", on_hint => "hint" }
    Toggle { on_change => "change", on_hint => "hint" }
    Checkbox { on_change => "change", on_hint => "hint" }
    Select { on_change => "change", on_hint => "hint" }
    Tabs { on_change => "change", on_hint => "hint" }
    ItemGroup { on_toggle => "toggle", on_hint => "hint" }
    DockFrame { on_toggle => "toggle", on_hint => "hint" }
    Toast { on_action => "action", on_dismiss => "dismiss", on_hint => "hint" }

    // The kinds whose only event is the pick. Nothing here is clickable through a description —
    // `realize` wires no `press` for them — but every one of them can be *picked*, and several
    // want to be: a `Card` standing for a thing, a `Panel` heading a plugin's section, a `Label`
    // that is the only handle on a row.
    VStack { on_hint => "hint" }
    HStack { on_hint => "hint" }
    Grid { on_hint => "hint" }
    CardGrid { on_hint => "hint" }
    Card { on_hint => "hint" }
    Scroll { on_hint => "hint" }
    Panel { on_hint => "hint" }
    Surface { on_hint => "hint" }
    Overlay { on_dismiss => "dismiss", on_hint => "hint" }
    KeyHintGroup { on_hint => "hint" }
    MarkerGroup { on_hint => "hint" }
    Label { on_hint => "hint" }
    Badge { on_hint => "hint" }
    Tag { on_hint => "hint" }
    Icon { on_hint => "hint" }
    StatusDot { on_hint => "hint" }
    Gauge { on_hint => "hint" }
    Spinner { on_hint => "hint" }
    NfIcon { on_hint => "hint" }
    ButtonGroup { on_hint => "hint" }
    Progress { on_hint => "hint" }
    Alert { on_hint => "hint" }
    Separator { on_hint => "hint" }
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
    /// **Held on** — a toggled status rather than a transient hover, painted as a persistent
    /// tone-tinted wash and a firm border under whatever the variant draws. The same state, and the
    /// same look, as an icon button's.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// **Show only the icon, keeping the words** — the label is not drawn and takes no space, and
    /// the button becomes square, since the horizontal room a button reserves exists for text. The
    /// words are still carried: they are what it says on hover, and what its row reads if a button
    /// group moves it into a menu. A button with no icon ignores it.
    pub fn icon_only(self, on: bool) -> Self {
        self.prop("icon_only", on)
    }
}

impl IconButton {
    /// The cell's square size in px.
    pub fn cell(self, px: f32) -> Self {
        self.prop("cell", px)
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

impl ButtonGroup {
    /// How wide each action is before the row starts collapsing (default
    /// [`IconOnly`](crate::ViewDisplay::IconOnly)).
    pub fn display(self, display: crate::ViewDisplay) -> Self {
        self.prop("display", display)
    }
    /// The look every action in the group takes, so it is said once rather than per button.
    pub fn variant(self, variant: ViewVariant) -> Self {
        self.prop("variant", variant)
    }
}

impl NfIcon {
    /// Which key this glyph is.
    pub fn glyph(self, glyph: crate::ViewNfGlyph) -> Self {
        self.prop("glyph", glyph)
    }
    /// Explicit glyph size in logical px. Unset = the inherited font size, which is what keeps a
    /// key glyph the size of the text beside it.
    pub fn size(self, px: f32) -> Self {
        self.prop("size", px)
    }
    /// Glyph colour **by theme token name**. Unset = the enclosing control's content colour.
    pub fn color(self, colour: &str) -> Self {
        self.prop("color", PropValue::Color(colour.to_string()))
    }
}

impl Progress {
    /// How far along, `0.0..=1.0`. Out-of-range values are clamped rather than refused, because a
    /// description is untrusted input and a bar that renders nothing is worse than a full one.
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
    /// The supporting line under the title — the common body, said in one string.
    ///
    /// A body that is more than a line of text is composed instead: give the card children in the
    /// `body` slot and they become its body.
    pub fn body_text(self, text: impl Into<String>) -> Self {
        self.prop("body_text", PropValue::Text(text.into()))
    }
    /// Whether it starts on screen. A described card that is closed takes no space until something
    /// opens it.
    pub fn default_open(self, open: bool) -> Self {
        self.prop("default_open", open)
    }
    /// Where the card sits in the box that holds it: `"top-right"` (the default), `"top-left"`,
    /// `"top-center"`, `"bottom-right"`, `"bottom-left"`, `"bottom-center"`.
    pub fn position(self, position: impl Into<String>) -> Self {
        self.prop("position", PropValue::Text(position.into()))
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
    /// Mark as the one a **back-and-forth** binding would return to — the faintest of the
    /// selection weights, under every other state.
    pub fn previous(self, on: bool) -> Self {
        self.prop("previous", on)
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
    /// The colour of the **"you were just here"** mark, overriding the theme's row-scale
    /// `previous_background`.
    ///
    /// The pair with [`highlight`](Row::highlight), for the same reason: the theme's default is
    /// tuned for a row in a list, and the same tint on a much larger surface reads differently —
    /// area changes how a lift reads. A token name or a literal.
    pub fn previous_tint(self, colour: &str) -> Self {
        self.prop("previous_tint", PropValue::Color(colour.to_string()))
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
    /// Mark as the one a **back-and-forth** binding would return to — the faintest of the
    /// selection weights, under every other state.
    pub fn previous(self, on: bool) -> Self {
        self.prop("previous", on)
    }
    /// **What the fold control's letter means**, for the theme to colour.
    ///
    /// Folding is a real act, so the toggle earns a letter — but which class of target it reads as
    /// belongs to whoever assembles the surface. Unset, it takes the picker's own colour.
    pub fn fold_hint_tone(self, tone: crate::ViewHintTone) -> Self {
        self.prop("fold_hint_tone", tone)
    }

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

impl KeyHintGroup {
    /// **The verb that opens this picker** — one string, and the picker is yours.
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::Intent;
    ///
    /// let panel = KeyHintGroup::new()
    ///     .opens_on("mypanel.pick")
    ///     .child(Row::new().on_hint(Intent::new("docker.restart")));
    /// ```
    ///
    /// ```toml
    /// [[keys.surface]]
    /// name = "mypanel"
    /// pick = "s"          # -> mypanel.pick
    /// ```
    ///
    /// A picker is otherwise the one thing a description could not have: natively it is a signal,
    /// an `open_when` binding it and an `on_action` closure that flips it — three things static
    /// data cannot carry. The widget owns all three behind this name, so a described picker is the
    /// native one and not a cut-down copy (F003/P082/T436).
    pub fn opens_on(self, action: impl Into<String>) -> Self {
        self.prop("opens_on", PropValue::Text(action.into()))
    }

    /// **Which set of targets this picker letters.** Unset — the default — means the ordinary
    /// set: everything beneath it that named no scope.
    ///
    /// A surface with two verbs over one tree gives each its own picker and its own letters:
    ///
    /// ```
    /// use heca_view::build::*;
    /// use heca_view::Intent;
    ///
    /// // Two pickers over the same cards. The first letters the cards, the second only the ⊠s.
    /// let jump  = KeyHintGroup::new().opens_on("map.jump");
    /// let close = KeyHintGroup::new().opens_on("map.close").scope("close");
    /// ```
    ///
    /// A picker whose scope matches nothing shows **no letters** rather than falling back to
    /// lettering everything — the fallback is the dangerous direction, since a "close" picker that
    /// quietly lettered every card would remove what you meant to go to.
    pub fn scope(self, scope: impl Into<String>) -> Self {
        self.prop("scope", PropValue::Text(scope.into()))
    }
}

impl Panel {
    /// The heading above the rule.
    pub fn title(self, text: impl Into<String>) -> Self {
        self.prop("title", PropValue::Text(text.into()))
    }
}

impl Overlay {
    /// **Which control the keyboard starts on**, named by its `key`.
    ///
    /// ```
    /// use heca_view::build::*;
    ///
    /// let confirm = Overlay::new()
    ///     .default_focus("cancel")
    ///     .child(Button::new().key("cancel").text("Cancel"));
    /// ```
    ///
    /// Yours to decide, because only you know which control is safe: a confirm starts on the
    /// button that changes nothing, a form on its first field, a menu on neither. Unset, nothing
    /// is focused.
    pub fn default_focus(self, key: &str) -> Self {
        self.prop("default_focus", PropValue::Text(key.to_string()))
    }

    /// **How it arrives and leaves.** Unset, it cuts.
    ///
    /// These are the built-ins. An animation nobody named is a Rust type handed to
    /// `heca_grid_ui::Overlay::animation`, which is what a plugin writing its own curve uses —
    /// static data cannot carry a live object, so it names one instead.
    pub fn animation(self, animation: ViewAnimation) -> Self {
        self.prop("animation", animation)
    }
    /// Modal (the default): a dimming scrim, and outside input swallowed. Off, outside input falls
    /// through to the page — a light-dismiss popover.
    pub fn blocking(self, on: bool) -> Self {
        self.prop("blocking", PropValue::Bool(on))
    }
    /// Whether it starts up. The starting value only — to follow state you hold, the host binds a
    /// signal with `open_when`. A surface **born** open is already there and plays no arrival; one
    /// that *becomes* open arrives.
    pub fn default_open(self, on: bool) -> Self {
        self.prop("default_open", PropValue::Bool(on))
    }
    /// **Blur what is behind it.** The scrim's counterpart: a scrim tints what is underneath, a
    /// frost takes its detail away, and a surface may want either, both or neither.
    ///
    /// Strength is the theme's `overlay_frost_radius` — a described surface asks for the effect,
    /// never a number, so a theme that wants a flat backdrop answers for every surface at once.
    pub fn frosted(self, on: bool) -> Self {
        self.prop("frosted", PropValue::Bool(on))
    }
}

impl Scroll {
    /// Which way it scrolls.
    pub fn axes(self, axes: ViewScrollAxes) -> Self {
        self.prop("axes", axes)
    }
    /// Where it puts the descendant it follows — minimally in view, or centred.
    pub fn reveal_align(self, align: ViewRevealAlign) -> Self {
        self.prop("reveal_align", align)
    }
    /// Whether a scrollbar may appear (default `true`); off also gives back its gutter.
    pub fn scrollbars(self, on: bool) -> Self {
        self.prop("scrollbars", PropValue::Bool(on))
    }
    /// Let the view move past the ends of its content, so a centred target still reaches the
    /// middle when there is nothing on one side of it.
    pub fn overscroll(self, on: bool) -> Self {
        self.prop("overscroll", PropValue::Bool(on))
    }
    /// Ease the view to where it is going over this many seconds; `0` jumps.
    pub fn smooth_scroll(self, seconds: f32) -> Self {
        self.prop("smooth_scroll", PropValue::Float(seconds as f64))
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

        assert_eq!(
            node.props.get("fill"),
            Some(&PropValue::Color("accent".into()))
        );
        assert!(matches!(node.props.get("border"), Some(PropValue::Map(_))));
        assert!(matches!(node.props.get("glow"), Some(PropValue::Map(_))));
    }

    /// A percentage width travels in the spelling `Length`'s deserializer accepts.
    #[test]
    fn a_percentage_width_is_written_the_way_length_reads_it() {
        let node = Surface::new().width_pct(0.5).into_node();
        assert_eq!(
            node.props.get("width"),
            Some(&PropValue::Text("50%".into()))
        );
    }

    /// An event lands under the name `realize` looks for.
    #[test]
    fn an_event_is_bound_under_its_canonical_name() {
        let node = Row::new()
            .on_press(Intent::new("docker.select"))
            .into_node();
        assert_eq!(
            node.events.get("press").map(|i| i.action.as_str()),
            Some("docker.select")
        );
    }

    /// **One builder writes ONE property, whichever kind of space it was given.**
    ///
    /// The retired names are still read on the far side, so a tree emitting `"gap_spacing"` keeps
    /// working — which means nothing downstream can tell you the builder picked the wrong name.
    /// This is the only place that can, so it pins the name rather than the effect.
    #[test]
    fn spacing_is_written_under_one_property_name() {
        let px = VStack::new().gap(8).into_node();
        assert_eq!(px.props.get("gap"), Some(&PropValue::Float(8.0)));
        assert!(
            !px.props.contains_key("gap_spacing"),
            "the retired name must not be emitted — it is read, not written",
        );

        let step = VStack::new().gap(crate::ViewSpacing::Sm).into_node();
        assert_eq!(
            step.props.get("gap"),
            Some(&PropValue::Text("sm".into())),
            "a step travels as its name, under the same property",
        );
        assert!(!step.props.contains_key("gap_spacing"));
    }

    /// Padding does the same, and a string is read as either kind.
    #[test]
    fn padding_is_written_under_one_property_name() {
        let node = Surface::new().padding("md").padding_x(4).into_node();
        assert_eq!(
            node.props.get("padding"),
            Some(&PropValue::Text("md".into()))
        );
        assert_eq!(node.props.get("padding_x"), Some(&PropValue::Float(4.0)));
        for retired in ["pad_spacing_x", "pad_spacing_y"] {
            assert!(
                !node.props.contains_key(retired),
                "{retired} must not be emitted"
            );
        }
    }
}
