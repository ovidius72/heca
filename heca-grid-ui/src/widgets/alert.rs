//! [`Alert`] — a callout surface with a colored left bar, a title, and optional
//! body text, themed by an [`AlertVariant`]. A display widget (no input). Title
//! and body are laid out manually (no children) as single lines.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::font::MONO_LINE_RATIO;
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Border, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Inner padding.
const PAD: f64 = 14.0;
/// Width of the colored left accent bar.
const BAR_W: f64 = 3.0;
/// Body text multiplier relative to the title (which uses the resolved font).
const BODY_SCALE: f32 = 0.9;
/// Gap between title and body.
const GAP: f64 = 6.0;
/// Default alert width.
const DEFAULT_WIDTH: f32 = 360.0;
/// Translucent fill alpha for the variant tint.
const FILL_ALPHA: u8 = 22;

/// Visual variant of an [`Alert`], mapped to theme tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertVariant {
    /// Informational (accent).
    #[default]
    Info,
    /// Success (positive).
    Success,
    /// Warning.
    Warning,
    /// Danger (error).
    Danger,
}

/// A callout surface with a title and optional body.
pub struct Alert {
    base: Base,
    title: Signal<String>,
    body: Option<String>,
    variant: AlertVariant,
}

impl Alert {
    /// A new info alert with `title`. Add body text with [`body`](Alert::body).
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(DEFAULT_WIDTH);
        let mut alert = Self {
            base,
            title: signal(title.into()),
            body: None,
            variant: AlertVariant::Info,
        };
        alert.remeasure();
        alert
    }

    /// Convenience constructors, one per variant.
    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).variant(AlertVariant::Success)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).variant(AlertVariant::Warning)
    }
    pub fn danger(title: impl Into<String>) -> Self {
        Self::new(title).variant(AlertVariant::Danger)
    }

    /// Set the variant.
    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the body text (shown on a second line).
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self.remeasure();
        self
    }

}

impl Component for Alert {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Height = padding + title line + optional body line, from the resolved font.
    fn remeasure(&mut self) {
        let title_h = self.base.font as f64 * MONO_LINE_RATIO as f64;
        let body_h = if self.body.is_some() {
            GAP + self.base.font as f64 * BODY_SCALE as f64 * MONO_LINE_RATIO as f64
        } else {
            0.0
        };
        self.base.style.height = Length::Px((2.0 * PAD + title_h + body_h) as f32);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (foreground, muted, radius) = {
            let t = cx.theme();
            (t.foreground, t.muted, t.control_radius())
        };
        let title_fs = self.base.font;
        let body_fs = self.base.font * BODY_SCALE;
        let color = match self.variant {
            AlertVariant::Info => cx.theme().accent,
            AlertVariant::Success => cx.theme().success,
            AlertVariant::Warning => cx.theme().warning,
            AlertVariant::Danger => cx.theme().danger,
        };
        let b = self.base.bounds;

        // Surface: dark fill tinted by the variant, bordered.
        cx.rect(
            b,
            color.with_alpha(FILL_ALPHA),
            Some(Border { color, width: 1.0 }),
            radius,
            None,
        );
        // Colored left accent bar.
        cx.rect(
            Rectangle::new(b.loc, Size::new(BAR_W, b.size.h)),
            color,
            None,
            0.0,
            None,
        );

        // Title (variant-colored, bold) then optional body (muted).
        let text_x = b.loc.x + BAR_W + PAD;
        let text_w = (b.size.w - BAR_W - 2.0 * PAD).max(0.0);
        let title_h = title_fs as f64 * MONO_LINE_RATIO as f64;
        cx.text(
            Rectangle::new(
                Point::new(text_x, b.loc.y + PAD),
                Size::new(text_w, title_h),
            ),
            &self.title.get_untracked(),
            color,
            title_fs,
            TextAlign::Start,
            true,
        );
        if let Some(body) = &self.body {
            let body_h = body_fs as f64 * MONO_LINE_RATIO as f64;
            cx.text(
                Rectangle::new(
                    Point::new(text_x, b.loc.y + PAD + title_h + GAP),
                    Size::new(text_w, body_h),
                ),
                body,
                foreground.lerp(muted, 0.2),
                body_fs,
                TextAlign::Start,
                false,
            );
        }
    }
}

impl LayoutExt for Alert {}
