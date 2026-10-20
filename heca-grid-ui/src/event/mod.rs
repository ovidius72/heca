//! The event vocabulary: what happened, said once, for every widget in the library.
//!
//! # Two halves, and the line between them
//!
//! **Raw input** is what a host has: a device moved, a button went down, a wheel turned. It is one
//! type — [`RawPointer`] inside [`Event::Raw`] — and a widget never sees it.
//!
//! **Resolved events** are what a widget handles: [`Event::Click`], [`Event::RightClick`],
//! [`Event::PointerEnter`], [`Event::Drop`]. The framework produces them from the raw stream
//! **once** ([`crate::pointer::route`]), hit-tests the target, and delivers them to that target and
//! then up its ancestors.
//!
//! That line is the whole design. Before it existed, every widget received every pointer event
//! wherever the pointer was, and each one re-derived the same four facts by hand: am I under the
//! cursor, was this press mine, is this the second click, did the pointer just leave me. Six
//! widgets carried their own copy of the hover test; [`Input`](crate::widgets::Input) carried its
//! own double-click clock; and a right-click could not reach a widget **at all**, because the press
//! carried no button — so the app rebuilt "what did you click" from a position and a registry of
//! strings, and the whole chain fell silent when one row forgot to declare its key.
//!
//! # Events say what happened, never what you should do about it
//!
//! [`Event::RightClick`], not `ContextMenu`. Naming an event after the response couples the
//! vocabulary to one reaction: Shift+F10 and the Menu key must be able to *emit* a right-click, and
//! a widget is free to answer one with something that is not a menu. The same rule is why there is
//! no `Event::Activate` here — that is a [`WidgetIntent`], resolved from a configurable binding.
//!
//! # What a widget author has to know
//!
//! Nothing. Put a widget in a tree and the events arrive:
//!
//! ```
//! use heca_grid_ui::prelude::*;
//!
//! let row = Row::new()
//!     .on_click(|_| println!("clicked"))
//!     .on_right_click(|e| println!("menu at {:?}", e.pos()));
//! ```
//!
//! …and inside a widget, the same events arrive as ordinary matches:
//!
//! ```ignore
//! fn on_event(&mut self, ev: &Event) -> Handled {
//!     match ev {
//!         // Already hit-tested: no `bounds.contains(pos)` here, ever.
//!         Event::Click(_) => { self.fire(); Handled::Yes }
//!         _ => Handled::No,
//!     }
//! }
//! ```


mod handled;
mod handlers;
mod intent;
mod keys;
mod kinds;
mod pointer;

pub use handled::Handled;
pub use handlers::{EventCx, HandlerOutcome, Handlers};
pub use intent::WidgetIntent;
pub(crate) use keys::remember_modifiers;
pub use keys::{GridKey, Modifiers, modifiers, typed_text};
pub use kinds::{Event, EventKind};
pub use pointer::{DragEvent, HintEvent, PointerButton, PointerEvent, RawPointer, RawPointerKind};

#[cfg(test)]
mod tests;
