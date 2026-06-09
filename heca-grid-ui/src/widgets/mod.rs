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
mod dock_frame;
mod flex;
mod gauge;
mod grid;
mod input;
mod item;
mod item_group;
mod label;
mod pane;
mod progress;
mod select;
mod separator;
mod spinner;
mod status_dot;
mod surface;
mod tabs;
mod toggle;

pub use alert::{Alert, AlertVariant};
pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonSize, ButtonVariant};
pub use card::Card;
pub use checkbox::{Checkbox, LabelSide};
pub use dock_frame::DockFrame;
pub use flex::{Container, Flex, container};
pub use gauge::Gauge;
pub use grid::Grid;
pub use input::Input;
pub use item::{ActiveMarker, Item};
pub use item_group::ItemGroup;
pub use label::Label;
pub use pane::Pane;
pub use progress::ProgressBar;
pub use select::Select;
pub use separator::{Orientation, Separator};
pub use spinner::Spinner;
pub use status_dot::{DotStatus, StatusDot};
pub use surface::Surface;
pub use tabs::Tabs;
pub use toggle::Toggle;
