//! [`Button`] — an interactive surface whose look is driven by a [`ButtonVariant`]
//! and the shared [`WidgetSize`](crate::style::WidgetSize) (GridCN/shadcn model),
//! with an **animated** hover that differs per variant:
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
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Border alpha at rest (semi-opaque); firms to fully solid on hover.
const REST_BORDER_ALPHA: f32 = 150.0;
/// Seconds for a full hover transition.
const HOVER_DURATION: f32 = 0.10;
/// Hover glow spread radius (px) — how far the halo reaches (bigger = wider).
const GLOW_RADIUS: f32 = 30.0;
/// Hover glow peak intensity — how bright (smaller = thinner/fainter).
const GLOW_INTENSITY: f32 = 0.12;

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

/// Reference padding (logical px) at [`WidgetSize::Large`]; smaller sizes scale it
/// down by [`WidgetSize::pad_scale`] (tighter than the font at `Small`). The font
/// scales centrally, so the box stays balanced at every size.
const BASE_PAD: f32 = 10.0;

fn alpha(p: f32) -> u8 {
    (p.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A clickable button. Its look comes from its [`ButtonVariant`].
pub struct Button {
    base: Base,
    label: Signal<String>,
    variant: ButtonVariant,
    show_glow: bool,
    show_border: bool,
    /// Animated hover amount, 0.0 (rest) → 1.0 (hovered).
    progress: f32,
    /// Press flash effect (brightens on press, fades out).
    flash: Flash,
    hovered: Signal<bool>,
    on_click: Option<Box<dyn Fn()>>,
}

impl Button {
    /// A primary button showing `label`.
    pub fn new(label: impl Into<String>) -> Self {
        let base = Base::new();
        // Padding + font derive from the size variant (default `Normal`) in
        // `remeasure`; the size is set via `LayoutExt::size`.
        let mut button = Self {
            base,
            label: signal(label.into()),
            variant: ButtonVariant::Primary,
            show_glow: true,
            show_border: true,
            progress: 0.0,
            flash: Flash::new(),
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

    /// Set an explicit font size — overrides the inherited theme font + size scale.
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.font_size = fs;
        self.base.font = fs;
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

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }

    /// Border that eases from semi-opaque (rest) to solid (hover) by `p`. The
    /// stroke `width` is the theme's `border_width` (so `border_width == 0` means
    /// no border, like every other surface).
    fn animated_border(&self, c: Color, p: f32, width: f32) -> Option<Border> {
        if !self.show_border || width <= 0.0 {
            return None;
        }
        let a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p.clamp(0.0, 1.0);
        Some(Border {
            color: c.with_alpha(a.round() as u8),
            width,
        })
    }

    /// Bold label, centered in the button box by the renderer (real metrics).
    fn paint_label(&self, cx: &mut PaintCx, color: Color) {
        cx.text(
            self.base.bounds,
            &self.label.get_untracked(),
            color,
            self.base.font,
            TextAlign::Center,
            true,
        );
    }

    /// A fill rising from the bottom by fraction `p`, with a glow (the
    /// bottom-to-top sweep).
    fn paint_rising_fill(&self, cx: &mut PaintCx, fill: Color, glow: Color, p: f32, radius: f32) {
        let b = self.base.bounds;
        let fh = b.size.h * p as f64;
        let rect = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + b.size.h - fh),
            Size::new(b.size.w, fh),
        );
        let g = self.show_glow.then_some(Glow {
            color: glow,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        cx.rect(rect, fill, None, radius, g);
    }

    /// A thin underline beneath the centered label.
    fn paint_underline(&self, cx: &mut PaintCx, color: Color) {
        let b = self.base.bounds;
        let fs = self.base.font;
        let chars = self.label.get_untracked().chars().count() as f32;
        let tw = (chars * fs * MONO_ADVANCE_RATIO) as f64;
        let x = b.loc.x + (b.size.w - tw) / 2.0;
        let y = b.loc.y + b.size.h / 2.0 + (fs as f64 * 0.5);
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

    fn focusable(&self) -> bool {
        !self.base.disabled.get_untracked()
    }

    /// Width + height track the resolved font (which already includes the size
    /// scale) plus size-scaled padding, so the whole button grows/shrinks together.
    fn remeasure(&mut self) {
        let chars = self.label.get_untracked().chars().count() as f32;
        let fs = self.base.font;
        let pad = BASE_PAD * self.base.size_scale();
        self.base.style.padding = pad;
        self.base.style.width = Length::Px((chars + 2.0) * fs * MONO_ADVANCE_RATIO + pad * 2.0);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO + pad * 2.0);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Snapshot theme colors so we can call &mut cx methods afterwards.
        let (surface, accent, glow_c, danger, background, foreground, muted, border_c, border_width, radius) = {
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
                t.border_width,
                t.control_radius(),
            )
        };
        let p = self.progress.clamp(0.0, 1.0);
        let b = self.base.bounds;

        match self.variant {
            ButtonVariant::Primary => {
                cx.rect(b, surface, self.animated_border(accent, p, border_width), radius, None);
                if p > 0.0 {
                    self.paint_rising_fill(cx, accent, glow_c, p, radius);
                }
                self.paint_label(cx, accent.lerp(background, p));
            }
            ButtonVariant::Destructive => {
                cx.rect(b, surface, self.animated_border(danger, p, border_width), radius, None);
                if p > 0.0 {
                    let g = self.show_glow.then_some(Glow {
                        color: danger,
                        radius: GLOW_RADIUS,
                        intensity: GLOW_INTENSITY * p,
                    });
                    cx.rect(b, danger.with_alpha(alpha(p)), None, radius, g);
                }
                self.paint_label(cx, danger.lerp(background, p));
            }
            ButtonVariant::Secondary => {
                // Border becomes more vivid on hover (brighter + solid).
                let bc = border_c.lerp(foreground, 0.4 * p);
                cx.rect(b, surface, self.animated_border(bc, p, border_width), radius, None);
                self.paint_label(cx, foreground);
            }
            ButtonVariant::Outline => {
                // Hover: vivid accent border, text → primary (accent), lightest glow.
                let fill = accent.with_alpha(alpha(p * 0.1));
                let g = (self.show_glow && p > 0.0).then_some(Glow {
                    color: glow_c,
                    radius: GLOW_RADIUS * 0.8,
                    intensity: GLOW_INTENSITY * 0.6 * p,
                });
                cx.rect(
                    b,
                    fill,
                    self.animated_border(muted.lerp(accent, p), p, border_width),
                    radius,
                    g,
                );
                self.paint_label(cx, muted.lerp(accent, p));
            }
            ButtonVariant::Ghost => {
                let border = if self.show_border && p > 0.0 && border_width > 0.0 {
                    Some(Border {
                        color: border_c.with_alpha(alpha(p)),
                        width: border_width,
                    })
                } else {
                    None
                };
                cx.rect(b, surface.with_alpha(alpha(p)), border, radius, None);
                self.paint_label(cx, muted.lerp(foreground, p));
            }
            ButtonVariant::Link => {
                // Color stays constant on hover; press flashes the TEXT (no bg).
                self.paint_label(cx, accent.lerp(foreground, self.flash.amount() * 0.7));
                if p > 0.0 {
                    self.paint_underline(cx, accent.with_alpha(alpha(p)));
                }
            }
        }

        // Press flash — brightening overlay (Link flashes its text above instead).
        // Filled variants need a stronger flash to read over their bright fill.
        if !matches!(self.variant, ButtonVariant::Link) {
            let strength = match self.variant {
                ButtonVariant::Primary | ButtonVariant::Destructive => 0.95,
                _ => 0.6,
            };
            cx.flash(b, self.flash.amount() * strength, radius);
        }

        // Dim the whole button when disabled.
        if self.base.disabled.get_untracked() {
            cx.dim(b, radius);
        }

        // Focus ring — only for keyboard focus (focus-visible) and when enabled.
        if self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.contains(*pos) => {
                self.flash.trigger();
                if let Some(f) = &self.on_click {
                    f();
                }
                Handled::Yes
            }
            // Keyboard activation: Space/Enter on the focused button == a click.
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.flash.trigger();
                if let Some(f) = &self.on_click {
                    f();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;

        // Hover progress eases toward the hovered target.
        let target = if self.hovered.get_untracked() {
            1.0
        } else {
            0.0
        };
        if (self.progress - target).abs() >= 1e-3 {
            let step = dt / HOVER_DURATION;
            self.progress = if self.progress < target {
                (self.progress + step).min(target)
            } else {
                (self.progress - step).max(target)
            };
            animating = true;
        } else {
            self.progress = target;
        }

        // Press flash fades out.
        animating |= self.flash.tick(dt);

        // Damage just our own rect each frame so the hover/press animation doesn't
        // force a whole-scene redraw (host safety net). Mirrors `Spinner`.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Button {}
