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
mod model;
mod paint_cx;
mod paint_walk;
#[cfg(test)]
mod tests;
mod tree;

pub use base::Base;
pub use behavior::Component;
pub use model::WidgetModel;
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

mod frame;
pub use frame::{FrameWake, frame_waker, install_frame_request, request_frame, frame_served};
