//! **How the server wakes the loop that owns it.**
//!
//! The server has no loop of its own in standalone — it rides the window's winit event loop, and a
//! terminal whose output changed wakes that loop by posting
//! [`AppEvent::BackendWake`](crate::app::events::AppEvent::BackendWake). A headless
//! server (no window) owns its own loop and wakes it through a channel instead. Both are the same
//! thing from the server's side: *call this and the loop will come back round*. [`ServerWaker`] is
//! that call, so nothing the server holds — the terminals, the notification timers — names winit or
//! a channel directly.
//!
//! It is `Send + Sync` because a terminal's PTY reader thread is what calls it when output arrives.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A way to wake the loop that owns the server. Standalone wraps the winit
/// [`EventLoopProxy`](winit::event_loop::EventLoopProxy); a headless server wraps its own channel
/// sender. Cheap to clone (an `Arc`).
#[derive(Clone)]
pub(crate) struct ServerWaker(Arc<dyn Fn() + Send + Sync>);

impl ServerWaker {
    /// A waker that runs `wake` to bring the owning loop back round.
    pub(crate) fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }

    /// The callback anything calls to bring the owning loop back round — a terminal's
    /// `wake_on_output`, and (headless) whatever else needs the loop to run. The one wake primitive.
    pub(crate) fn on_output(&self) -> Arc<dyn Fn() + Send + Sync> {
        self.0.clone()
    }

    /// Wake the owner after an incoming message or a closed connection.
    pub(crate) fn wake(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for ServerWaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerWaker")
    }
}

/// **One wake in flight at a time.** A terminal that writes without pause calls its waker for every
/// chunk of output — thousands of times a second — and each call used to post its own event, so the
/// loop ran once per chunk: reading every terminal, mirroring the chrome and asking for a frame each
/// time, far more often than the screen can show.
///
/// [`ServerWaker::latched`] wraps a waker in this latch: a call wakes the loop only when no wake is
/// already waiting, and the loop [`reset`](Self::reset)s it as it starts the turn the wake asked
/// for — before it reads anything, so output that arrives during the turn wakes it again and
/// nothing is missed.
#[derive(Clone, Debug, Default)]
pub(crate) struct WakeLatch(Arc<AtomicBool>);

impl WakeLatch {
    /// The loop is starting the turn a wake asked for: the next output wakes it again.
    pub(crate) fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl ServerWaker {
    /// A waker that runs `wake` only when no earlier wake is still waiting, and the latch its loop
    /// resets at the start of each turn. See [`WakeLatch`].
    pub(crate) fn latched(wake: impl Fn() + Send + Sync + 'static) -> (Self, WakeLatch) {
        let latch = WakeLatch::default();
        let pending = latch.0.clone();
        let waker = Self::new(move || {
            if !pending.swap(true, Ordering::AcqRel) {
                wake();
            }
        });
        (waker, latch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn many_wakes_before_the_loop_runs_are_one_and_a_reset_opens_the_next() {
        let posted = Arc::new(AtomicUsize::new(0));
        let count = posted.clone();
        let (waker, latch) = ServerWaker::latched(move || { count.fetch_add(1, Ordering::SeqCst); });
        for _ in 0..1000 {
            waker.wake();
        }
        assert_eq!(posted.load(Ordering::SeqCst), 1, "a thousand chunks of output, one wake");
        latch.reset();
        waker.wake();
        assert_eq!(posted.load(Ordering::SeqCst), 2, "after the turn starts, the next output wakes again");
    }
}
