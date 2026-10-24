use std::path::PathBuf;

use heca_core::agent::IntegrationResource;
use serde::{Deserialize, Serialize};

/// Operation shared by CLI and settings callers. Inspection never installs or repairs anything.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum IntegrationOperation {
    /// Install the chosen integration and enable reporting on its first installation.
    Install,
    /// Apply bundled updates while preserving the user's reporting choices.
    Update,
    /// Remove owned resources and disable reporting for this program.
    Uninstall,
}

/// State checked against the actual files, rather than only the installation receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallationState {
    /// No installation receipt exists.
    NotInstalled,
    /// Files or registrations exist without an ownership receipt; they are not adopted silently.
    Unmanaged,
    /// The recorded resources match the bundled integration and the actual files.
    Installed,
    /// Installed resources are intact, but the bundled version or content has changed.
    Outdated,
    /// An installed resource is missing, unreadable or changed.
    Modified,
    /// An interrupted operation has recovery data waiting for the next explicit change.
    Incomplete,
}

/// One missing or changed piece of an installation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationIssue {
    /// File requiring attention.
    pub path: PathBuf,
    /// Why this resource does not match the recorded installation.
    pub description: String,
}

/// A program's bundled version and the installation found on this machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationStatus {
    /// Registered driver identity.
    pub driver: String,
    /// Configured program label.
    pub name: String,
    /// Integration version shipped by this Heca release.
    pub bundled_version: String,
    /// Last installed version, when a receipt exists.
    pub installed_version: Option<String>,
    /// State checked against the filesystem.
    pub state: InstallationState,
    /// Missing, changed or interrupted resources.
    pub issues: Vec<InstallationIssue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Owned {
    pub resource: IntegrationResource,
    pub managed: bool,
    pub previous: Option<bool>,
    pub created_keys: Vec<Vec<String>>,
    pub created_file: bool,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Receipt {
    pub schema: u32,
    pub driver: String,
    pub version: String,
    pub fingerprint: String,
    pub owned: Vec<Owned>,
}
