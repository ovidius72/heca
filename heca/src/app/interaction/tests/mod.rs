//! Tests for [`super`], split by what each group tests.

use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use heca_core::layout::column::Pane;
use heca_core::layout::testing::Windowed;
use heca_core::layout::types::{Point, Rectangle};
use heca_core::layout::workspace::FloatingPane;
use heca_core::layout::{FocusDomain, PaneId, Size};

pub(super) use super::*;
pub(super) use super::route::*;

/// Helper to create a minimal Session for routing tests.
pub(super) fn test_session() -> Windowed {
    Windowed::new(Size::new(800.0, 600.0), 1.0)
}


/// A surface identity for the tests, from a name the way a real one is. This module only needs
/// two that differ — and naming them is the point: a surface is known by the key it declares,
/// so a test says `surface("expose")`, not an id it had to be handed.
pub(super) fn surface(name: &str) -> SurfaceKey {
    SurfaceKey::of(name)
}

mod floating;
mod focus;
mod policy;
mod route;
mod view_intent;
