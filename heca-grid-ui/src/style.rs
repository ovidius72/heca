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
    pub font_size: f32,
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
            font_size: 14.0,
        }
    }
}

impl Style {
    /// Map the layout fields onto a `taffy::Style` for the layout engine.
    pub fn to_taffy(&self) -> taffy::Style {
        use taffy::prelude::*;
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
            ..Default::default()
        }
    }
}
