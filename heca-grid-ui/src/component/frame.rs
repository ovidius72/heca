//! One coalesced frame requester, shared by widgets and their background jobs.

use std::{cell::RefCell, sync::{Arc, atomic::{AtomicBool, Ordering}}};

struct Request { callback: Box<dyn Fn() + Send + Sync>, pending: AtomicBool }

/// A window's frame requester. Capture on the UI thread and send to background work.
#[derive(Clone, Default)]
pub struct FrameWake(Option<Arc<Request>>);

impl FrameWake {
    /// Ask for one frame. Repeated requests coalesce until the host serves it.
    pub fn request(&self) {
        if let Some(request) = &self.0
            && !request.pending.swap(true, Ordering::AcqRel) { (request.callback)(); }
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) { (Some(a), Some(b)) => Arc::ptr_eq(a, b), (None, None) => true, _ => false }
    }
}

thread_local! { static FRAME: RefCell<FrameWake> = RefCell::new(FrameWake::default()); }

/// Install this window's redraw callback. It must be safe for a background job to call.
pub fn install_frame_request(f: impl Fn() + Send + Sync + 'static) {
    FRAME.with(|frame| *frame.borrow_mut() = FrameWake(Some(Arc::new(Request {
        callback: Box::new(f), pending: AtomicBool::new(false),
    }))));
}
/// Capture this window's requester. A no-op before installation, including headless tests.
pub fn frame_waker() -> FrameWake { FRAME.with(|frame| frame.borrow().clone()) }
/// Request a redraw through the same mechanism background jobs use.
pub fn request_frame() { frame_waker().request(); }
/// Re-arm the requester. [`super::PaintCx::new`] does this for every paint pass;
/// a host may also call it when serving a request without painting.
pub fn frame_served() {
    if let Some(request) = frame_waker().0 { request.pending.store(false, Ordering::Release); }
}
