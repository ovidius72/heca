//! [`Badge`] — a small pill label for status/metadata. A display widget (no
//! input): it self-measures to its text and paints a neon chip — a translucent
//! fill, a solid colored border, and colored text — in a [`BadgeVariant`] mapped
//! to theme tokens. The `Outline` variant drops the fill for a quieter look.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;

/// Horizontal padding inside the pill.
const PAD_H: f32 = 9.0;
/// Vertical padding inside the pill.
const PAD_V: f32 = 4.0;
/// Badge font multiplier — small chip text relative to the base font (≈11px @15).
const BADGE_FONT_SCALE: f32 = 0.73;
/// Translucent fill alpha for colored variants.
const FILL_ALPHA: u8 = 38;
/// Glow spread radius (px).
const GLOW_RADIUS: f32 = 10.0;
/// Glow intensity.
const GLOW_INTENSITY: f32 = 0.07;

/// Visual variant of a [`Badge`], mapped to theme tokens at paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    /// Accent (primary) chip.
    #[default]
    Accent,
    /// Muted/neutral chip.
    Neutral,
    /// Success (positive) chip.
    Success,
    /// Warning chip.
    Warning,
    /// Danger (negative) chip.
    Danger,
    /// Quiet outline: no fill, muted border, foreground text.
    Outline,
}

/// A small pill label. Self-sizes to its text.
pub struct Badge {
    base: Base,
    label: Signal<String>,
    variant: BadgeVariant,
}

impl Badge {
    /// A new accent badge showing `label`.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.font_scale = BADGE_FONT_SCALE; // small chip text, relative to base
        let mut badge = Self {
            base,
            label: signal(label.into()),
            variant: BadgeVariant::Accent,
        };
        badge.remeasure();
        badge
    }

    /// Convenience constructors, one per variant.
    pub fn accent(label: impl Into<String>) -> Self {
        Self::new(label)
    }
    pub fn neutral(label: impl Into<String>) -> Self {
        Self::new(label).variant(BadgeVariant::Neutral)
    }
    pub fn success(label: impl Into<String>) -> Self {
        Self::new(label).variant(BadgeVariant::Success)
    }
    pub fn warning(label: impl Into<String>) -> Self {
        Self::new(label).variant(BadgeVariant::Warning)
    }
    pub fn danger(label: impl Into<String>) -> Self {
        Self::new(label).variant(BadgeVariant::Danger)
    }
    pub fn outline(label: impl Into<String>) -> Self {
        Self::new(label).variant(BadgeVariant::Outline)
    }

    /// Set the variant.
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

}

impl Component for Badge {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Pill size tracks the resolved font.
    fn remeasure(&mut self) {
        let chars = self.label.get_untracked().chars().count() as f32;
        let fs = self.base.font;
        self.base.style.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO + 2.0 * PAD_H);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO + 2.0 * PAD_V);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (muted, foreground) = {
            let t = cx.theme();
            (t.muted, t.foreground)
        };
        let pill = self.base.bounds;
        // Pill widget: round harder than a box (×2), clamped to the capsule max —
        // radius:0 → square, a moderate radius → full pill.
        let radius = (cx.theme().radius * 2.0).min((pill.size.h / 2.0) as f32);
        let white = Color::rgb(255, 255, 255);

        let (fill, border_c, text_c, glow) = if self.variant == BadgeVariant::Outline {
            (Color::TRANSPARENT, muted, foreground, None)
        } else {
            let c = match self.variant {
                BadgeVariant::Accent => cx.theme().accent,
                BadgeVariant::Neutral => muted,
                BadgeVariant::Success => cx.theme().success,
                BadgeVariant::Warning => cx.theme().warning,
                BadgeVariant::Danger => cx.theme().danger,
                BadgeVariant::Outline => unreachable!(),
            };
            let glow = Some(Glow {
                color: c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            (c.with_alpha(FILL_ALPHA), c, c.lerp(white, 0.25), glow)
        };

        cx.rect(
            pill,
            fill,
            Some(Border {
                color: border_c,
                width: 1.0,
            }),
            radius,
            glow,
        );
        cx.text(
            pill,
            &self.label.get_untracked(),
            text_c,
            self.base.font,
            TextAlign::Center,
            true,
        );
    }
}

impl LayoutExt for Badge {}
