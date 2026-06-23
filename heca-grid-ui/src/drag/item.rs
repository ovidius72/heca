//! Drag surface and item identifiers.
//!
//! [`DragSurfaceId`] identifies which surface is participating in a drag.
//! [`DragItemId`] is an opaque wrapper around a flat index — each surface
//! interprets it internally.
//!
//! Note there is intentionally **no** "item kind" or payload type here: *what*
//! is being dragged is the app's concern, carried as the generic payload `P` on
//! [`DragContext<P>`](crate::drag::DragContext). The framework stays domain-neutral.

/// Closed set of drag surfaces.
///
/// Add new variants as new surfaces are introduced (right sidebar, inspector, etc.).
/// The compiler will flag all match arms that need updating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragSurfaceId {
    LeftSidebar,
    // RightSidebar — add when the right sidebar gets its tree model.
}

/// Opaque item identifier within a surface.
///
/// Each surface interprets this internally (flat index into sidebar tree,
/// grid coordinate, etc.). Use [`DragItemId::new`] to construct and
/// [`DragItemId::raw`] to extract the inner value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragItemId(usize);

impl DragItemId {
    /// Create a new item ID from a flat index.
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Access the raw flat index.
    ///
    /// Only call this in surface-specific code that knows how to interpret
    /// the index for a particular [`DragSurfaceId`].
    pub fn raw(self) -> usize {
        self.0
    }
}
