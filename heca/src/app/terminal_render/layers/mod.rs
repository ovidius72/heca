//! **Retained terminal layers**: each terminal is rasterised into a texture that outlives the
//! frame, redrawn only where it changed, and blitted where the scene puts it.

mod blit;
mod damage;
mod sync;

pub(crate) use blit::{blit_retained_terminal_layer, queue_terminal_dynamic_overlays};
pub(crate) use sync::sync_retained_terminal_layers;
