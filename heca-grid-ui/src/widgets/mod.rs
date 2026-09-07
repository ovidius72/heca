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
mod card;
mod panel;
mod choice;
mod checkbox;
mod chrome_region;
mod card_grid;
mod command_palette;
mod context_menu;
mod dialog;
mod dock_frame;
mod flex;
mod focus_scope;
mod gauge;
mod grid;
mod icon;
mod nf_icon;
mod icon_button;
mod input;
mod item;
mod item_group;
pub(crate) mod key_hint;
mod key_hint_group;
mod label;
mod marker_group;
pub mod overlay;
mod pane;
mod progress;
mod rail_cell;
mod row;
mod select;
mod separator;
mod scroll_bar;
mod scroll_region;
mod spinner;
mod status_dot;
mod surface;
mod tabs;
mod tag;
mod toast;
mod toggle;
mod button_group;
pub mod tooltip;
mod visibility;

pub use alert::{Alert, AlertVariant};
pub use badge::{Badge, BadgeVariant};
pub use badge_button::BadgeButton;
pub use button::{Button, ButtonVariant};
pub use choice::{choice_at, Choice};
pub use card::Card;
pub use panel::Panel;
pub use checkbox::{Checkbox, LabelSide};
pub use chrome_region::{ChromeRegion, RegionMode};
pub use card_grid::{CardGrid, GridCell};
pub use command_palette::{Command, CommandPalette};
pub use context_menu::{ContextMenu, Menu, MenuAnchor, MenuEntry, MenuItem};
pub use dialog::{Dialog, DIALOG_BTN_GAP, DIALOG_GAP, DIALOG_PAD};
pub use overlay::{
    paint_panel_chrome, place_anchored, place_anchored_on, place_at_point, place_beside,
    AnchorSide, BesideSide, Overlay, OverlayPosition, PanelChrome, PanelElevation,
    DEFAULT_ANCHOR_GAP,
};
pub use dock_frame::DockFrame;
pub use flex::{Container, Flex, container};
pub use focus_scope::FocusScope;
pub use gauge::Gauge;
pub use grid::Grid;
pub use icon::{Glyph, Icon};
pub use nf_icon::{NfGlyph, NfIcon};
pub use icon_button::IconButton;
pub use input::Input;
pub use item::{ActiveMarker, Item};
pub use item_group::ItemGroup;
pub use key_hint_group::{KeyHintGroup, DEFAULT_LETTERS};
pub use key_hint::{
    HintPlacement, HintStyle, KeyCap, KeyHint, KeycapVariant, keycap_size, keycap_size_nf, paint_keycap,
    paint_keycap_nf,
};
pub use label::{Ellipsis, Label};
pub use marker_group::MarkerGroup;
pub use pane::{Pane, PaneFrame};
pub use progress::ProgressBar;
pub use rail_cell::RailCell;
pub use row::Row;
pub use select::Select;
pub use separator::{Orientation, Separator};
pub use spinner::Spinner;
pub use status_dot::{DotStatus, StatusDot};
pub use surface::Surface;
pub use scroll_bar::ScrollBar;
pub use scroll_region::{RevealAlign, ScrollAxes, ScrollInfo, ScrollRegion};
pub use tabs::Tabs;
pub use tag::Tag;
pub use toast::{Toast, ToastAction, ToastPosition, ToastSeverity, ToastSpec, ToastStack};
pub use toggle::Toggle;
pub use button_group::{ButtonGroup, Display};
pub use tooltip::{Tip, Tooltip, TooltipSide};
pub use visibility::Visibility;
