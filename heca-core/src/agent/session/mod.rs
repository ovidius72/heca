//! Private launch wrappers and report collection for the lifetime of one terminal.

use std::{
    collections::VecDeque,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use super::channel::ReportChannel;
use super::{AgentDrivers, AgentError, AgentState};

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
    /// The status an agent shows the moment it starts (`settings.agents.launched_status`).
    pub launched_status: String,
}

/// **How heca's launcher is invoked for `program`**, up to the program's own executable and
/// arguments — the one spelling of it, for the PATH wrappers a typed command runs and for a program
/// heca starts itself (a resumed agent), so the two cannot drift.
pub(super) fn launcher_words(
    options: &AgentOptions,
    directory: &std::path::Path,
    program: &str,
) -> Vec<std::ffi::OsString> {
    vec![
        options.launcher.clone().into(),
        super::launcher::FLAG.into(),
        directory.into(),
        options.launch_timeout.as_millis().to_string().into(),
        options.max_report_bytes.to_string().into(),
        program.into(),
    ]
}

pub(crate) struct AgentSession {
    // Stop the watcher before its private directory is removed.
    channel: Option<ReportChannel>,
    /// The options last configured, for starting a program through the launcher.
    options: Option<AgentOptions>,
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
            channel: None,
            options: None,
            directory,
            drivers: AgentDrivers::new(),
            updates: VecDeque::new(),
            capacity: options.channel_capacity,
            wake,
        };
        session.configure(options)?;
        Ok(session)
    }

    /// Keep the terminal's connection and PATH entry stable while the user changes opt-in.
    /// An empty driver set has no wrappers, watcher or active integration.
    pub(crate) fn configure(&mut self, options: AgentOptions) -> Result<(), AgentError> {
        if options.max_report_bytes == 0
            || options.channel_capacity == 0
            || options.launch_timeout.is_zero()
        {
            return Err(AgentError::Report(
                "agent report limits and launch timeout must be greater than zero".into(),
            ));
        }
        let scripts = wrappers::scripts(&options, self.directory.path())?;
        let reporter = super::reporting::ReportingHelper {
            executable: options.launcher.clone(),
            max_report_bytes: options.max_report_bytes,
            launched_status: options.launched_status.clone(),
            hook_launcher: None,
        };
        if options.drivers.iter().next().is_none() {
            self.channel = None;
        } else if let Some(channel) = &self.channel {
            channel.configure(options.drivers.clone(), reporter, options.channel_capacity)?;
        } else {
            self.channel = Some(ReportChannel::watch(
                self.directory.path(),
                options.drivers.clone(),
                reporter,
                options.channel_capacity,
                self.wake.clone(),
            )?);
        }
        wrappers::publish(&self.bin(), scripts)?;
        self.capacity = options.channel_capacity;
        while self.updates.len() > self.capacity {
            self.updates.pop_front();
        }
        self.drivers = options.drivers.clone();
        self.options = Some(options);
        Ok(())
    }

    /// **`process` started through heca's launcher**, when a driver recognizes it — so an agent heca
    /// starts itself (resumed with a saved session) reports exactly as one typed at a prompt does:
    /// the launch handshake, its integration, its line from launch. `None` when no driver claims it
    /// (it is started as it is).
    pub(crate) fn through_launcher(
        &self,
        process: &crate::backend::ProcessLaunch,
    ) -> Option<crate::backend::ProcessLaunch> {
        let options = self.options.as_ref()?;
        let program = process.executable.file_name()?.to_str()?;
        self.drivers.recognize(program)?;
        let mut words = launcher_words(options, self.directory.path(), program);
        words.push(process.executable.clone().into());
        words.extend(process.arguments.iter().cloned());
        let executable = std::path::PathBuf::from(words.remove(0));
        Some(crate::backend::ProcessLaunch {
            executable,
            arguments: words,
            cwd: process.cwd.clone(),
        })
    }

    pub(crate) fn bin(&self) -> PathBuf {
        self.directory.path().join("bin")
    }
    pub(crate) fn disable(&mut self) -> Result<(), AgentError> {
        wrappers::publish(&self.bin(), Default::default())?;
        self.channel = None;
        self.options = None;
        self.drivers = AgentDrivers::new();
        Ok(())
    }
    pub(crate) fn drivers(&self) -> &AgentDrivers {
        &self.drivers
    }
    pub(crate) fn drain(&self) -> Vec<AgentState> {
        self.channel
            .as_ref()
            .map(ReportChannel::drain)
            .unwrap_or_default()
    }

    pub(crate) fn record(&mut self, state: Option<AgentState>) {
        if self.updates.len() == self.capacity {
            self.updates.pop_front();
        }
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
        return Err(AgentError::Launch(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("agent executable must be one filename: {program}"),
        )));
    }
    Ok(())
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b".~_/-".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
