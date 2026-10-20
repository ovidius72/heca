//! **How a container flows its children** — direction, justification, alignment and the semantic
//! size variant.

use super::*;


/// Main-axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Row,
    Column,
}

/// Main-axis distribution of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Overall **size variant** of a widget. Scales the widget's font **and** its
/// intrinsic padding / fixed dimensions together, so the whole control grows or
/// shrinks proportionally. The font part is applied centrally during layout (see
/// [`LayoutEngine`](crate::layout::LayoutEngine)); each widget scales its own
/// padding by [`pad_scale`](WidgetSize::pad_scale) in `remeasure`.
///
/// `Large` matches the historical (un-sized) look; the default is `Normal`, a more
/// compact baseline. `Header` is the one step *above* `Large` — an emphasized control
/// (`1.25×` the base font) with a tight cluster padding, for icon buttons that sit in a
/// pane/info-bar header and must read a touch larger than the body text.
/// Set per widget via [`LayoutExt::size`](crate::builders::LayoutExt::size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetSize {
    /// **Supporting text** — a line of metadata under a name (`0.72×`): a step below `Small`, which
    /// is for compact controls. A folder, a branch and its counts read as detail, not as a second
    /// title.
    Caption,
    /// Compact controls (`0.8×`).
    Small,
    /// The default — tighter than the raw base font (`0.9×`).
    #[default]
    Normal,
    /// Roomy controls at the full base font + padding (`1.0×`).
    Large,
    /// Emphasized header controls (`1.25×` font) in a tight cluster — for header /
    /// info-bar action buttons that should out-size the body text.
    Header,
}

impl WidgetSize {
    /// Multiplier for the inherited **font** size.
    pub fn font_scale(self) -> f32 {
        match self {
            WidgetSize::Caption => 0.72,
            WidgetSize::Small => 0.8,
            WidgetSize::Normal => 0.9,
            WidgetSize::Large => 1.0,
            WidgetSize::Header => 1.25,
        }
    }

    /// Multiplier for a widget's intrinsic **padding / fixed dimensions** (track,
    /// box, chevron…). Tighter than the font at `Small` so compact controls aren't
    /// dominated by their padding — the height of a `Small` button is mostly
    /// padding, so this is what actually makes it sidebar-compact. `Normal`/`Large`
    /// match the font scale (no change to them). `Header` deliberately keeps a *snug*
    /// padding (below `Small`) so an emphasized header icon stays large while the
    /// button cluster reads as one tight group, not a row of chunky boxes.
    pub fn pad_scale(self) -> f32 {
        match self {
            WidgetSize::Caption | WidgetSize::Small => 0.5,
            WidgetSize::Normal => 0.9,
            WidgetSize::Large => 1.0,
            WidgetSize::Header => 0.4,
        }
    }
}


impl Justify {
    pub(super) fn to_taffy(self) -> taffy::JustifyContent {
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
    pub(super) fn to_taffy(self) -> taffy::AlignItems {
        use taffy::AlignItems as A;
        match self {
            Align::Start => A::Start,
            Align::Center => A::Center,
            Align::End => A::End,
            Align::Stretch => A::Stretch,
        }
    }
}
