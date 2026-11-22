//! **Builder-method traits**: each says one kind of thing about a widget, and the type system keeps
//! the ones that only fit some widgets on those widgets.
//!
//! - [`LayoutExt`] — arrangement; for any container, including layout-only `Flex`.
//! - [`StyleExt`] — visual decoration; for **surface** components only (`Surface`, `Card`,
//!   `Button`). Deliberately *not* implemented for `Flex`, so a pure layout container cannot be
//!   turned into a styled surface.
//! - [`Parent`] — holds children; a `Label` has no `.child()`.
//! - [`PlaceExt`] — where an item goes in the grid that holds it; boxed subtrees too.
//! - What **every** widget gets, one trait per subject, each implemented for every [`Component`]
//!   so nothing opts in: [`EventExt`] (what happens to it), [`DragExt`] (what it does in a drag),
//!   [`MenuExt`] (its right-click menu), [`TooltipExt`], [`HintExt`] (its pick letter),
//!   [`IdentityExt`] (who it is), [`PresenceExt`] (whether and how it is there) and [`ToneExt`]
//!   (its hue). The prelude brings them all.
//!
//! Default methods mutate `base_mut().style`, so a widget opts into a set of builders just by
//! writing `impl LayoutExt for MyWidget {}` (zero boilerplate).
//!
//! [`Component`]: crate::component::Component

mod bound;
mod children;
mod drag;
mod events;
mod flash;
mod focus;
mod hint;
mod identity;
mod layout;
mod menu;
mod place;
mod presence;
mod style;
#[cfg(test)]
mod tests;
mod text_value;
mod tone;
mod tooltip;

pub(crate) use bound::{Bound, apply as apply_bound};
pub use bound::{BoundChildren, MapChildren};
pub use children::{IntoChildren, IntoComponent, IntoContextMenu, Parent};
pub use drag::DragExt;
pub use events::EventExt;
pub use flash::FlashExt;
pub use focus::FocusExt;
pub use hint::HintExt;
pub use identity::{IdentityExt, IntoKey};
pub use layout::LayoutExt;
pub use menu::MenuExt;
pub use place::PlaceExt;
pub use presence::{BoolValue, IntoBoolValue, PresenceExt};
pub use style::StyleExt;
pub use text_value::{IntoTextValue, TextValue};
pub use tone::ToneExt;
pub use tooltip::TooltipExt;
