//! A small caption aligned with a quiet horizontal rule.

use crate::{Base, Component, LayoutExt, PaintCx, Tone, ToneExt, WidgetModel, WidgetSize};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::Spacing;
use crate::widgets::{Flex, Label, Separator};

/// A theme-sized label and rule, composed from ordinary widgets.
///
/// Empty or whitespace-only text removes the whole caption from layout. Text changes update
/// the same label, and the normal layout builders control its size, gap and padding.
/// It adds no enclosing frame, body, focus target or terminal-specific behaviour.
///
/// ```
/// use heca_grid_ui::{Caption, LayoutExt, Tone, ToneExt, WidgetSize};
/// let caption = Caption::new("Build output")
///     .text("Release output")
///     .size(WidgetSize::Normal)
///     .content_tone(Tone::Accent)
///     .gap("sm");
/// ```
pub struct Caption {
    row: Flex,
    text: Signal<String>,
    was_empty: bool,
}

#[heca_grid_ui_macros::props]
impl Caption {
    /// A small, muted caption. The line follows the theme's border colour.
    pub fn new(text: impl Into<String>) -> Self {
        let text = signal(text.into());
        let mut row = Flex::row()
            .width("100%")
            .size(WidgetSize::Small)
            .content_tone(Tone::Muted)
            .align("center")
            .gap(Spacing::Xs)
            .padding_x(Spacing::Xs);
        row.base_mut().children = parts(text);
        let was_empty = text.get_untracked().trim().is_empty();
        Self { row, text, was_empty }
    }

    /// Set the caption text without replacing its label or rule.
    #[heca_grid_ui_macros::prop]
    pub fn text(self, text: impl Into<String>) -> Self {
        self.text.set(text.into());
        self
    }

    /// Follow an existing live text signal. Blank text collapses automatically.
    #[heca_grid_ui_macros::host_only("live signals cannot cross as static properties")]
    pub fn with_text_signal(mut self, text: Signal<String>) -> Self {
        self.text = text;
        self.row.base_mut().children = parts(text);
        self.row.base().mark_needs_layout();
        self
    }

    /// Update a mounted caption through the same signal its label reads.
    pub fn text_signal(&self) -> Signal<String> {
        self.text
    }
}

fn parts(text: Signal<String>) -> Vec<WidgetModel> {
    vec![
        WidgetModel::new(Label::new("").with_text_signal(text)),
        WidgetModel::new(Separator::horizontal().align_self("center").grow(1.0)),
    ]
}

impl Caption {
    fn is_empty(&self) -> bool {
        self.text.get_untracked().trim().is_empty()
    }
}

impl Component for Caption {
    fn base(&self) -> &Base { self.row.base() }
    fn base_mut(&mut self) -> &mut Base { self.row.base_mut() }

    fn set_text(&self, text: String) -> bool {
        self.text.set(text);
        true
    }

    fn text_summary(&self) -> Option<String> {
        (!self.is_empty()).then(|| self.text.get_untracked())
    }

    fn taffy_style(&self) -> taffy::Style {
        let mut style = self.row.taffy_style();
        // Content presence must not overwrite an author's own `.hidden(true)` setting.
        if self.is_empty() { style.display = taffy::Display::None; }
        style
    }

    fn is_shown(&self) -> bool { !self.is_empty() && self.row.is_shown() }

    fn paint(&self, cx: &mut PaintCx) {
        let size = self.row.base().bounds.size;
        if self.is_shown() && size.w > 0.0 && size.h > 0.0 { self.row.paint(cx); }
    }

    fn on_layout(&mut self) { self.row.on_layout(); }

    fn tick(&mut self, dt: f32) -> bool {
        let empty = self.is_empty();
        let changed = empty != self.was_empty;
        self.was_empty = empty;
        if changed { self.row.base().mark_needs_layout(); }
        self.row.tick(dt) || changed
    }
}

impl LayoutExt for Caption {}

#[cfg(test)]
mod tests;
