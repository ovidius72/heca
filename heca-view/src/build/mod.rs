//! Typed builders for [`ViewNode`] — the authoring layer (plugin-task-ui-2, F003/P011/T006).
//!
//! A [`ViewNode`] is a uniform bag of properties, which is right for a *wire format* and wrong for
//! writing by hand: nothing stops `Label::new("x").prop("title", …)`, and nothing reports it either
//! — the property is simply ignored at realize time.
//!
//! ```
//! use heca_view::build::*;
//! use heca_view::{Intent, PropValue};
//!
//! let tree = VStack::new()
//!     .gap(8.0)
//!     .child(
//!         Panel::new().title("CONTAINERS").child(
//!             Row::new()
//!                 .padding(6.0)
//!                 .on_press(Intent::new("docker.select").arg("id", PropValue::Text("web".into())))
//!                 .child(Label::new("nginx"))
//!                 .child(Badge::new("UP")),
//!         ),
//!     )
//!     .child(Separator::new())
//!     .child(Input::new().placeholder("filter…"))
//!     .into_node();
//! ```
//!
//! **A type per kind**, so a method exists only where the widget really has that property.
//! `Label::new("x").title("y")` does not compile. The shared arrangement and appearance properties
//! live on [`Style`], written once, because they apply to every kind alike (F003/P017/T007).
//! Children live on [`Parent`], implemented only for the kinds `realize` attaches children to — a
//! `Toast` takes none, and now cannot be given any.
//!
//! **This is a convenience, not a gate.** `ViewNode` stays public and `.prop(..)` still works: a
//! plugin shipping a serialized tree never passes through here, so the builders cannot be the only
//! way in, and pretending otherwise would be a lie about where the boundary is.
//!
//! # Staying honest
//!
//! `every_widget_property_is_reachable_from_the_sdk` in `heca-view-realize` walks
//! [`WidgetKind::ALL`] against each widget's generated `PROP_NAMES` and fails when a property has
//! no builder here. It fails **closed**: a new property must reach the SDK, or someone must name an
//! exception and say why. Without it this file becomes the hand-written list that falls behind,
//! which is the defect this whole phase exists to remove.

// ── The per-kind properties ───────────────────────────────────────────────────────────────
// Each block mirrors that widget's generated `PROP_NAMES`, and the drift guard in
// `heca-view-realize` fails when one falls behind.

#[macro_use]
mod macros;
mod containers;
mod controls;
mod display;
mod events;
mod style;
#[cfg(test)]
mod tests;

pub use containers::*;
pub use controls::*;
pub use display::*;
pub use macros::Parent;
pub use style::Style;
