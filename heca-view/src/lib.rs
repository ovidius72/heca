//! `ViewNode` — the declarative, serializable widget-tree model (plugin-task-ui-1).
//!
//! This is the **app-wide** UI description: any UI — native chrome, overlays, and plugin
//! panels — can be expressed as a tree of `ViewNode`s and turned into retained grid-ui
//! `Component`s by the mapper `realize()` in `heca-view-realize` (plugin-task-ui-3).
//! It is **generic** (its [`WidgetKind`] covers the whole grid-ui vocabulary) and fully
//! **serializable** (serde), so the exact same model authored in Rust is what a WASM plugin
//! ships over the boundary.
//!
//! This crate carries **no dependency but serde** — no widget library, no renderer, no taffy.
//! That is the point of it living apart from the app (F003/P017/T009): a plugin can depend on
//! the vocabulary without compiling the thing that draws it.
//!
//! Behaviour is expressed **only** through [`Intent`]s (an action id + args), never Rust
//! closures — so the model stays serializable and uniform for native and plugin UI alike.
//! A node that carries an `on_press`/`on_change` intent is *actionable*; `realize` makes every
//! actionable node pickable by `prefix+/` for free, and `on_hint` says what a pick does when that
//! differs from a click.
//!
//! Adding a widget = one [`WidgetKind`] variant + one arm in `realize`. Nothing here holds
//! layout or paint logic — this is pure description.
//!
//! Consumed by `realize` (plugin-task-ui-3), the Modal `body` (ui-4) and plugins. Everything
//! here is `pub`: it is the published vocabulary, so there is nothing to mark dead.

pub mod build;
mod dropdown;
mod kind;
mod node;
mod scalars;
#[cfg(test)]
mod tests;
mod value;

pub use dropdown::*;
pub use kind::*;
pub use node::*;
pub use scalars::*;
pub use value::*;
