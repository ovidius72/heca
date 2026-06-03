//! [`Button`] — an interactive surface whose look is driven by a [`ButtonVariant`]
//! and [`ButtonSize`], mapped to theme tokens (the GridCN/shadcn model: 6
//! variants × sizes). Tracks hover via a [`Signal`] and fires a click callback.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle};

/// Visual variant of a [`Button`] (GridCN/shadcn set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Filled with the accent color — the primary call to action.
    #[default]
    Primary,
    /// Filled with a muted surface; supporting action.
    Secondary,
    /// Filled with the danger color; irreversible / destructive action.
    Destructive,
    /// Transparent with an accent border; outlined action.
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
        let white = Color::rgb(255, 255, 255);
        match self.variant {
            ButtonVariant::Primary => Look {
                fill: if hover { t.accent.lerp(white, 0.14) } else { t.accent },
                text: t.background,
                border: None,
                glow: Some(Glow {
                    color: t.glow,
                    radius: if hover { 28.0 } else { 14.0 },
                    intensity: if hover { 1.4 } else { 0.8 },
                }),
            },
            ButtonVariant::Secondary => Look {
                fill: if hover {
                    t.surface.lerp(t.foreground, 0.08)
                } else {
                    t.surface
                },
                text: t.foreground,
                border: Some(Border {
                    color: t.border,
                    width: 1.0,
                }),
                glow: None,
            },
            ButtonVariant::Destructive => Look {
                fill: if hover { t.danger.lerp(white, 0.12) } else { t.danger },
                text: t.background,
                border: None,
                glow: Some(Glow {
                    color: t.danger,
                    radius: if hover { 28.0 } else { 14.0 },
                    intensity: if hover { 1.4 } else { 0.8 },
                }),
            },
            ButtonVariant::Outline => Look {
                fill: if hover {
                    t.accent.with_alpha(30)
                } else {
                    Color::TRANSPARENT
                },
                text: if hover { t.foreground } else { t.accent },
                border: Some(Border {
                    color: t.accent,
                    width: 1.2,
                }),
                glow: Some(Glow {
                    color: t.glow,
                    radius: if hover { 20.0 } else { 10.0 },
                    intensity: if hover { 1.1 } else { 0.4 },
                }),
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
        let radius = cx.theme().radius.max(4.0);
        cx.rect(self.base.bounds, look.fill, look.border, radius, look.glow);

        // Vertically + horizontally centered label.
        let b = self.base.bounds;
        let fs = self.base.style.font_size;
        let line_h = (fs * MONO_LINE_RATIO) as f64;
        let centered = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + (b.size.h - line_h) / 2.0),
            b.size,
        );
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
