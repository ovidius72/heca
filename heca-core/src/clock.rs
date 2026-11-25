//! **heca's clock** — the one place anything that moves with time asks what time it is.
//!
//! Every widget that moves with time — a flash pulsing, a caret blinking, a double click being told
//! from two clicks — and every layout animation — a column sliding, a pane growing — reads
//! [`now`](crate::clock::now) and [`since`](crate::clock::since) here instead of the system's clock
//! directly. It lives in `heca-core` so the layout and the widgets (`heca_grid_ui::clock` is this
//! same clock) read one clock. In the app it *is* the system's clock. A test can [`hold`](crate::clock::hold) it and [`advance`](crate::clock::advance) it by hand, so it says "100 ms later"
//! instead of sleeping 100 ms: the test then sees exactly the frames it asked for on any machine,
//! however slow or busy, and runs in no time.
//!
//! ```ignore
//! heca_core::clock::hold();                                   // time stops for this thread
//! flash.repeat_for(Tone::Warning, Duration::from_secs(1));
//! for _ in 0..10 {
//!     heca_core::clock::advance(Duration::from_millis(100));  // and moves only when told
//!     // … tick, lay out and paint, then look at what was drawn
//! }
//! heca_core::clock::release();                                // back to the system's clock
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

/// **How long since `then`** — [`now`] minus `then`, never negative. What `Instant::elapsed` is,
/// read from this clock.
pub fn since(then: Instant) -> Duration {
    now().saturating_duration_since(then)
}

/// **How long until the next beat of `period`** — the next moment that is a whole number of
/// `period`s after one instant shared by the whole program.
///
/// Anything that moves in steps (a flash at 15 steps a second) waits for this instead of counting
/// from when it started. Two of them started at different moments then step at the **same**
/// instants, and one frame draws both. Counting each from its own start put their steps between
/// each other's: four cards flashing at 15 steps a second asked for 60 frames a second, and the
/// window never rested.
pub fn until_beat(period: Duration) -> Duration {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let period = period.as_secs_f64();
    if period <= 0.0 {
        return Duration::ZERO;
    }
    let epoch = *EPOCH.get_or_init(|| now() - Duration::from_secs(1));
    let into = now().saturating_duration_since(epoch).as_secs_f64() % period;
    Duration::from_secs_f64(period - into)
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

    /// **Two things stepping at the same rate step at the same instants**, whenever each started.
    #[test]
    fn the_beat_is_shared_whenever_each_one_started() {
        hold();
        let period = Duration::from_millis(66);
        let first = now() + until_beat(period);
        advance(Duration::from_millis(23));
        let second = now() + until_beat(period);
        assert_eq!(first, second, "a later starter waits for the same beat");
        advance(until_beat(period));
        assert_eq!(
            until_beat(period),
            period,
            "on a beat, the next is a whole period away"
        );
        release();
    }

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
