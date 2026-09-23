//! Drag and drop.
//!
//! **A row is dragged by the name it already declares.** There is no registry to enrol with, no id
//! to hand out and no closed list of surfaces that may participate: a widget says `.draggable_as()`
//! what it is and `.accepts()` what it takes, in words it chooses, and the framework runs the
//! gesture — threshold, `DragStart`, `Drag`, `DragEnter`/`DragOver`, `Drop`, `DragEnd` — over the
//! same tree that lays out and paints. A plugin's own row is draggable on the same terms as a
//! built-in one, which is the whole point.
//!
//! ## Design rules
//!
//! - **No GPU code.** This crate emits state; the renderer rasterizes it.
//! - **No app-specific types.** What a drop *means* is the host's; [`Dropped`] carries the two
//!   names and lets the host say.
//! - **Identity, never position.** [`resolve`] returns the node it found, path included, so a
//!   caller never has to search for it again by name — two seatings of one container give their
//!   rows the same name.

mod resolve;
pub(crate) mod sink;

pub use resolve::{
    DropAction, DropHit, DropSide, resolve_at, resolve_at_filtered, resolve_at_for, set_swap_rule,
    source_at,
};
pub use sink::{Dropped, has_drop_sink, install_drop_sink};
