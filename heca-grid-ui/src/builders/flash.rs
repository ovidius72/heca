//! Optional semantic feedback on any component.

use crate::{Component, FlashHandle};

/// Bind visual feedback without adding painting, timers or event forwarding to a component.
pub trait FlashExt: Component + Sized {
    /// Bind an idle or shared flash handle. The theme controls its pulses and light strength.
    /// Takes a handle or an `Option` of one: `None` binds nothing, so a part that flashes only
    /// sometimes (a row with a pane to watch) is written in one line.
    fn flash(mut self, handle: impl Into<Option<FlashHandle>>) -> Self {
        self.base_mut().flash = handle.into();
        self
    }
}

impl<T: Component + Sized> FlashExt for T {}
