//! Errors at the description/host boundary, with the failing node's child path.

use std::fmt;
use heca_view::WidgetId;

/// Why a description or widget registration could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RealizeErrorKind {
    UnknownWidget(WidgetId),
    DuplicateWidget(WidgetId),
    InvalidDefinition { widget: WidgetId, detail: String },
    UnsupportedVersion { widget: WidgetId, requested: u32, minimum: u32, maximum: u32 },
    InvalidProperty { widget: WidgetId, property: String, detail: String },
    InvalidChild { widget: WidgetId, index: usize, detail: String },
}

/// A failure is returned to the host; it never becomes an empty placeholder widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizeError {
    pub kind: RealizeErrorKind,
    /// Indices from the root to the failing node; empty means the root.
    pub path: Vec<usize>,
}

impl RealizeError {
    pub(crate) fn new(kind: RealizeErrorKind) -> Self { Self { kind, path: Vec::new() } }

/// A property a factory could not use. Registered input validators report this as a
/// recoverable diagnostic and let the native default apply.
    pub fn invalid_property(widget: WidgetId, property: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(RealizeErrorKind::InvalidProperty { widget, property: property.into(), detail: detail.into() })
    }
}

impl fmt::Display for RealizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "widget tree at {:?}: ", self.path)?;
        match &self.kind {
            RealizeErrorKind::UnknownWidget(id) => write!(f, "unknown widget '{id}'"),
            RealizeErrorKind::DuplicateWidget(id) => write!(f, "widget '{id}' is already registered"),
            RealizeErrorKind::InvalidDefinition { widget, detail } => write!(f, "invalid definition for '{widget}': {detail}"),
            RealizeErrorKind::UnsupportedVersion { widget, requested, minimum, maximum } => write!(f, "widget '{widget}' version {requested} is unsupported (expected {minimum}..={maximum})"),
            RealizeErrorKind::InvalidProperty { widget, property, detail } => write!(f, "invalid '{property}' on '{widget}': {detail}"),
            RealizeErrorKind::InvalidChild { widget, index, detail } => write!(f, "invalid child {index} of '{widget}': {detail}"),
        }
    }
}

impl std::error::Error for RealizeError {}
