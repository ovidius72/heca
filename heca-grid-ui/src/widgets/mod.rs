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
mod flex;
mod input;
mod label;
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
pub use checkbox::Checkbox;
pub use flex::{container, Container, Flex};
pub use input::Input;
pub use label::Label;
pub use separator::{Orientation, Separator};
pub use spinner::Spinner;
pub use status_dot::{DotStatus, StatusDot};
pub use surface::Surface;
pub use tabs::Tabs;
pub use toggle::Toggle;
