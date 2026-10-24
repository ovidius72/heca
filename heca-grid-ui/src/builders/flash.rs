//! Optional semantic feedback on any component.

use crate::{Component, FlashHandle};

/// Bind visual feedback without adding painting, timers or event forwarding to a component.
pub trait FlashExt: Component + Sized {
    /// Bind an idle or shared flash handle. The theme controls its pulses and light strength.
    fn flash(mut self, handle: FlashHandle) -> Self {
        self.base_mut().flash = Some(handle);
        self
    }
}

impl<T: Component + Sized> FlashExt for T {}
