use super::{Parent, Style};
use crate::{
    PropValue, ViewAnimation, ViewMarker, ViewNode, ViewRevealAlign, ViewScrollAxes, WidgetKind,
};

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
    /// **A picker over its own children.** Open it and everything pickable beneath wears a letter;
    /// typing one runs that node's `hint`. A transparent wrapper the rest of the time.
    ///
    /// [`opens_on`](KeyHintGroup::opens_on) is the whole of it — see there for why a picker needs
    /// no key of its own and no state from the host.
    KeyHintGroup => KeyHintGroup
);

builder!(
    /// **A thing, said in a line or a few** — status pip, icon, title, suffix, and lines under it.
    ///
    /// ```ignore
    /// Row::new().on_press(Intent::new("docker.open")).child(
    ///     Tile::new()
    ///         .child(StatusDot::new().prop("slot", "status"))
    ///         .child(Icon::new().glyph(ViewGlyph::Terminal).prop("slot", "icon"))
    ///         .child(Label::new("nginx").bold(true).prop("slot", "title"))
    ///         .child(Label::new("(web)").prop("slot", "suffix"))
    ///         .child(Label::new("up 3 days")),          // no slot: a line under the head
    /// )
    /// ```
    Tile => Tile
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
impl Parent for KeyHintGroup {}
impl Parent for Tile {}

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
