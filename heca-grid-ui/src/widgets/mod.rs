//! Built-in components. Every widget embeds [`Base`](crate::component::Base)
//! and implements [`Component`](crate::component::Component).
//!
//! Phase A ships the foundational primitives: [`Flex`] (the flexible box,
//! aliased as [`Container`]) and [`Label`]. Interactive and Tron-flavored
//! widgets (Button, Card, Hud, Gauge, Sidebar, Pane, …) arrive in Phase C.

mod alert;
mod badge;
mod button;
mod card;
mod checkbox;
mod chrome_region;
mod command_palette;
mod context_menu;
mod dock_frame;
mod flex;
mod gauge;
mod grid;
mod icon;
mod icon_button;
mod input;
mod item;
mod item_group;
mod key_hint;
mod label;
mod marker_group;
mod modal;
mod pane;
mod progress;
mod rail_cell;
mod row;
mod select;
mod separator;
mod scroll_region;
mod spinner;
mod status_dot;
mod surface;
mod tabs;
mod tag;
mod toast;
mod toast_stack;
mod toggle;
mod tooltip;
mod visibility;

pub use alert::{Alert, AlertVariant};
pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonVariant};
pub use card::Card;
pub use checkbox::{Checkbox, LabelSide};
pub use chrome_region::{ChromeRegion, RegionMode};
pub use command_palette::{Command, CommandPalette};
pub use context_menu::{ContextMenu, MenuEntry};
pub use dock_frame::DockFrame;
pub use flex::{Container, Flex, container};
pub use gauge::Gauge;
pub use grid::Grid;
pub use icon::{Glyph, Icon};
pub use icon_button::IconButton;
pub use input::Input;
pub use item::{ActiveMarker, Item};
pub use item_group::ItemGroup;
pub use key_hint::{HintPlacement, KeyHint};
pub use label::Label;
pub use marker_group::MarkerGroup;
pub use modal::Modal;
pub use pane::{Pane, PaneFrame};
pub use progress::ProgressBar;
pub use rail_cell::RailCell;
pub use row::Row;
pub use select::Select;
pub use separator::{Orientation, Separator};
pub use spinner::Spinner;
pub use status_dot::{DotStatus, StatusDot};
pub use surface::Surface;
pub use scroll_region::ScrollRegion;
pub use tabs::Tabs;
pub use tag::Tag;
pub use toast::{Toast, ToastSeverity};
pub use toast_stack::{ToastCorner, ToastSpec, ToastStack};
pub use toggle::Toggle;
pub use tooltip::{Tooltip, TooltipSide};
pub use visibility::Visibility;
