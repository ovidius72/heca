//! [`Button`] — an interactive surface whose look is driven by a [`ButtonVariant`]
//! and [`ButtonSize`] (GridCN/shadcn model), with an **animated** hover that
//! differs per variant:
//!
//! | Variant | Hover behavior |
//! |---------|----------------|
//! | Primary (default) | solid border + accent fill that **sweeps bottom→top** with a glow |
//! | Secondary | no glow; border **firms up** (rest semi-opaque → solid) |
//! | Destructive | like default but red, **fades in** (no sweep) |
//! | Outline | dim border → accent, faint fill + glow |
//! | Ghost | no border/bg at rest → **opaque bg + border fade in** |
//! | Link | text only → **underline** appears |
//!
//! Hover progress animates over time via [`Component::tick`]. Glow and border
//! are toggleable (`.glow(bool)`, `.bordered(bool)`).

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Border alpha at rest (semi-opaque); firms to fully solid on hover.
const REST_BORDER_ALPHA: f32 = 150.0;
/// Seconds for a full hover transition.
const HOVER_DURATION: f32 = 0.10;

/// Visual variant of a [`Button`] (GridCN/shadcn set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Accent border; fill sweeps in from the bottom on hover.
    #[default]
    Primary,
    /// Muted surface; border firms up on hover (no glow).
    Secondary,
    /// Danger colors; fades in on hover.
    Destructive,
    /// Dim outline that brightens to accent on hover.
    Outline,
    /// No chrome until hover (bg + border fade in).
    Ghost,
    /// Text only; underline appears on hover.
    Link,
}

/// Size of a [`Button`] — controls font size and padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ButtonSize {
    fn font_size(self) -> f32 {
        match self {
            ButtonSize::Small => 12.0,
            ButtonSize::Medium => 14.0,
            ButtonSize::Large => 16.0,
        }
    }
    fn padding(self) -> f32 {
        match self {
            ButtonSize::Small => 7.0,
            ButtonSize::Medium => 10.0,
            ButtonSize::Large => 13.0,
        }
    }
}

fn alpha(p: f32) -> u8 {
    (p.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A clickable button. Its look comes from its [`ButtonVariant`].
pub struct Button {
    base: Base,
    label: Signal<String>,
    variant: ButtonVariant,
    size: ButtonSize,
    show_glow: bool,
    show_border: bool,
    /// Animated hover amount, 0.0 (rest) → 1.0 (hovered).
    progress: f32,
    hovered: Signal<bool>,
    on_click: Option<Box<dyn Fn()>>,
}

impl Button {
    /// A primary button showing `label`.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.font_size = ButtonSize::Medium.font_size();
        base.style.padding = ButtonSize::Medium.padding();
        let mut button = Self {
            base,
            label: signal(label.into()),
            variant: ButtonVariant::Primary,
            size: ButtonSize::Medium,
            show_glow: true,
            show_border: true,
            progress: 0.0,
            hovered: signal(false),
            on_click: None,
        };
        button.remeasure();
        button
    }

    /// Convenience constructors, one per variant.
    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label)
    }
    pub fn secondary(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Secondary)
    }
    pub fn destructive(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Destructive)
    }
    pub fn outline(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Outline)
    }
    pub fn ghost(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Ghost)
    }
    pub fn link(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Link)
    }

    /// Set the variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the size (updates font size + padding).
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self.base.style.font_size = size.font_size();
        self.base.style.padding = size.padding();
        self.remeasure();
        self
    }

    /// Enable or disable the hover glow (default: enabled).
    pub fn glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Show or hide the border (default: shown for bordered variants).
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.show_border = enabled;
        self
    }

    /// Set the click callback.
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.hovered
    }

    fn remeasure(&mut self) {
        let chars = self.label.get_untracked().chars().count() as f32;
        let fs = self.base.style.font_size;
        let pad = self.base.style.padding * 2.0;
        self.base.style.width = Length::Px((chars + 2.0) * fs * MONO_ADVANCE_RATIO + pad);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO + pad);
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }

    /// Border that eases from semi-opaque (rest) to solid (hover) by `p`.
    fn animated_border(&self, c: Color, p: f32) -> Option<Border> {
        if !self.show_border {
            return None;
        }
        let a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p.clamp(0.0, 1.0);
        Some(Border {
            color: c.with_alpha(a.round() as u8),
            width: 1.5,
        })
    }

    /// Vertically centered label in `color`.
    fn paint_label(&self, cx: &mut PaintCx, color: Color) {
        let b = self.base.bounds;
        let fs = self.base.style.font_size;
        let line_h = (fs * 1.2) as f64;
        let ty = b.loc.y + (b.size.h - line_h) / 2.0 + (fs as f64 * 0.10);
        cx.text(
            Rectangle::new(Point::new(b.loc.x, ty), b.size),
            &self.label.get_untracked(),
            color,
            fs,
            TextAlign::Center,
        );
    }

    /// A fill rising from the bottom by fraction `p`, with a glow (the
    /// bottom-to-top sweep).
    fn paint_rising_fill(&self, cx: &mut PaintCx, fill: Color, glow: Color, p: f32) {
        let b = self.base.bounds;
        let fh = b.size.h * p as f64;
        let rect = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + b.size.h - fh),
            Size::new(b.size.w, fh),
        );
        let g = self.show_glow.then_some(Glow {
            color: glow,
            radius: 44.0,
            intensity: 0.28,
        });
        cx.rect(rect, fill, None, 0.0, g);
    }

    /// A thin underline beneath the centered label.
    fn paint_underline(&self, cx: &mut PaintCx, color: Color) {
        let b = self.base.bounds;
        let fs = self.base.style.font_size;
        let chars = self.label.get_untracked().chars().count() as f32;
        let tw = (chars * fs * MONO_ADVANCE_RATIO) as f64;
        let x = b.loc.x + (b.size.w - tw) / 2.0;
        let line_h = (fs * 1.2) as f64;
        let y = b.loc.y + (b.size.h - line_h) / 2.0 + (fs as f64 * 1.15);
        cx.rect(
            Rectangle::new(Point::new(x, y), Size::new(tw, 1.5)),
            color,
            None,
            0.0,
            None,
        );
    }
}

impl Component for Button {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Snapshot theme colors so we can call &mut cx methods afterwards.
        let (surface, accent, glow_c, danger, background, foreground, muted, border_c) = {
            let t = cx.theme();
            (
                t.surface,
                t.accent,
                t.glow,
                t.danger,
                t.background,
                t.foreground,
                t.muted,
                t.border,
            )
        };
        let p = self.progress.clamp(0.0, 1.0);
        let b = self.base.bounds;

        match self.variant {
            ButtonVariant::Primary => {
                cx.rect(b, surface, self.animated_border(accent, p), 0.0, None);
                if p > 0.0 {
                    self.paint_rising_fill(cx, accent, glow_c, p);
                }
                self.paint_label(cx, accent.lerp(background, p));
            }
            ButtonVariant::Destructive => {
                cx.rect(b, surface, self.animated_border(danger, p), 0.0, None);
                if p > 0.0 {
                    let g = self.show_glow.then_some(Glow {
                        color: danger,
                        radius: 40.0,
                        intensity: 0.3 * p,
                    });
                    cx.rect(b, danger.with_alpha(alpha(p)), None, 0.0, g);
                }
                self.paint_label(cx, danger.lerp(background, p));
            }
            ButtonVariant::Secondary => {
                cx.rect(b, surface, self.animated_border(border_c, p), 0.0, None);
                self.paint_label(cx, foreground);
            }
            ButtonVariant::Outline => {
                let fill = accent.with_alpha(alpha(p * 0.1));
                let g = (self.show_glow && p > 0.0).then_some(Glow {
                    color: glow_c,
                    radius: 30.0,
                    intensity: 0.25 * p,
                });
                cx.rect(
                    b,
                    fill,
                    self.animated_border(muted.lerp(accent, p), p),
                    0.0,
                    g,
                );
                self.paint_label(cx, muted.lerp(foreground, p));
            }
            ButtonVariant::Ghost => {
                let border = if self.show_border && p > 0.0 {
                    Some(Border {
                        color: border_c.with_alpha(alpha(p)),
                        width: 1.5,
                    })
                } else {
                    None
                };
                cx.rect(b, surface.with_alpha(alpha(p)), border, 0.0, None);
                self.paint_label(cx, muted.lerp(foreground, p));
            }
            ButtonVariant::Link => {
                let color = accent.lerp(foreground, p);
                self.paint_label(cx, color);
                if p > 0.0 {
                    self.paint_underline(cx, color.with_alpha(alpha(p)));
                }
            }
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.contains(*pos) => {
                if let Some(f) = &self.on_click {
                    f();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let target = if self.hovered.get_untracked() {
            1.0
        } else {
            0.0
        };
        if (self.progress - target).abs() < 1e-3 {
            self.progress = target;
            return false;
        }
        let step = dt / HOVER_DURATION;
        if self.progress < target {
            self.progress = (self.progress + step).min(target);
        } else {
            self.progress = (self.progress - step).max(target);
        }
        true
    }
}

impl LayoutExt for Button {}
