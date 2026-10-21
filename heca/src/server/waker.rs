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
}

impl std::fmt::Debug for ServerWaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ServerWaker")
    }
}
