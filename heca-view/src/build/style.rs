use crate::{Intent, PropMap, PropValue, ViewAlign, ViewEvent, ViewJustify, ViewNode, ViewSize};

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

    /// **CSS `flex: <n>` — take `n` parts of the room the parent has to give**, whatever this node
    /// holds. `1.0` beside `3.0` is a quarter and three quarters; `0.0` is as big as its content.
    /// Nothing in a `Grid`, whose tracks size its cells. The same builder as native
    /// `LayoutExt::flex`, written as the `flex` property.
    ///
    /// Prefer it to [`flex_grow`](Self::flex_grow), which divides only what is left after every
    /// child took its content, so children holding different amounts never land in their ratio.
    fn flex(self, parts: f32) -> Self {
        self.prop("flex", parts)
    }
    /// **CSS `order` — where this node is laid out among its siblings.** Lower first; ties and
    /// nodes that say nothing keep the order they were added. `3`, or `[0, 5]` to slot between two
    /// siblings at `0` and `1`. Visual only: paint, Tab and the letter picker keep the tree's order.
    /// The same builder as native `LayoutExt::order`, written as the `order` property.
    fn order(self, order: impl Into<crate::ViewOrder>) -> Self {
        self.prop("order", order.into())
    }
    /// Share of leftover space this node takes (CSS `flex-grow` alone). Usually you want
    /// [`flex`](Self::flex).
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

    /// Show the tooltip after the library's shorter rest instead of the default — a name instead of
    /// a number, for a tip that carries what the node had to cut short.
    ///
    /// Says nothing on a node that declared no [`tooltip`](Style::tooltip).
    fn tooltip_quick(self, quick: bool) -> Self {
        self.prop("tooltip_quick", quick)
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

    /// **What a leader-key pick does to this node**, when that is not simply what a click does.
    ///
    /// Unbound, a pick falls back to `press`, so every actionable node is reachable by letter for
    /// free. Bind it when the two genuinely differ: heca's sidebar row activates the pane and leaves
    /// the sidebar on a click, and stays in the sidebar on a hint pick.
    ///
    /// On every kind, because natively a hint is `ComponentExt::on_hint` — universal, on `Base`, no
    /// widget opts in — and `realize` writes a node's `hint` event into that same slot for every
    /// kind. A node carrying one **is** a pick target, whether or not it can be clicked. It is one
    /// method here, not a row per kind in the event table, so a kind added later cannot be left
    /// without it.
    fn on_hint(mut self, intent: Intent) -> Self {
        self.node_mut()
            .events
            .insert(ViewEvent::Hint.name().to_string(), intent);
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

/// A `Length` percentage, in the spelling its deserializer accepts.
fn pct(fraction: f32) -> PropValue {
    PropValue::Text(format!("{}%", fraction * 100.0))
}
