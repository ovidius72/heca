use super::Style;
use crate::{
    PropValue, ViewEllipsis, ViewGlyph, ViewNode, ViewOrientation, ViewSeverity, ViewTextAlign,
    ViewVariant, WidgetKind,
};

builder_text!(
    /// A run of text.
    Label => Label
);

builder_text!(
    /// A small status chip.
    Badge => Badge
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
    /// A thin themed divider.
    Separator => Separator
);

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
    /// The most lines a wrapping label may take; text that needs more ends its last line with `…`.
    /// Only meaningful with [`wrap`](Self::wrap). Unset ⇒ as many lines as it needs.
    pub fn max_lines(self, n: usize) -> Self {
        self.prop("max_lines", n)
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

impl Badge {
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

impl Gauge {
    /// The value, 0..=1.
    pub fn value(self, value: f32) -> Self {
        self.prop("value", value)
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
    /// The most lines the title and the body text may each take; text that needs more ends its last
    /// line with `…`. Unset ⇒ as many lines as it needs — the text wraps, it is never cut to one.
    pub fn max_lines(self, n: usize) -> Self {
        self.prop("max_lines", n)
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
