//! Names to library values: hint tone and placement, tooltip side, toast position and severity,
//! variant and size.

use heca_grid_ui::{ButtonVariant, ToastPosition, ToastSeverity, WidgetSize};
use heca_view::{PropValue, ViewNode, ViewSize, ViewVariant};

/// A hint placement by the name serde gives [`ViewHintPlacement`](heca_view::ViewHintPlacement) —
/// `"top_center"`, `"center"`, `"center_right"`, `"top_right"`, `"top_left"`. An unknown name keeps the
/// default. A keycap tone by the name serde gives [`ViewHintTone`](heca_view::ViewHintTone).
pub(crate) fn hint_tone(name: &str) -> Option<heca_grid_ui::Tone> {
    use heca_grid_ui::Tone as T;
    Some(match name {
        "accent" => T::Accent,
        "muted" => T::Muted,
        "warning" => T::Warning,
        "success" => T::Success,
        "danger" => T::Danger,
        _ => return None,
    })
}

pub(crate) fn hint_placement(name: &str) -> Option<heca_grid_ui::widgets::HintPlacement> {
    use heca_grid_ui::widgets::HintPlacement as P;
    Some(match name {
        "top_center" => P::TopCenter,
        "center" => P::Center,
        "center_right" => P::CenterRight,
        "top_right" => P::TopRight,
        "top_left" => P::TopLeft,
        _ => return None,
    })
}

/// A tooltip side by the name serde gives [`ViewTooltipSide`](heca_view::ViewTooltipSide) — `"top"`,
/// `"bottom"`, `"left"`, `"right"`. An unknown name keeps the default, as every other untrusted value here
/// does.
pub(crate) fn tooltip_side(name: &str) -> Option<heca_grid_ui::TooltipSide> {
    Some(match name {
        "top" => heca_grid_ui::TooltipSide::Top,
        "bottom" => heca_grid_ui::TooltipSide::Bottom,
        "left" => heca_grid_ui::TooltipSide::Left,
        "right" => heca_grid_ui::TooltipSide::Right,
        _ => return None,
    })
}

/// A `Toast`'s `"severity"` prop, by name. Unknown / absent → `Info` (the widget's own default).
/// A described position name → [`ToastPosition`]. An unknown name is **not** an error: like every
/// other named value a description carries, it degrades to the widget's default rather than
/// failing a plugin's tree.
pub(crate) fn toast_position(name: &str) -> Option<ToastPosition> {
    Some(match name {
        "top-right" => ToastPosition::TopRight,
        "top-left" => ToastPosition::TopLeft,
        "top-center" => ToastPosition::TopCenter,
        "bottom-right" => ToastPosition::BottomRight,
        "bottom-left" => ToastPosition::BottomLeft,
        "bottom-center" => ToastPosition::BottomCenter,
        _ => return None,
    })
}

pub(crate) fn severity_prop(node: &ViewNode) -> ToastSeverity {
    match node.props.get("severity").and_then(PropValue::as_text) {
        Some("success") => ToastSeverity::Success,
        Some("warning") => ToastSeverity::Warning,
        Some("danger") => ToastSeverity::Danger,
        _ => ToastSeverity::Info,
    }
}

// ── Semantic enum → grid-ui enum ──
// The model carries semantic mirrors of the grid-ui enums (so it never depends on grid-ui);
// realize is the single place that binds them across.

// There is no `ViewAlign` → `Align` mapper here on purpose. Alignment is an ordinary property:
// it travels through `prop_to_json` and lands on the widget's own `Layout` by name, on both axes
// and on children (`align_self` / `justify_self`) — see
// `grid_alignment_is_authorable_on_both_axes`. A hand-written mapper existed until the
// generic property surface landed and was dead from that day; the app's module-wide
// `allow(dead_code)` is what kept it invisible.

pub(crate) fn map_variant(v: ViewVariant) -> ButtonVariant {
    match v {
        ViewVariant::Primary => ButtonVariant::Primary,
        ViewVariant::Secondary => ButtonVariant::Secondary,
        ViewVariant::Destructive => ButtonVariant::Destructive,
        ViewVariant::Outline => ButtonVariant::Outline,
        ViewVariant::Ghost => ButtonVariant::Ghost,
        ViewVariant::Link => ButtonVariant::Link,
    }
}

pub(crate) fn map_size(s: ViewSize) -> WidgetSize {
    match s {
        ViewSize::Caption => WidgetSize::Caption,
        ViewSize::Small => WidgetSize::Small,
        ViewSize::Normal => WidgetSize::Normal,
        ViewSize::Large => WidgetSize::Large,
        ViewSize::Header => WidgetSize::Header,
    }
}
