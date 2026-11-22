//! **The tooltip a widget shows under the pointer.**

use crate::component::Component;

/// **The tooltip a widget shows under the pointer** — its text, its side and how long it waits.
pub trait TooltipExt: Component + Sized {
    /// **What this widget says on hover.**
    ///
    /// ```ignore
    /// Button::new("Close").icon(Glyph::Minus).tooltip("Close the pane")
    /// ```
    ///
    /// One line, on the widget, on **every** widget — and the framework owns the rest: the reveal
    /// delay is timed off the hover clock the pointer router already keeps, and the bubble is drawn
    /// in the one place every widget passes through, beside the hint letter and the drag feedback.
    ///
    /// It used to be a wrapper you put *around* the widget
    /// ([`Tooltip`](crate::widgets::Tooltip)), which put the rule in every caller's discipline —
    /// the showcase wraps four buttons in four tooltips in a row — and made a tooltip impossible on
    /// a widget held by a typed container, because wrapping it changes what it is. The wrapper
    /// survives only for a region that is not a widget you can put a builder on, exactly as
    /// [`KeyHint`](crate::widgets::KeyHint) did when the pick declaration moved onto every widget.
    ///
    /// **What it takes** ([`IntoTip`](crate::widgets::tooltip::IntoTip)): words, which wrap and
    /// keep their line breaks (`"zsh\npane-1"` is two lines); a `Signal<String>`, which the bubble
    /// follows live without the widget being rebuilt; or a whole widget — a segmented
    /// [`Tag`](crate::widgets::Tag), a row of parts — laid out inside the bubble like anything else.
    ///
    /// ```ignore
    /// row.tooltip(Tag::new().segment(program).segment(model).segment(folder))
    /// ```
    #[heca_grid_ui_macros::prop]
    fn tooltip(mut self, tip: impl crate::widgets::tooltip::IntoTip) -> Self {
        if let Some(tip) = tip.into_tip_if_any() {
            self.base_mut().tooltip = Some(tip);
        }
        self
    }

    /// **A tooltip whose words are live** — the same as [`tooltip`](Self::tooltip) given a signal,
    /// which is the form to write; kept so code written against it keeps working.
    fn tooltip_signal(self, text: crate::reactive::Signal<String>) -> Self {
        self.tooltip(text)
    }

    /// Which side of this widget its tooltip anchors to (default `Top`). Flipped automatically
    /// when there is no room on that side, so this is a preference, not a placement.
    ///
    /// No-op when the widget has declared no tooltip — the side is part of the tip, not a style of
    /// its own.
    #[heca_grid_ui_macros::prop]
    fn tooltip_side(mut self, side: crate::widgets::tooltip::TooltipSide) -> Self {
        if let Some(tip) = self.base_mut().tooltip.as_mut() {
            tip.side = side;
        }
        self
    }

    /// Seconds the pointer must rest before this widget's tooltip appears (default `0.5`).
    /// No-op when the widget has declared no tooltip.
    #[heca_grid_ui_macros::prop]
    fn tooltip_delay(mut self, seconds: f32) -> Self {
        if let Some(tip) = self.base_mut().tooltip.as_mut() {
            tip.set_delay(seconds);
        }
        self
    }

    /// **Show this widget's tooltip after the library's shorter rest**
    /// ([`QUICK_DELAY`](crate::widgets::tooltip::QUICK_DELAY)) instead of the default — for a tip
    /// that carries what the widget had to cut short (a branch name in a narrow row), so the reader
    /// who goes to the pointer for it is not kept waiting. A name instead of a number at the call
    /// site; the one that [`tooltip_delay`](Self::tooltip_delay) takes in seconds stays for a delay
    /// that is genuinely its own. No-op when the widget has declared no tooltip.
    #[heca_grid_ui_macros::prop]
    fn tooltip_quick(mut self, quick: bool) -> Self {
        if quick && let Some(tip) = self.base_mut().tooltip.as_mut() {
            tip.set_quick();
        }
        self
    }
}

impl<T: Component + Sized> TooltipExt for T {}
