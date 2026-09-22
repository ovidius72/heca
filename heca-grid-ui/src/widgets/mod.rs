//! Built-in components. Every widget embeds [`Base`](crate::component::Base)
//! and implements [`Component`](crate::component::Component).
//!
//! Phase A ships the foundational primitives: [`Flex`] (the flexible box,
//! aliased as [`Container`]) and [`Label`]. Interactive and Tron-flavored
//! widgets (Button, Card, Hud, Gauge, Sidebar, Pane, …) arrive in Phase C.

mod alert;
mod badge;
mod badge_button;
mod button;
mod button_group;
mod card;
mod card_grid;
mod checkbox;
mod choice;
mod chrome_region;
mod command_palette;
mod context_menu;
mod dialog;
mod dock_frame;
mod flex;
mod focus_scope;
mod gauge;
mod grid;
mod icon;
mod icon_button;
mod input;
mod item;
mod item_group;
pub(crate) mod key_hint;
mod key_hint_group;
mod label;
mod marker_group;
mod nf_icon;
pub mod overlay;
mod pane;
mod panel;
mod progress;
mod rail_cell;
mod row;
mod scroll_bar;
mod scroll_region;
mod select;
mod separator;
mod spinner;
mod status_dot;
mod surface;
mod tabs;
mod tag;
mod toast;
mod toggle;
pub mod tooltip;
mod visibility;

pub use alert::{Alert, AlertVariant};
pub use badge::{Badge, BadgeVariant};
pub use badge_button::BadgeButton;
pub use button::{Button, ButtonVariant};
pub use button_group::{ButtonGroup, Display};
pub use card::Card;
pub use card_grid::{CardGrid, GridCell};
pub use checkbox::{Checkbox, LabelSide};
pub use choice::{Choice, choice_at};
pub use chrome_region::{ChromeRegion, RegionMode};
pub use command_palette::{Command, CommandPalette};
pub use context_menu::{ContextMenu, Menu, MenuAnchor, MenuEntry, MenuItem};
pub use dialog::{DIALOG_BTN_GAP, DIALOG_GAP, DIALOG_PAD, Dialog};
pub use dock_frame::DockFrame;
pub use flex::{Container, Flex, container};
pub use focus_scope::FocusScope;
pub use gauge::Gauge;
pub use grid::Grid;
pub use icon::{Glyph, Icon};
pub use icon_button::IconButton;
pub use input::Input;
pub use item::{ActiveMarker, Item};
pub use item_group::ItemGroup;
pub use key_hint::{
    HintPlacement, HintStyle, HintTone, KeyCap, KeyHint, KeycapVariant, keycap_size,
    keycap_size_nf, paint_keycap, paint_keycap_nf,
};
pub use key_hint_group::{DEFAULT_LETTERS, KeyHintGroup};
pub use label::{Ellipsis, Label};
pub use marker_group::MarkerGroup;
pub use nf_icon::{NfGlyph, NfIcon};
pub use overlay::{
    AnchorSide, BesideSide, DEFAULT_ANCHOR_GAP, Overlay, OverlayPosition, PanelChrome,
    PanelElevation, SurfaceHandle, paint_panel_chrome, place_anchored, place_anchored_on,
    place_at_point, place_beside,
};
pub use pane::Pane;
pub use panel::Panel;
pub use progress::ProgressBar;
pub use rail_cell::RailCell;
pub use row::Row;
pub use scroll_bar::ScrollBar;
pub use scroll_region::{RevealAlign, ScrollAxes, ScrollInfo, ScrollRegion};
pub use select::Select;
pub use separator::{Orientation, Separator};
pub use spinner::Spinner;
pub use status_dot::{DotStatus, StatusDot};
pub use surface::Surface;
pub use tabs::Tabs;
pub use tag::Tag;
pub use toast::{Toast, ToastAction, ToastPosition, ToastSeverity, ToastSpec, ToastStack};
pub use toggle::Toggle;
pub use tooltip::{Tip, Tooltip, TooltipSide};
pub use visibility::Visibility;
