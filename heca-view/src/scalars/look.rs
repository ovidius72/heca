//! The value sets that say how a thing looks.

use serde::{Deserialize, Serialize};

use crate::PropValue;

/// Semantic size variant — mirrors grid-ui `WidgetSize`; `realize` maps it across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSize {
    Small,
    Normal,
    Large,
    Header,
}

/// Semantic visual variant — mirrors grid-ui `ButtonVariant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewVariant {
    Primary,
    Secondary,
    Destructive,
    Outline,
    Ghost,
    Link,
}

value_set! {
    /// How a surface arrives and leaves — mirrors grid-ui `NamedAnimation` (`Overlay`).
    ///
    /// The **built-ins**, which is all a description can name: a live animation is a Rust type, and a
    /// plugin that writes its own reaches it from native code rather than from data.
    pub enum ViewAnimation {
        /// A cut: there, then gone.
        None => "none",
        /// A dissolve, both ways.
        Fade => "fade",
        /// Growing in from smaller, shrinking away again.
        Zoom => "zoom",
        /// The exposé's gesture: it shrinks away, and the dissolve rides the shrink.
        ZoomFade => "zoom_fade",
    }
}

value_set! {
    /// How serious a message is — mirrors both grid-ui `ToastSeverity` **and** `AlertVariant`, which
    /// carry the same four values. One mirror, because two would be the same list written twice.
    pub enum ViewSeverity {
        Info => "info",
        Success => "success",
        Warning => "warning",
        Danger => "danger",
    }
}

value_set! {
    /// Which side of a widget its tooltip anchors to — mirrors grid-ui `TooltipSide`.
    ///
    /// A **preference, not a placement**: the framework flips it to the opposite side when there is no
    /// room, so an author says where they would like the bubble and never where it must go.
    pub enum ViewTooltipSide {
        Top => "top",
        Bottom => "bottom",
        Left => "left",
        Right => "right",
    }
}

value_set! {
    /// Where a node's hint letter sits over it — mirrors grid-ui `HintPlacement`.
    ///
    /// The picker draws the cap itself; this only says where. `TopLeft` is what the picker drew for
    /// every large target before letters became the widget's own to place, and it is kept as a variant
    /// because that rule was right for a card or a content pane: out of the way of what the target
    /// shows, and never on its border.
    pub enum ViewHintPlacement {
        TopCenter => "top_center",
        Center => "center",
        CenterRight => "center_right",
        TopRight => "top_right",
        TopLeft => "top_left",
    }
}

value_set! {
    /// **What a keycap means** — mirrors grid-ui `HintTone`. The theme picks the colour, so a letter
    /// follows a theme reload and a plugin never writes a hex.
    ///
    /// `Accent` is a place to go; `Muted` a structural control — fold this, close that — which is not
    /// somewhere to navigate; the rest are further classes so two kinds of target never read alike.
    pub enum ViewHintTone {
        Accent => "accent",
        Muted => "muted",
        Warning => "warning",
        Success => "success",
        Danger => "danger",
    }
}

value_set! {
    /// How a selected row shows it — mirrors grid-ui `ActiveMarker` (`Row`, `Item`).
    pub enum ViewMarker {
        None => "none",
        Bar => "bar",
        Check => "check",
    }
}

impl From<ViewVariant> for PropValue {
    fn from(v: ViewVariant) -> Self {
        PropValue::Variant(v)
    }
}

impl From<ViewSize> for PropValue {
    fn from(v: ViewSize) -> Self {
        PropValue::Size(v)
    }
}
