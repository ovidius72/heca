use super::*;

pub(crate) struct ExternalDriver {
    id: String,
    programs: Vec<String>,
    report_driver: String,
    icon: Option<AgentIcon>,
}

impl ExternalDriver {
    pub(crate) fn new(id: &str) -> Self {
        Self { id: id.into(), programs: vec!["demo-agent".into()], report_driver: id.into(), icon: None }
    }
}

impl AgentDriver for ExternalDriver {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "Demo" }
    fn icon(&self) -> Option<AgentIcon> { self.icon.clone() }
    fn programs(&self) -> &[String] { &self.programs }

    fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
        let mut environment: std::collections::BTreeMap<String, String> = context.environment.get("DEMO_CONFIG")
            .map(|value| ("DEMO_CONFIG".into(), value.clone())).into_iter().collect();
        environment.insert("DEMO_LAUNCHER".into(), context.reporter.executable.display().to_string());
        Ok(AgentSetup {
            arguments: vec!["--status-file".into(), context.directory.join("status").display().to_string()],
            environment,
        })
    }

    fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        let payload = match input {
            AgentInput::Notification(notification) => notification.payload.as_str(),
            AgentInput::SideChannel(bytes) => std::str::from_utf8(bytes)
                .map_err(|error| AgentError::Report(error.to_string()))?,
        };
        let Some(status) = payload.strip_prefix("demo:") else { return Ok(None) };
        Ok(Some(AgentState::new(&self.report_driver, self.name(), AgentStatus::new(status))))
    }
}

#[test]
fn external_driver_recognizes_paths_and_reports_an_open_status() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let notification = TerminalNotification { code: 9, payload: "demo:reviewing".into() };
    assert_eq!(
        drivers.read_status("/custom/bin/demo-agent", AgentInput::Notification(&notification)).unwrap(),
        Some(AgentState::new("demo", "Demo", AgentStatus::new("reviewing"))),
    );
}

#[test]
fn unrelated_program_cannot_set_agent_status() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let notification = TerminalNotification { code: 9, payload: "demo:working".into() };
    assert_eq!(drivers.read_status("editor", AgentInput::Notification(&notification)).unwrap(), None);
}

#[test]
fn ordinary_notification_is_not_guessed_to_be_an_agent_status() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let notification = TerminalNotification { code: 9, payload: "Download complete".into() };
    assert_eq!(drivers.read_status("demo-agent", AgentInput::Notification(&notification)).unwrap(), None);
}

#[test]
fn driver_cannot_claim_another_drivers_identity() {
    let mut drivers = AgentDrivers::new();
    let mut driver = ExternalDriver::new("demo");
    driver.report_driver = "another".into();
    drivers.register(driver).unwrap();
    let notification = TerminalNotification { code: 9, payload: "demo:working".into() };
    assert!(drivers.read_status("demo-agent", AgentInput::Notification(&notification)).is_err());
}

#[test]
fn empty_report_status_is_rejected() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let notification = TerminalNotification { code: 9, payload: "demo:".into() };
    assert!(drivers.read_status("demo-agent", AgentInput::Notification(&notification)).is_err());
}

#[test]
fn duplicate_registration_keeps_the_original_driver() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    assert!(drivers.register(ExternalDriver::new("demo")).is_err());
    assert_eq!(drivers.iter().count(), 1);
}

#[test]
fn a_registered_icon_follows_both_report_channels_without_entering_native_reports() {
    let mut driver = ExternalDriver::new("an-added-program");
    let icon = AgentIcon::svg("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"/>");
    driver.icon = Some(icon.clone());
    let mut drivers = AgentDrivers::new();
    drivers.register(driver).unwrap();
    let notification = TerminalNotification { code: 9, payload: "demo:reviewing".into() };
    for input in [AgentInput::Notification(&notification), AgentInput::SideChannel(b"demo:reviewing")] {
        let report = drivers.read_status("demo-agent", input).unwrap().unwrap();
        assert_eq!(report.icon, Some(icon.clone()));
    }
    let raw = AgentState::new("an-added-program", "Demo", AgentStatus::new("reviewing"));
    assert!(!serde_json::to_string(&raw).unwrap().contains("icon"));
}

#[test]
fn optional_presentation_fields_preserve_old_reports_and_round_trip_assets() {
    let old = r#"{"driver":"demo","name":"Demo","status":"working","task":null,"detail":null,"turn_id":null}"#;
    let mut report: AgentState = serde_json::from_str(old).unwrap();
    assert_eq!(report, AgentState::new("demo", "Demo", AgentStatus::new("working")));
    for icon in [AgentIcon::file("/package/icons/logo.png"), AgentIcon::svg("<svg/>")] {
        report.icon = Some(icon);
        let bytes = serde_json::to_vec(&report).unwrap();
        assert_eq!(serde_json::from_slice::<AgentState>(&bytes).unwrap(), report);
    }
}

#[test]
fn omitted_model_and_title_are_preserved_only_within_a_known_session() {
    let mut previous = AgentState::new("demo", "Demo", AgentStatus::new("working"));
    previous.session_id = Some("first".into());
    previous.model = Some("Reported model".into());
    previous.session_title = Some("Reported title".into());
    let mut next = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    next.session_id = Some("first".into());
    let kept = next.clone().retain_session(Some(&previous));
    assert_eq!(kept.model, previous.model);
    assert_eq!(kept.session_title, previous.session_title);
    for id in [Some("second".into()), None] {
        next.session_id = id;
        let changed = next.clone().retain_session(Some(&previous));
        assert!(changed.model.is_none() && changed.session_title.is_none());
    }
}
