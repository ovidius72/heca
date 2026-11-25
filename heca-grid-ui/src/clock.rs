//! **The library's clock** — `heca_core::clock`, the one clock everything that moves with time
//! reads: widgets here, layout animations in `heca-core`. A test that holds it holds both.
//!
//! ```ignore
//! heca_grid_ui::clock::hold();                                   // time stops for this thread
//! heca_grid_ui::clock::advance(Duration::from_millis(100));      // and moves only when told
//! heca_grid_ui::clock::release();                                // back to the system's clock
//! ```

pub use heca_core::clock::{advance, hold, now, release, since, until_beat};
