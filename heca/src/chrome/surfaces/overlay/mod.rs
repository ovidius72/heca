//! **Overlays** — dialogs and described panels opened over the app. Each one is a layer in the
//! [`LayerRegistry`](crate::chrome::LayerRegistry), never a second stack beside it.
//!
//! A caller (a handler, a plugin, an RPC line) hands over a [`ModalSpec`] and what to run when it
//! closes. The body is realized, a real button is added per [`ModalAction`] — each carrying the
//! overlay's id — and the whole is pushed as a layer. Because the buttons are real components they
//! can be picked by letter and reached with Tab. Choosing or dismissing comes back as
//! `SubmitOverlay` / `CloseOverlay`, resolved in one place ([`resolve`]) that takes the layer down
//! and runs the completion. RPC drives the same actions by id.
//!
//! Menus are put on screen by the context-menu surface, as layers of the same registry.
//!
//! | | owns |
//! |---|---|
//! | [`spec`](mod@spec) | what a dialog is: its id, its buttons, what it hands back |
//! | [`host`](mod@host) | the open overlays: which is on top, whether content is covered, closing one |
//! | [`modal`](mod@modal) | opening a dialog, or a panel from a description |

mod host;
mod modal;
mod spec;

#[cfg(test)]
mod tests;

pub(crate) use host::{OverlayHost, collect_form, content_covered, discard_overlays, resolve, top_modal};
pub(crate) use modal::open_modal;
pub use spec::OverlayId;
pub(crate) use spec::{ModalAction, ModalResult, ModalSpec};
