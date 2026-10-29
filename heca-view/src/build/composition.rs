//! Generic composition wrappers use the same open identifiers as other described widgets.

use super::{Parent, Style};
use crate::{PropValue, ViewFrameStyle, ViewNode, ViewOrientation, ViewRegionMode};

builder!(/// Lazily keep a native child until its content key changes.
    Keyed => "keyed");
impl Parent for Keyed {}
impl Keyed {
    pub fn content_key(self, key: impl Into<String>) -> Self { self.prop("content_key", key.into()) }
    pub fn texts(self, values: impl IntoIterator<Item = (String, String)>) -> Self {
        self.prop("texts", PropValue::Map(values.into_iter().map(|(key, value)| (key, value.into())).collect()))
    }
}

builder!(
    /// A generic decorated container. Its content can be any described widget or layout.
    Pane => "pane"
);
impl Parent for Pane {}

impl Pane {
    /// The kind of border, independent of content and headers.
    pub fn border_style(self, style: ViewFrameStyle) -> Self { self.prop("border_style", style) }
    /// Blur the content behind the pane by this logical radius.
    pub fn frosted(self, radius: f32) -> Self { self.prop("frosted", radius) }
    /// Override the frame color with a theme token or literal.
    pub fn border_color(self, color: &str) -> Self { self.prop("border_color", PropValue::Color(color.into())) }
}

/// Wrap content with the native focus-within outline and keyboard-containment behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusScope(pub(super) ViewNode);

impl FocusScope {
    /// Wrap one widget or layout, exactly as the native constructor does.
    pub fn new(child: impl Into<ViewNode>) -> Self {
        Self(ViewNode::new("focus_scope").child(child))
    }
    /// Override the focus outline color.
    pub fn color(self, color: &str) -> Self { self.prop("color", PropValue::Color(color.into())) }
    /// Override the outline radius; ordinary style radius uses the same property.
    pub fn radius(self, radius: f32) -> Self { self.prop("radius", radius) }
    /// Keep keyboard focus on the scope rather than controls inside it.
    pub fn keeps_keyboard(self, keeps: bool) -> Self { self.prop("keeps_keyboard", keeps) }
}
builder_impls!(FocusScope);

builder!(
    /// The generic oriented, collapsible container used for chrome regions.
    ChromeRegion => "chrome_region"
);
impl Parent for ChromeRegion {}

impl ChromeRegion {
    /// A vertical region, the default orientation.
    pub fn vertical() -> Self { Self::new() }
    /// A horizontal region.
    pub fn horizontal() -> Self { Self::new().prop("orientation", ViewOrientation::Horizontal) }
    /// Initial display mode.
    pub fn mode(self, mode: ViewRegionMode) -> Self { self.prop("mode", mode) }
    /// Expanded extent along the collapsing axis.
    pub fn expanded_size(self, extent: f32) -> Self { self.prop("expanded_size", extent) }
    /// Collapsed rail extent along the collapsing axis.
    pub fn rail_size(self, extent: f32) -> Self { self.prop("rail_size", extent) }
}
