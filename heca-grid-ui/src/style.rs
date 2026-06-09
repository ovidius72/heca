//! Component style: layout (mapped to taffy) + Tron visual tokens.
//!
//! The layout enums here ([`Direction`], [`Justify`], [`Align`], [`Length`])
//! are our own, mapped to `taffy` internally — so `taffy` never leaks into the
//! public API and could be swapped without breaking widget code.

use crate::color::Color;
use crate::scene::{Border, Glow};

/// Main-axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
}

/// Main-axis distribution of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Cross-axis alignment of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// A size along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Sized by content / flex rules.
    #[default]
    Auto,
    /// Fixed logical pixels.
    Px(f32),
    /// Fraction of the parent (`0.0..=1.0`).
    Pct(f32),
}

impl Length {
    fn to_taffy(self) -> taffy::Dimension {
        use taffy::prelude::*;
        match self {
            Length::Auto => auto(),
            Length::Px(v) => length(v),
            Length::Pct(p) => percent(p),
        }
    }
}

impl Justify {
    fn to_taffy(self) -> taffy::JustifyContent {
        use taffy::JustifyContent as J;
        match self {
            Justify::Start => J::Start,
            Justify::Center => J::Center,
            Justify::End => J::End,
            Justify::SpaceBetween => J::SpaceBetween,
            Justify::SpaceAround => J::SpaceAround,
            Justify::SpaceEvenly => J::SpaceEvenly,
        }
    }
}

impl Align {
    fn to_taffy(self) -> taffy::AlignItems {
        use taffy::AlignItems as A;
        match self {
            Align::Start => A::Start,
            Align::Center => A::Center,
            Align::End => A::End,
            Align::Stretch => A::Stretch,
        }
    }
}

/// Layout + visual style for a component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    // ── Layout ──
    pub direction: Direction,
    pub justify: Justify,
    pub align: Align,
    pub gap: f32,
    pub padding: f32,
    pub width: Length,
    pub height: Length,
    pub flex_grow: f32,

    // ── Visual ──
    pub fill: Option<Color>,
    pub border: Option<Border>,
    pub glow: Option<Glow>,
    pub accent: Color,
    pub fg: Color,
    pub radius: f32,
    /// Explicit font size in logical px. `0.0` = inherit the theme base font.
    pub font_size: f32,
    /// Semantic multiplier applied to the inherited base font (header ≈ 2.0,
    /// caption ≈ 0.8, body = 1.0). Ignored when `font_size` is set explicitly.
    pub font_scale: f32,
    /// When true the node is removed from layout entirely (`display: none`) — it
    /// takes no space and paints nothing. Used by collapsible containers
    /// (e.g. [`ItemGroup`](crate::widgets::ItemGroup)) to fold rows away.
    pub hidden: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            direction: Direction::Row,
            justify: Justify::Start,
            align: Align::Stretch,
            gap: 0.0,
            padding: 0.0,
            width: Length::Auto,
            height: Length::Auto,
            flex_grow: 0.0,
            fill: None,
            border: None,
            glow: None,
            accent: Color::rgb(137, 180, 250),
            fg: Color::rgb(205, 214, 244),
            radius: 0.0,
            // 0.0 = inherit the theme's `font_size`; a widget's `.font_size(x)`
            // (x > 0) overrides it. Resolved centrally during layout.
            font_size: 0.0,
            font_scale: 1.0,
            hidden: false,
        }
    }
}

impl Style {
    /// Map the layout fields onto a `taffy::Style` for the layout engine.
    pub fn to_taffy(&self) -> taffy::Style {
        use taffy::prelude::*;
        if self.hidden {
            return taffy::Style { display: Display::None, ..Default::default() };
        }
        taffy::Style {
            display: Display::Flex,
            flex_direction: match self.direction {
                Direction::Row => FlexDirection::Row,
                Direction::Column => FlexDirection::Column,
            },
            justify_content: Some(self.justify.to_taffy()),
            align_items: Some(self.align.to_taffy()),
            gap: Size {
                width: length(self.gap),
                height: length(self.gap),
            },
            padding: Rect {
                left: length(self.padding),
                right: length(self.padding),
                top: length(self.padding),
                bottom: length(self.padding),
            },
            size: Size {
                width: self.width.to_taffy(),
                height: self.height.to_taffy(),
            },
            flex_grow: self.flex_grow,
            // Widgets use explicit Px sizes; never let a flex container squish them.
            flex_shrink: 0.0,
            ..Default::default()
        }
    }
}
