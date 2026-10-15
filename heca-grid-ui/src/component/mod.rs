//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

mod base;
mod behavior;
mod dispatch;
mod paint_cx;
mod paint_walk;
#[cfg(test)]
mod tests;
mod tree;

pub use base::Base;
pub use behavior::Component;
pub use dispatch::*;
pub use paint_cx::PaintCx;
pub use paint_walk::*;
pub use tree::*;

use crate::color::Color;
use crate::drag::DropSide;
use crate::hint::DeclaredAction;
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{
    Border, BracketCmd, DrawCommand, FontRole, Glow, HostCmd, HostDraw, RectCmd, ScanlineCmd,
    Scene, Shadow, TextAlign, TextCmd, TextStyle,
};
use crate::style::Style;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

/// The event vocabulary lives in [`crate::event`]; it is re-exported here because every widget
/// already imports its events from `component`, and moving a type is not a reason to touch fifty
/// files.
pub use crate::event::{
    DragEvent, Event, EventCx, EventKind, GridKey, Handled, Handlers, Modifiers, PointerButton,
    PointerEvent, RawPointer, RawPointerKind, WidgetIntent,
};

thread_local! {
    /// Host-installed hook the widget tree calls to schedule the next frame. The
    /// event loop wires it to its redraw request; widgets reach it via
    /// [`request_frame`]. Thread-local because the UI runs single-threaded.
    static FRAME_REQUEST: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
    /// Whether a frame has already been asked for and not yet served. See [`request_frame`].
    static FRAME_PENDING: Cell<bool> = const { Cell::new(false) };
}

/// Install the callback the widget tree uses to ask the host for the next frame.
///
/// The host (event loop) wires this to its "schedule a redraw" call once at
/// startup. It is the bridge that lets a widget which invalidates itself (an
/// animation step, a caret move, a reactive state change) wake the renderer —
/// the foundation for repainting only what changed instead of every frame.
pub fn install_frame_request(f: impl Fn() + 'static) {
    FRAME_REQUEST.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Ask the host to schedule a frame. A no-op until [`install_frame_request`] is set (e.g. in
/// headless tests), so widget code can always call it safely.
///
/// **One ask per frame, however many widgets ask.** Every widget that changes anything calls this
/// — through `mark_needs_paint` and `mark_needs_layout` — so on a busy tree it runs hundreds of
/// times between two frames. Each of those used to reach the host, and a host whose hook posts to
/// an event queue then woke up hundreds of times to serve a single redraw: measured at 101, 133,
/// 176 and 185 wake-ups in one second against 3 to 15 frames actually drawn, on an *idle* window.
///
/// The docs here have always said the host coalesces; nothing did. It happens here instead of in
/// each host, because every host would otherwise have to know this and one of them would forget —
/// and a widget asking twice is not a mistake to push back onto callers, it is the normal case.
///
/// [`frame_served`] re-arms it, and the host calls that once per turn of its loop.
pub fn request_frame() {
    if FRAME_PENDING.with(|p| p.replace(true)) {
        return;
    }
    FRAME_REQUEST.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f();
        }
    });
}

/// **The host has taken the request** — the next [`request_frame`] should reach it again.
///
/// Called once per turn of the host's loop, before it decides whether to draw. Anything that asks
/// after this point is asking for the *next* frame and must get through, which is why this is not
/// tied to painting: a tree that marks itself while the host is mid-pass still wakes it again.
pub fn frame_served() {
    FRAME_PENDING.with(|p| p.set(false));
}
