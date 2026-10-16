//! The registry's merge order, the entries the built-in menus carry, and how a declarer's identity
//! becomes a target.

use crate::providers::ChromeCtx;

mod entries;
mod registry;
mod target;

/// An observe-only facade over an empty chrome store — what a provider's `build` receives.
fn test_ctx() -> ChromeCtx<'static> {
    // Leaked deliberately: a test-only store that must outlive the borrow in `App`.
    let store: &'static crate::chrome::SharedChromeState = Box::leak(Box::new(
        crate::chrome::SharedChromeState::new(300.0, 300.0),
    ));
    ChromeCtx::new(store)
}
