//! What an extension service shows one window, on the wire.

use serde_json::Value;

/// One step of a service's projection for one window. The payload is the service's own JSON;
/// everything around it is what heca needs to keep the window in step.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ServiceUpdate {
    /// `<extension>.<short>`.
    pub(crate) service: String,
    /// The service's [`SCHEMA_VERSION`](crate::entry::service::Service::SCHEMA_VERSION).
    pub(crate) schema_version: u32,
    /// The run of the server this came from. A window holding another epoch's data needs a snapshot.
    pub(crate) server_epoch: u64,
    /// The revision the window holds after applying this.
    pub(crate) revision: u64,
    pub(crate) kind: ServiceUpdateKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ServiceUpdateKind {
    /// Everything; whatever the window held is replaced.
    Snapshot(Value),
    /// A change on top of `base_revision`, which the window must hold.
    Delta { base_revision: u64, value: Value },
    /// The window is shown nothing now: drop what it was shown.
    Removed,
}
