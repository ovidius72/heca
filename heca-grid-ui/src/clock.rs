//! **The library's clock** — the one place a widget asks what time it is.
//!
//! Every widget that moves with time — a flash pulsing, a caret blinking, a double click being told
//! from two clicks — reads [`now`](crate::clock::now) here instead of the system's clock directly. In the app it *is*
//! the system's clock. A test can [`hold`](crate::clock::hold) it and [`advance`](crate::clock::advance) it by hand, so it says "100 ms later"
//! instead of sleeping 100 ms: the test then sees exactly the frames it asked for on any machine,
//! however slow or busy, and runs in no time.
//!
//! ```ignore
//! heca_grid_ui::clock::hold();                                   // time stops for this thread
//! flash.repeat_for(Tone::Warning, Duration::from_secs(1));
//! for _ in 0..10 {
//!     heca_grid_ui::clock::advance(Duration::from_millis(100));  // and moves only when told
//!     // … tick, lay out and paint, then look at what was drawn
//! }
//! heca_grid_ui::clock::release();                                // back to the system's clock
//! ```
//!
//! The held time is per thread: a test holding it does not move time for a test beside it.

use std::cell::Cell;
use std::time::{Duration, Instant};

thread_local! {
    /// The held time on this thread, or `None` for the system's clock.
    static HELD: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// **What time it is** — for every widget that moves with time. The system's clock, unless this
/// thread [`hold`]s it.
pub fn now() -> Instant {
    HELD.with(Cell::get).unwrap_or_else(Instant::now)
}

/// **Stop time on this thread** at the present moment: [`now`] returns it until [`advance`] moves it
/// or [`release`] gives the system's clock back. For tests.
pub fn hold() {
    HELD.with(|held| held.set(Some(Instant::now())));
}

/// **Move this thread's held time forward** by `by` (holding it first if it was not held). For
/// tests.
pub fn advance(by: Duration) {
    HELD.with(|held| held.set(Some(held.get().unwrap_or_else(Instant::now) + by)));
}

/// **Give this thread the system's clock back.**
pub fn release() {
    HELD.with(|held| held.set(None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_clock_moves_only_when_advanced_and_is_released_back() {
        hold();
        let start = now();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(now(), start, "held: real time passing does not move it");
        advance(Duration::from_millis(100));
        assert_eq!(now() - start, Duration::from_millis(100));
        release();
        assert!(
            now() >= Instant::now() - Duration::from_secs(1),
            "released: the system's clock again"
        );
    }
}
