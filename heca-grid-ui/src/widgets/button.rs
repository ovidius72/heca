//! [`Button`] — an interactive surface whose look is driven by a [`ButtonVariant`]
//! and [`ButtonSize`], mapped to theme tokens (the GridCN/shadcn model). Tron
//! treatment: sharp corners, dark interior + neon border at rest, the border
//! firms up and a glow bloom appears on hover. Glow and border are toggleable.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle};

/// Border alpha at rest (semi-opaque); firms to fully solid on hover.
const REST_BORDER_ALPHA: u8 = 150;

/// Visual variant of a [`Button`] (GridCN/shadcn set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Accent border + accent text at rest; fills with the accent on hover.
    #[default]
    Primary,
    /// Muted surface with a dim border; supporting action.
    Secondary,
    /// Danger border + danger text; fills red on hover.
    Destructive,
    /// Dim outline that brightens to accent on hover.
    Outline,
    /// No chrome until hover; low-emphasis action.
    Ghost,
    /// Text-only, accent-colored; inline link-style action.
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

/// Resolved per-frame appearance for the current variant + state.
struct Look {
    fill: Color,
    text: Color,
    border: Option<Border>,
    glow: Option<Glow>,
}

/// A clickable button. Its look comes from its [`ButtonVariant`].
pub struct Button {
    base: Base,
    label: Signal<String>,
    variant: ButtonVariant,
    size: ButtonSize,
    show_glow: bool,
    show_border: bool,
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

    /// Enable or disable the hover glow bloom (default: enabled).
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

    /// Naive monospace sizing (real shaping later). Width gets ~2 chars of slack
    /// so the centered label never overflows.
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

    /// Map variant + hover state onto concrete colors from the theme.
    fn look(&self, t: &Theme, hover: bool) -> Look {
        // Border eases from semi-opaque (rest) to fully solid (hover).
        let border_of = |c: Color, w: f32| -> Option<Border> {
            if !self.show_border {
                return None;
            }
            let color = if hover { c } else { c.with_alpha(REST_BORDER_ALPHA) };
            Some(Border { color, width: w })
        };
        let glow_of = |c: Color, radius: f32, intensity: f32| -> Option<Glow> {
            (self.show_glow && hover).then_some(Glow {
                color: c,
                radius,
                intensity,
            })
        };

        match self.variant {
            ButtonVariant::Primary => Look {
                fill: if hover { t.accent } else { t.surface },
                text: if hover { t.background } else { t.accent },
                border: border_of(t.accent, 1.5),
                glow: glow_of(t.glow, 28.0, 1.4),
            },
            ButtonVariant::Secondary => Look {
                fill: if hover {
                    t.surface.lerp(t.foreground, 0.06)
                } else {
                    t.surface
                },
                text: t.foreground,
                border: border_of(t.border, 1.5),
                glow: glow_of(t.glow, 14.0, 0.6),
            },
            ButtonVariant::Destructive => Look {
                fill: if hover { t.danger } else { t.surface },
                text: if hover { t.background } else { t.danger },
                border: border_of(t.danger, 1.5),
                glow: glow_of(t.danger, 28.0, 1.4),
            },
            ButtonVariant::Outline => Look {
                fill: if hover {
                    t.accent.with_alpha(26)
                } else {
                    Color::TRANSPARENT
                },
                text: if hover { t.foreground } else { t.muted },
                border: border_of(if hover { t.accent } else { t.muted }, 1.5),
                glow: glow_of(t.glow, 18.0, 1.0),
            },
            ButtonVariant::Ghost => Look {
                fill: if hover { t.surface } else { Color::TRANSPARENT },
                text: if hover { t.foreground } else { t.muted },
                border: None,
                glow: None,
            },
            ButtonVariant::Link => Look {
                fill: Color::TRANSPARENT,
                text: if hover { t.foreground } else { t.accent },
                border: None,
                glow: None,
            },
        }
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
        let hover = self.hovered.get_untracked();
        let look = self.look(cx.theme(), hover);
        // GridCN buttons are sharp-cornered.
        cx.rect(self.base.bounds, look.fill, look.border, 0.0, look.glow);

        // Vertically center the label. The renderer draws a ~1.2*fs line box from
        // the top, so center on that and nudge down slightly (caps have no
        // descenders, so they otherwise read a touch high).
        let b = self.base.bounds;
        let fs = self.base.style.font_size;
        let line_h = (fs * 1.2) as f64;
        let ty = b.loc.y + (b.size.h - line_h) / 2.0 + (fs as f64 * 0.10);
        let centered = Rectangle::new(Point::new(b.loc.x, ty), b.size);
        cx.text(
            centered,
            &self.label.get_untracked(),
            look.text,
            fs,
            TextAlign::Center,
        );
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
}

impl LayoutExt for Button {}
