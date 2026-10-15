//! **The hue a widget reads in**, for itself and what it holds.

use crate::color::Color;
use crate::component::Component;

/// **The hue a widget reads in** — its accent, and the colour of the text inside it.
pub trait ToneExt: Component + Sized {
    /// **Override the accent — for this widget and everything inside it.**
    ///
    /// ```ignore
    /// Surface::new().accent(theme.colors.danger)   // this card is a danger card
    ///     .child(Button::new("Delete"))            // …and this follows, without being told
    /// ```
    ///
    /// **The theme is always first.** Set nothing and every widget reads the theme's accent, which
    /// is what almost everything should do. The order is: **this widget's own → the nearest
    /// ancestor that set one → the theme.** So this is an override on top of the theme, exactly
    /// like [`fill`](super::StyleExt::background), [`border`](super::StyleExt::border) and
    /// [`glow`](super::StyleExt::glow) — not a second source of colour.
    ///
    /// **What it reaches**: a control's own chrome — focus rings, hover and press fills, selected
    /// washes, scrollbar thumbs, a caret. Everything that would otherwise be the theme accent.
    ///
    /// ⚠️ **It does NOT redefine a declared meaning.** `Badge::accent`, `Alert::info`,
    /// `ToastSeverity::Info` and a destructive `Button` keep the colour their variant *names* —
    /// what the author said the thing IS, which a container does not get to restyle. A `Delete`
    /// inside a warning-toned panel is still a `Delete`.
    ///
    /// ⚠️ **Two different things are spelled `accent` on a [`Badge`](crate::widgets::Badge) and a
    /// [`BadgeButton`](crate::widgets::BadgeButton)**: `Badge::accent("3")` *constructs* the
    /// accent-coloured variant, while `.accent(colour)` overrides the hue. The first is a
    /// meaning, the second is a value; the compiler tells them apart by their arguments, and a
    /// reader has only this note.
    ///
    /// ⚠️ **A literal colour does not follow a theme reload**, the same trade every other colour
    /// override makes. Pass a colour you read from the theme at the moment you build, and rebuild
    /// (or rewrite it) when the theme changes — which is what the pane shell does every frame.
    ///
    /// **Why it exists.** A subtree that needed its own accent used to get one by being painted
    /// under a *copy of the theme* with the accent swapped. That forces a separate paint call per
    /// subtree, and is therefore what stopped any container from painting its own children — the
    /// reason a column could not own the panes inside it. The hue is inherited now, so painting is
    /// one walk for everything.
    ///
    /// Read back with [`PaintCx::accent`](crate::component::PaintCx::accent), which applies the
    /// whole order above. Applied to the subtree by `paint_child`, so nothing opts in.
    fn accent(mut self, c: Color) -> Self {
        self.base_mut().style.visual.accent = Some(c);
        self
    }


    /// **The colour of the text inside, by meaning** — CSS `color`, inherited.
    ///
    /// ```ignore
    /// Flex::column().content_tone(Tone::Danger).child(Label::new("This cannot be undone."))
    /// ```
    ///
    /// Every text or glyph inside without a colour of its own follows it; a control that colours
    /// its own content keeps it. See [`Visual::content_tone`](crate::style::Visual::content_tone).
    fn content_tone(mut self, tone: crate::Tone) -> Self {
        self.base_mut().style.visual.content_tone = Some(tone);
        self
    }
}

impl<T: Component + Sized> ToneExt for T {}
