//! **Which step was slow** — every timed step that takes longer than a threshold, printed as it ends.
//!
//! Off unless `HECA_SLOW_MS` is set to a number of milliseconds. When it is, every step wrapped in
//! [`step`] that takes at least that long prints one line on stderr, inner steps before the outer
//! ones that contain them:
//!
//! ```text
//! [slow]    12.004s    41.3 ms  server.autosave
//! [slow]    12.004s    43.0 ms  server.pump
//! ```
//!
//! [`frame_times`](crate::app::frame_times) averages 120 frames, which hides a single long one; this
//! names it, in the window and in the server alike (run `heca --standalone` to see both on one
//! terminal — a background server writes nowhere).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The environment variable that turns the log on, and its threshold in milliseconds.
const SWITCH: &str = "HECA_SLOW_MS";

/// The threshold, read once; `None` when the log is off.
fn threshold() -> Option<Duration> {
    static THRESHOLD: OnceLock<Option<Duration>> = OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        let ms: f64 = std::env::var(SWITCH).ok()?.trim().parse().ok()?;
        Some(Duration::from_secs_f64(ms.max(0.0) / 1000.0))
    })
}

/// When the log started, so every line says when it happened (seconds since the start).
fn origin() -> Instant {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    *ORIGIN.get_or_init(Instant::now)
}

/// A timed step: prints when dropped, if it took at least the threshold. Free when the log is off.
pub(crate) struct Step {
    name: &'static str,
    started: Option<Instant>,
}

/// **Time a step until the returned value is dropped.** `let _slow = slow_steps::step("server.pump");`
pub(crate) fn step(name: &'static str) -> Step {
    Step { name, started: threshold().map(|_| { origin(); Instant::now() }) }
}

impl Drop for Step {
    fn drop(&mut self) {
        let (Some(started), Some(threshold)) = (self.started, threshold()) else { return };
        let took = started.elapsed();
        if took >= threshold {
            let at = started.duration_since(origin()).as_secs_f64();
            eprintln!("[slow] {at:>9.3}s {:>7.1} ms  {}", took.as_secs_f64() * 1000.0, self.name);
        }
    }
}
