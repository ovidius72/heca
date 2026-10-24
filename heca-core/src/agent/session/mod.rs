//! Private launch wrappers and report collection for the lifetime of one terminal.

use std::{collections::VecDeque, fs, io, path::{Component, Path, PathBuf}, sync::Arc, time::Duration};

use super::{AgentDrivers, AgentError, AgentState};
use super::channel::ReportChannel;

mod wrappers;

/// Reporting policy from the host's configuration, without a second set of defaults.
#[derive(Clone, Debug)]
pub struct AgentOptions {
    pub drivers: AgentDrivers,
    pub max_report_bytes: usize,
    pub channel_capacity: usize,
    pub launch_timeout: Duration,
    /// The host executable, whose shared entry handles `launcher::run_if_requested`.
    pub launcher: PathBuf,
}

pub(crate) struct AgentSession {
    // Stop the watcher before its private directory is removed.
    channel: Option<ReportChannel>,
    directory: tempfile::TempDir,
    drivers: AgentDrivers,
    updates: VecDeque<Option<AgentState>>,
    capacity: usize,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl AgentSession {
    pub(crate) fn prepare(
        options: AgentOptions,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, AgentError> {
        let directory = tempfile::Builder::new().prefix("heca-agent-").tempdir()?;
        fs::create_dir(directory.path().join("bin"))?;
        let mut session = Self {
            channel: None, directory, drivers: AgentDrivers::new(),
            updates: VecDeque::new(), capacity: options.channel_capacity, wake,
        };
        session.configure(options)?;
        Ok(session)
    }

    /// Keep the terminal's connection and PATH entry stable while the user changes opt-in.
    /// An empty driver set has no wrappers, watcher or active integration.
    pub(crate) fn configure(&mut self, options: AgentOptions) -> Result<(), AgentError> {
        if options.max_report_bytes == 0 || options.channel_capacity == 0 || options.launch_timeout.is_zero() {
            return Err(AgentError::Report("agent report limits and launch timeout must be greater than zero".into()));
        }
        let scripts = wrappers::scripts(&options, self.directory.path())?;
        let reporter = super::reporting::ReportingHelper {
            executable: options.launcher.clone(), max_report_bytes: options.max_report_bytes,
        };
        if options.drivers.iter().next().is_none() {
            self.channel = None;
        } else if let Some(channel) = &self.channel {
            channel.configure(options.drivers.clone(), reporter, options.channel_capacity)?;
        } else {
            self.channel = Some(ReportChannel::watch(self.directory.path(), options.drivers.clone(),
                reporter, options.channel_capacity, self.wake.clone())?);
        }
        wrappers::publish(&self.bin(), scripts)?;
        self.capacity = options.channel_capacity;
        while self.updates.len() > self.capacity { self.updates.pop_front(); }
        self.drivers = options.drivers;
        Ok(())
    }

    pub(crate) fn bin(&self) -> PathBuf { self.directory.path().join("bin") }
    pub(crate) fn disable(&mut self) -> Result<(), AgentError> {
        wrappers::publish(&self.bin(), Default::default())?;
        self.channel = None;
        self.drivers = AgentDrivers::new();
        Ok(())
    }
    pub(crate) fn drivers(&self) -> &AgentDrivers { &self.drivers }
    pub(crate) fn drain(&self) -> Vec<AgentState> { self.channel.as_ref().map(ReportChannel::drain).unwrap_or_default() }

    pub(crate) fn record(&mut self, state: Option<AgentState>) {
        if self.updates.len() == self.capacity { self.updates.pop_front(); }
        self.updates.push_back(state);
    }

    pub(crate) fn take_updates(&mut self) -> Vec<Option<AgentState>> {
        self.updates.drain(..).collect()
    }
}

fn validate_program(program: &str) -> Result<(), AgentError> {
    let mut parts = Path::new(program).components();
    if program.contains(['\0', '\n', '\r'])
        || !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
    {
        return Err(AgentError::Launch(io::Error::new(io::ErrorKind::InvalidInput,
            format!("agent executable must be one filename: {program}"))));
    }
    Ok(())
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn encode(value: &str) -> String {
    value.bytes().map(|byte| {
        if byte.is_ascii_alphanumeric() || b".~_/-".contains(&byte) {
            char::from(byte).to_string()
        } else {
            format!("%{byte:02X}")
        }
    }).collect()
}

#[cfg(test)]
mod tests;
