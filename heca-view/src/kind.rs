//! Open, serializable widget identities. Availability is decided by the host registry.

use std::borrow::Cow;
use std::fmt;
use serde::{Deserialize, Serialize};

/// A widget identifier on the wire. Any identifier is representable; the host reports whether
/// its implementation is registered. Existing built-in names retain their serialized spelling.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WidgetId(Cow<'static, str>);

impl WidgetId {
    /// A custom or received widget identifier. Construction does not require registration.
    pub fn new(name: impl Into<String>) -> Self {
        Self(Cow::Owned(name.into()))
    }

    /// A statically declared identifier, without allocating its name.
    pub const fn from_static(name: &'static str) -> Self {
        Self(Cow::Borrowed(name))
    }

    /// The exact name used in serialized descriptions and registry lookup.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for WidgetId {
    fn from(name: &str) -> Self { Self::new(name) }
}

impl From<String> for WidgetId {
    fn from(name: String) -> Self { Self::new(name) }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}

// Compatibility shortcuts for the existing typed SDK, not a closed vocabulary or runtime
// inventory. New widgets may use their own WidgetId without adding a shortcut here.
macro_rules! builtin_shortcuts {
    ($($name:ident => $wire:literal,)*) => {
        // Keep existing Rust source spellings during the model migration.
        #[allow(non_upper_case_globals)]
        impl WidgetId {
            $(pub const $name: Self = Self::from_static($wire);)*
        }
        impl fmt::Debug for WidgetId {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.as_str() {
                    $($wire => f.write_str(stringify!($name)),)*
                    _ => f.debug_tuple("WidgetId").field(&self.as_str()).finish(),
                }
            }
        }
        // Frozen legacy aliases are only a wire/source compatibility fixture. Realizer coverage
        // enumerates its actual registrations, including extensions, rather than this fixture.
        #[cfg(test)]
        pub(crate) fn legacy_wire_ids() -> Vec<WidgetId> {
            vec![$(WidgetId::$name,)*]
        }
    };
}

builtin_shortcuts! {
    VStack => "v_stack",
    HStack => "h_stack",
    Row => "row",
    Grid => "grid",
    TreeLayout => "tree_layout",
    MessageList => "message_list",
    Resizable => "resizable",
    CardGrid => "card_grid",
    Card => "card",
    Scroll => "scroll",
    Panel => "panel",
    Surface => "surface",
    ItemGroup => "item_group",
    DockFrame => "dock_frame",
    Overlay => "overlay",
    MarkerGroup => "marker_group",
    Tabs => "tabs",
    Choice => "choice",
    KeyHintGroup => "key_hint_group",
    Label => "label",
    Button => "button",
    IconButton => "icon_button",
    Badge => "badge",
    BadgeButton => "badge_button",
    Tag => "tag",
    Icon => "icon",
    Image => "image",
    Input => "input",
    Select => "select",
    Toggle => "toggle",
    Checkbox => "checkbox",
    StatusDot => "status_dot",
    Gauge => "gauge",
    ScrollBar => "scroll_bar",
    Alert => "alert",
    Toast => "toast",
    RailCell => "rail_cell",
    Item => "item",
    Separator => "separator",
    Line => "line",
    Splitter => "splitter",
    LandingSlot => "landing_slot",
    Spinner => "spinner",
    Progress => "progress",
    NfIcon => "nf_icon",
    ButtonGroup => "button_group",
    Tile => "tile",
}
