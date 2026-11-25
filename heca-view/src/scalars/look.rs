//! The value sets that say how a thing looks.

use serde::{Deserialize, Serialize};

use crate::PropValue;

/// Semantic size variant — mirrors grid-ui `WidgetSize`; `realize` maps it across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSize {
    Caption,
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
    /// A Badge's native visual vocabulary.
    pub enum ViewBadgeVariant {
        Accent => "accent", Neutral => "neutral", Success => "success",
        Warning => "warning", Danger => "danger", Outline => "outline",
    }
    /// How high a [`Surface`](crate::build::Surface) sits above the page — mirrors grid-ui
    /// `PanelElevation`; set, the surface wears the panel look (theme fill, corners, edge, shadow).
    pub enum ViewElevation {
        /// A panel over the page: a menu's, a dialog's, the palette's material. Full shadow.
        Panel => "panel",
        /// Hovering just above its target, like a tooltip. A lighter shadow.
        Hover => "hover",
    }
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

impl From<ViewVariant> for ViewBadgeVariant {
    fn from(variant: ViewVariant) -> Self {
        match variant {
            ViewVariant::Primary => Self::Accent,
            ViewVariant::Secondary => Self::Neutral,
            ViewVariant::Ghost | ViewVariant::Outline | ViewVariant::Link => Self::Outline,
            ViewVariant::Destructive => Self::Danger,
        }
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
        TopLeftCorner => "top_left_corner",
    }
}

value_set! {
    /// A keycap chip's look — mirrors grid-ui `KeycapVariant`: `Filled` (a solid glowing chip,
    /// legible over anything) or `Bordered` (a quiet outlined chip on a panel of its own).
    pub enum ViewKeycapVariant {
        Filled => "filled",
        Bordered => "bordered",
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
    /// A row's background at rest — mirrors grid-ui `RowVariant`: none, a veil, or a solid box.
    pub enum ViewRowVariant {
        Plain => "plain",
        Tinted => "tinted",
        Filled => "filled",
    }
}

value_set! {
    /// How a row marks the cursor — mirrors grid-ui `CursorLook`: the theme's glowing ring, a fill
    /// alone, or a menu entry's pill (the accent over the whole row, a short bar at its edge, the
    /// text quiet until chosen).
    pub enum ViewCursorLook {
        Ring => "ring",
        Fill => "fill",
        Pill => "pill",
    }
}

value_set! {
    /// A dock frame's header shape — mirrors grid-ui `DockFrameVariant`: a dock's (grip and fold
    /// chevron before the title) or a section's (title, then the chevron at the right; an optional
    /// `leading` column beside the whole frame).
    pub enum ViewDockFrameVariant {
        Dock => "dock",
        Section => "section",
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

value_set! {
    /// How an image fits its box; layout dimensions and aspect ratio size the box separately.
    pub enum ViewImageFit { Contain => "contain", Cover => "cover", Stretch => "stretch" }
}
