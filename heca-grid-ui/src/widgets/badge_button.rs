//! [`BadgeButton`] — a clickable badge/chip.
//!
//! A small pill-shaped button that reuses the visual language of [`Badge`](crate::widgets::Badge) but is
//! interactive like a button: hover tint, press flash, keyboard focus ring, and
//! `Enter`/`Space` activation.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::effects::Flash;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::Length;
use crate::widgets::badge::BadgeVariant;

/// Horizontal padding inside the pill.
const PAD_H: f32 = 9.0;
/// Vertical padding inside the pill.
const PAD_V: f32 = 4.0;
/// Badge font multiplier — small chip text relative to the base font.
const BADGE_FONT_SCALE: f32 = 0.73;
/// Extra fill alpha while hovered/focused.
const HOVER_FILL_EXTRA: u8 = 22;
/// Glow spread radius (px).
const GLOW_RADIUS: f32 = 10.0;
/// Resting glow intensity.
const GLOW_INTENSITY: f32 = 0.07;
/// Hover glow intensity.
const HOVER_GLOW_INTENSITY: f32 = 0.11;

/// A clickable badge/chip. Self-sizes to its text.
pub struct BadgeButton {
    base: Base,
    label: Signal<String>,
    seen_label: String,
    variant: BadgeVariant,
    flash: Flash,
}

#[heca_grid_ui_macros::props]
impl BadgeButton {
    /// A new accent badge button showing `label`.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.one_click_target = true; // and one click target (Base::one_click_target)
        base.style.visual.font_scale = BADGE_FONT_SCALE;
        let mut button = Self {
            base,
            label: signal(label.into()),
            seen_label: String::new(),
            variant: BadgeVariant::Accent,
            flash: Flash::new(),
        };
        button.seen_label = button.label.get_untracked();
        button.remeasure();
        button
    }

    /// Convenience constructors, one per badge variant.
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

    /// Set the visual variant.
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }


    /// The reactive label signal, so hosts can update the text live.
    pub fn label_signal(&self) -> Signal<String> {
        self.label
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.base.pointer.hovered
    }
}

impl Component for BadgeButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The badge button renders its own text (it composes no `Label`), so it supplies its
    /// [accessible name](Component::text_summary) itself.
    fn text_summary(&self) -> Option<String> {
        Some(self.label.get_untracked())
    }

    fn remeasure(&mut self) {
        let label = self.label.get_untracked();
        self.seen_label = label.clone();
        let chars = label.chars().count() as f32;
        let fs = self.base.font;
        let s = self.base.size_scale();
        self.base.style.layout.width =
            Length::Px(chars * fs * MONO_ADVANCE_RATIO + 2.0 * PAD_H * s);
        self.base.style.layout.height = Length::Px(fs * MONO_LINE_RATIO + 2.0 * PAD_V * s);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let hovered = self.base.hovered() || self.base.focused_by_keyboard();
        let pill = self.base.bounds;
        let radius = (cx.theme().colors.border_radius * 2.0).min((pill.size.h / 2.0) as f32);
        let (muted, foreground, danger) = {
            let t = cx.theme();
            (t.colors.muted, t.colors.foreground, t.colors.danger)
        };
        let pulse = self.flash.amount();

        let (fill, border_c, text_c, glow) = if self.variant == BadgeVariant::Outline {
            let border_c = if hovered {
                cx.accent()
                    .with_alpha(cx.theme().colors.interaction.outline_hover)
            } else {
                muted.with_alpha(cx.theme().colors.interaction.outline_rest)
            };
            let fill = if hovered {
                cx.accent()
                    .with_alpha(cx.theme().colors.interaction.badge_fill / 2)
            } else {
                Color::TRANSPARENT
            };
            let glow = hovered.then_some(Glow {
                color: cx.accent(),
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            (fill, border_c, foreground, glow)
        } else {
            let c = match self.variant {
                BadgeVariant::Accent => cx.theme().colors.accent,
                BadgeVariant::Neutral => muted,
                BadgeVariant::Success => cx.theme().colors.success,
                BadgeVariant::Warning => cx.theme().colors.warning,
                BadgeVariant::Danger => danger,
                BadgeVariant::Outline => unreachable!(),
            };
            let fill_alpha = cx
                .theme()
                .colors
                .interaction
                .badge_fill
                .saturating_add(if hovered { HOVER_FILL_EXTRA } else { 0 });
            let glow = Some(Glow {
                color: c,
                radius: GLOW_RADIUS,
                intensity: if hovered {
                    HOVER_GLOW_INTENSITY
                } else {
                    GLOW_INTENSITY
                },
            });
            (
                c.with_alpha(fill_alpha),
                c,
                c.lerp(foreground, if hovered { 0.15 } else { 0.25 }),
                glow,
            )
        };

        let white = Color::rgb(255, 255, 255);
        let border = cx.border(border_c.lerp(white, pulse * 0.15));
        cx.rect(pill, fill.lerp(white, pulse * 0.08), border, radius, glow);
        cx.text(
            pill,
            &self.label.get_untracked(),
            text_c.lerp(white, pulse * 0.10),
            self.base.font,
            TextAlign::Center,
            TextStyle::BOLD,
        );
        if self.focusable() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(pill, ring, radius);
        }
    }

    /// The press shows on the control itself; what it does belongs to its click handlers.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if matches!(ev, Event::Click(_)) && self.base.activatable && !self.base.disabled.get_untracked() {
            self.flash.trigger();
        }
        Handled::No
    }

    fn is_control(&self) -> bool {
        true
    }


    fn tick(&mut self, dt: f32) -> bool {
        let next = self.label.get_untracked();
        if next != self.seen_label {
            self.seen_label = next;
            self.remeasure();
            self.base.mark_needs_paint();
        }
        self.flash.tick(dt)
    }
}

impl LayoutExt for BadgeButton {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::EventExt;
    use crate::event::PointerButton;
    use crate::reactive::SignalUpdate;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn button_click_fires_callback() {
        let hit = Rc::new(Cell::new(0));
        let out = hit.clone();
        let mut b = BadgeButton::new("LIVE").on_click(move |_| out.set(out.get() + 1));
        b.base_mut().bounds = heca_core::layout::Rectangle::new(
            heca_core::layout::Point::new(0.0, 0.0),
            heca_core::layout::Size::new(80.0, 24.0),
        );
        // A click is a press **and** a release on the same widget — pressing and dragging off
        // cancels, exactly as it does everywhere else on the machine.
        let pos = heca_core::layout::Point::new(10.0, 10.0);
        assert_eq!(
            crate::component::dispatch(&mut b, &Event::pointer_pressed(pos, PointerButton::Left)),
            Handled::Yes
        );
        assert_eq!(hit.get(), 0, "the press alone has not clicked anything yet");
        // The click bubbles on to whatever holds the button, as a mouse click does; what matters
        // is that the button's own handler heard it.
        let _ = crate::component::dispatch(&mut b, &Event::pointer_released(pos, PointerButton::Left));
        assert_eq!(hit.get(), 1);
    }

    #[test]
    fn label_signal_remeasures_on_tick() {
        let mut b = BadgeButton::new("A");
        let old = b.base().style.layout.width;
        b.label_signal().set("HELLO".to_string());
        let _ = b.tick(0.016);
        assert_ne!(b.base().style.layout.width, old);
    }
}
