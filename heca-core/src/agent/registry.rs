use std::{fmt, sync::Arc};

use super::{AgentDriver, AgentError, AgentInput, AgentState};

/// Driver collection owned by the terminal service. Registration order decides recognition.
#[derive(Clone, Default)]
pub struct AgentDrivers {
    drivers: Vec<Arc<dyn AgentDriver>>,
}

impl AgentDrivers {
    /// An empty collection, including no implicit built-in program names.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an implementation through the same path used by every built-in.
    ///
    /// # Errors
    /// Rejects empty and duplicate driver identifiers.
    pub fn register(&mut self, driver: impl AgentDriver + 'static) -> Result<(), AgentError> {
        self.register_shared(Arc::new(driver))
    }

    /// Add an already-shared implementation without wrapping it in another allocation.
    ///
    /// # Errors
    /// Rejects empty and duplicate driver identifiers.
    pub fn register_shared(&mut self, driver: Arc<dyn AgentDriver>) -> Result<(), AgentError> {
        if driver.id().is_empty() || self.driver(driver.id()).is_some() {
            return Err(AgentError::Driver(driver.id().to_owned()));
        }
        self.drivers.push(driver);
        Ok(())
    }

    /// All registered implementations, in recognition order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn AgentDriver>> {
        self.drivers.iter()
    }

    /// An implementation by its own identity.
    pub fn driver(&self, id: &str) -> Option<&Arc<dyn AgentDriver>> {
        self.drivers.iter().find(|driver| driver.id() == id)
    }

    /// Recognize a foreground program through the registered implementations.
    pub fn recognize(&self, program: &str) -> Option<&Arc<dyn AgentDriver>> {
        self.drivers.iter().find(|driver| driver.recognizes(program))
    }

    /// Decode input only through the recognized program's implementation.
    ///
    /// # Errors
    /// Propagates malformed reports and refuses an implementation claiming another identity.
    pub fn read_status(
        &self,
        program: &str,
        input: AgentInput<'_>,
    ) -> Result<Option<AgentState>, AgentError> {
        let Some(driver) = self.recognize(program) else {
            return Ok(None);
        };
        read_report(driver.as_ref(), input)
    }
}

/// One validation and presentation path for terminal notifications and private-file reports.
pub(super) fn read_report(driver: &dyn AgentDriver, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
    let Some(mut report) = driver.read_status(input)? else { return Ok(None) };
    if report.driver != driver.id() || report.status.as_str().is_empty() {
        return Err(AgentError::Report(driver.id().to_owned()));
    }
    report.name = driver.name().to_owned();
    if let Some(icon) = driver.icon() { report.icon = Some(icon); }
    Ok(Some(report))
}

impl fmt::Debug for AgentDrivers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.drivers.iter().map(|driver| driver.id())).finish()
    }
}
