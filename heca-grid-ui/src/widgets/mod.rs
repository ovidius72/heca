//! Built-in components. Every widget embeds [`Base`](crate::component::Base)
//! and implements [`Component`](crate::component::Component).
//!
//! Phase A ships the foundational primitives: [`Flex`] (the flexible box,
//! aliased as [`Container`]) and [`Label`]. Interactive and Tron-flavored
//! widgets (Button, Card, Hud, Gauge, Sidebar, Pane, …) arrive in Phase C.

mod button;
mod card;
mod flex;
mod label;
mod surface;

pub use button::{Button, ButtonSize, ButtonVariant};
pub use card::Card;
pub use flex::{container, Container, Flex};
pub use label::Label;
pub use surface::Surface;
