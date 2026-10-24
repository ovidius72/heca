//! Explicit machine-local installation. CLI, embeddings and future settings share these operations.

mod document;
mod model;
mod storage;

use std::{fs::{self, File, OpenOptions}, io, path::{Component, PathBuf}};

use heca_core::agent::{AgentDrivers, AgentIntegration, IntegrationContext, IntegrationResource};
use sha2::{Digest, Sha256};

pub use model::{InstallationIssue, InstallationState, InstallationStatus, IntegrationOperation};
use model::Receipt;

const RECEIPT_SCHEMA: u32 = 1;

/// Installer over the same drivers used by terminal reporting. No session server or window required.
pub struct Integrations {
    drivers: AgentDrivers,
    context: IntegrationContext,
    config: PathBuf,
}

impl Integrations {
    /// Registered drivers and the user's normal installation locations, without a server or window.
    ///
    /// Construction installs nothing. Invalid configuration is returned to the caller rather
    /// than replaced with defaults before an explicit installation change.
    pub fn machine_local() -> io::Result<Self> {
        let config = heca_config::loader::AppConfig::try_load().map_err(io::Error::other)?;
        super::integrations(&config.config.settings.agents)
    }

    /// Use explicit user locations, so tests and callers never need to change process-global state.
    pub fn new(drivers: AgentDrivers, context: IntegrationContext, config: PathBuf) -> Self {
        Self { drivers, context, config }
    }

    /// Check every driver that supplies a persistent integration. This never changes files.
    pub fn list(&self) -> io::Result<Vec<InstallationStatus>> {
        self.drivers.iter().filter_map(|driver| {
            match driver.integration(&self.driver_context(driver.id())) {
                Ok(Some(_)) => Some(self.inspect(driver.id())),
                Ok(None) => None,
                Err(error) => Some(Err(io::Error::other(error))),
            }
        }).collect()
    }

    /// Compare the receipt with the actual owned files and native registrations, without repair.
    pub fn inspect(&self, id: &str) -> io::Result<InstallationStatus> {
        let (name, bundle) = self.bundle(id)?;
        let receipt = self.receipt(id)?;
        let mut status = InstallationStatus {
            driver: id.to_owned(), name,
            bundled_version: bundle.version.clone(),
            installed_version: receipt.as_ref().map(|receipt| receipt.version.clone()),
            state: InstallationState::NotInstalled, issues: Vec::new(),
        };
        if storage::read(&self.journal())?.is_some() {
            status.state = InstallationState::Incomplete;
            status.issues.push(InstallationIssue {
                path: self.journal(), description: "an interrupted installation needs recovery before the next change".into(),
            });
            return Ok(status);
        }
        let Some(receipt) = receipt else {
            let mut files = storage::Files::default();
            for resource in &bundle.resources {
                let present = match resource {
                    IntegrationResource::File { .. } => files.get(resource.path())?.is_some(),
                    IntegrationResource::TomlBool { .. } => false,
                    _ => document::matches(&mut files, &model::Owned {
                        resource: resource.clone(), managed: false, previous: None,
                        created_keys: Vec::new(), created_file: false,
                    })?,
                };
                if present {
                    status.state = InstallationState::Unmanaged;
                    status.issues.push(InstallationIssue {
                        path: resource.path().to_owned(), description: "existing content has no Heca ownership receipt".into(),
                    });
                }
            }
            return Ok(status);
        };
        status.state = if receipt.fingerprint == fingerprint(&bundle)? {
            InstallationState::Installed
        } else { InstallationState::Outdated };
        let mut files = storage::Files::default();
        for owned in &receipt.owned {
            match document::matches(&mut files, owned) {
                Ok(true) => {}
                result => {
                    status.state = InstallationState::Modified;
                    status.issues.push(InstallationIssue {
                        path: owned.resource.path().to_owned(),
                        description: match result {
                            Err(error) => error.to_string(),
                            _ => "owned content is missing or has changed".into(),
                        },
                    });
                }
            }
        }
        Ok(status)
    }

    /// Perform an explicit installation change. Modified owned content is never overwritten.
    ///
    /// Changes are validated together, journalled privately and written atomically per file.
    /// A subsequent change recovers an interrupted batch before doing new work.
    pub fn apply(&self, id: &str, operation: IntegrationOperation) -> io::Result<InstallationStatus> {
        let _lock = self.lock()?;
        storage::recover(&self.journal())?;
        let (_, bundle) = self.bundle(id)?;
        let previous = self.receipt(id)?;
        if matches!(operation, IntegrationOperation::Update) && previous.is_none() {
            return Err(io::Error::other("integration is not installed; install it first"));
        }
        let mut files = storage::Files::default();
        if let Some(previous) = &previous {
            for owned in &previous.owned {
                if owned.managed && !document::matches(&mut files, owned)?
                    && files.get(owned.resource.path())?.is_some()
                {
                    return Err(io::Error::other(format!(
                        "owned content changed; inspect it before making changes: {}", owned.resource.path().display(),
                    )));
                }
            }
            for owned in previous.owned.iter().rev() { document::remove(&mut files, owned)?; }
        }
        let receipt_path = self.receipt_path(id);
        if matches!(operation, IntegrationOperation::Uninstall) {
            files.set(&receipt_path, None)?;
        } else {
            let mut owned = Vec::new();
            for resource in &bundle.resources {
                if let Some(entry) = document::add(&mut files, resource)? { owned.push(entry); }
            }
            let receipt = Receipt {
                schema: RECEIPT_SCHEMA, driver: id.to_owned(), version: bundle.version.clone(),
                fingerprint: fingerprint(&bundle)?, owned,
            };
            files.set(&receipt_path, Some(serde_json::to_vec_pretty(&receipt)?))?;
        }
        // Installation and the user's current reporting choice are separate. Updates never
        // re-enable reporting that the user turned off after installing.
        if previous.is_none() && matches!(operation, IntegrationOperation::Install)
            || matches!(operation, IntegrationOperation::Uninstall)
        {
            document::add(&mut files, &IntegrationResource::TomlBool {
                path: self.config.clone(),
                keys: ["settings", "agents", "programs", id, "enabled"].map(str::to_owned).into(),
                value: !matches!(operation, IntegrationOperation::Uninstall),
            })?;
        }
        files.commit(&self.journal())?;
        self.inspect(id)
    }

    fn bundle(&self, id: &str) -> io::Result<(String, AgentIntegration)> {
        let driver = self.drivers.driver(id).ok_or_else(|| io::Error::other(format!("unknown agent integration: {id}")))?;
        let bundle = driver.integration(&self.driver_context(id)).map_err(io::Error::other)?
            .ok_or_else(|| io::Error::other(format!("driver '{id}' needs no persistent integration")))?;
        for resource in &bundle.resources {
            let path = resource.path();
            if !path.is_absolute() || path.components().any(|part| matches!(part, Component::ParentDir))
                || path == self.journal() || path == self.context.directory.join("install.lock")
                || path.starts_with(self.context.directory.join("receipts"))
            {
                return Err(io::Error::other(format!("invalid integration resource path: {}", path.display())));
            }
        }
        Ok((driver.name().to_owned(), bundle))
    }

    fn receipt(&self, id: &str) -> io::Result<Option<Receipt>> {
        let Some(bytes) = storage::read(&self.receipt_path(id))? else { return Ok(None) };
        let receipt: Receipt = serde_json::from_slice(&bytes)?;
        if receipt.schema != RECEIPT_SCHEMA || receipt.driver != id {
            return Err(io::Error::other("invalid or unsupported integration receipt"));
        }
        Ok(Some(receipt))
    }

    fn driver_context(&self, id: &str) -> IntegrationContext {
        let mut context = self.context.clone();
        context.directory = context.directory.join("drivers").join(encoded_id(id));
        context
    }

    fn receipt_path(&self, id: &str) -> PathBuf {
        let name = encoded_id(id);
        self.context.directory.join("receipts").join(format!("{name}.json"))
    }

    fn journal(&self) -> PathBuf { self.context.directory.join("pending.json") }

    fn lock(&self) -> io::Result<File> {
        fs::create_dir_all(&self.context.directory)?;
        let path = self.context.directory.join("install.lock");
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(io::Error::other("integration lock must not be a symlink"));
        }
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.try_lock().map_err(|error| io::Error::other(format!("another integration operation is running: {error}")))?;
        Ok(file)
    }
}

fn fingerprint(bundle: &AgentIntegration) -> io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(bundle)?)))
}

fn encoded_id(id: &str) -> String {
    id.bytes().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
