//! Continuing a program's own conversation: the one rule for when that is possible.

use std::fmt;

use super::AgentDrivers;
use crate::backend::ProcessLaunch;

/// Why a program's conversation could not be continued. Each says what is missing, so the host can
/// tell the user instead of starting a new conversation in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResumeRefusal {
    /// The program never reported a native session identity, so there is nothing to continue.
    NoSessionId,
    /// No registered driver has this identity (the integration is off, or its extension is gone).
    UnknownDriver(String),
    /// The driver has no way to continue a conversation, or cannot use this identity.
    Unsupported(String),
}

impl fmt::Display for ResumeRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSessionId => f.write_str("it never reported its own session identity"),
            Self::UnknownDriver(id) => write!(f, "its driver '{id}' is not registered"),
            Self::Unsupported(id) => write!(f, "its driver '{id}' cannot continue a conversation"),
        }
    }
}

impl AgentDrivers {
    /// **The launch that continues the conversation `session_id` of `driver`'s program**, through
    /// that driver's own [`resume`](super::AgentDriver::resume). Never a fresh conversation: when the
    /// identity is missing, the driver is not registered, or it cannot resume, the reason comes back.
    ///
    /// # Errors
    /// A [`ResumeRefusal`] naming what is missing.
    pub fn resume(
        &self,
        driver: &str,
        session_id: Option<&str>,
        original: &ProcessLaunch,
    ) -> Result<ProcessLaunch, ResumeRefusal> {
        let session_id = session_id.filter(|id| !id.is_empty()).ok_or(ResumeRefusal::NoSessionId)?;
        let driver = self.driver(driver).ok_or_else(|| ResumeRefusal::UnknownDriver(driver.to_owned()))?;
        driver.resume(original, session_id).ok_or_else(|| ResumeRefusal::Unsupported(driver.id().to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tests::ExternalDriver;
    use crate::agent::{AgentDriver, AgentError, AgentLaunchContext, AgentSetup, AgentInput, AgentState};

    /// A driver that continues a conversation by adding `--resume <id>` to the original arguments.
    struct Resuming(ExternalDriver);

    impl AgentDriver for Resuming {
        fn id(&self) -> &str { self.0.id() }
        fn name(&self) -> &str { self.0.name() }
        fn programs(&self) -> &[String] { self.0.programs() }
        fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> { self.0.prepare(context) }
        fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> { self.0.read_status(input) }
        fn resume(&self, original: &ProcessLaunch, session_id: &str) -> Option<ProcessLaunch> {
            let mut launch = original.clone();
            launch.arguments.extend(["--resume".into(), session_id.into()]);
            Some(launch)
        }
    }

    fn original() -> ProcessLaunch {
        ProcessLaunch { executable: "/bin/demo-agent".into(), arguments: vec!["--label".into(), "two words".into()], cwd: Some("/work".into()) }
    }

    fn drivers() -> AgentDrivers {
        let mut drivers = AgentDrivers::new();
        drivers.register(Resuming(ExternalDriver::new("resuming"))).unwrap();
        drivers.register(ExternalDriver::new("plain")).unwrap();
        drivers
    }

    #[test]
    fn a_driver_with_a_resume_continues_the_conversation_it_was_given() {
        let launch = drivers().resume("resuming", Some("native-9"), &original()).unwrap();
        assert_eq!(launch.arguments, ["--label", "two words", "--resume", "native-9"]);
        assert_eq!((launch.executable, launch.cwd), (original().executable, original().cwd));
    }

    #[test]
    fn a_missing_identity_a_missing_driver_and_a_driver_without_resume_each_refuse() {
        let drivers = drivers();
        assert_eq!(drivers.resume("resuming", None, &original()), Err(ResumeRefusal::NoSessionId));
        assert_eq!(drivers.resume("resuming", Some(""), &original()), Err(ResumeRefusal::NoSessionId));
        assert_eq!(drivers.resume("gone", Some("1"), &original()), Err(ResumeRefusal::UnknownDriver("gone".into())));
        assert_eq!(drivers.resume("plain", Some("1"), &original()), Err(ResumeRefusal::Unsupported("plain".into())));
    }
}
