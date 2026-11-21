use super::*;

struct AddedDriver {
    programs: Vec<String>,
}

impl AgentDriver for AddedDriver {
    fn id(&self) -> &str {
        "custom"
    }
    fn name(&self) -> &str {
        "Custom agent"
    }
    fn icon(&self) -> Option<AgentIcon> {
        Some(AgentIcon::svg("<svg/>"))
    }
    fn programs(&self) -> &[String] {
        &self.programs
    }
    fn prepare(&self, _: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
        Ok(AgentSetup::default())
    }
    fn read_status(&self, _: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        Ok(Some(AgentState::new(
            self.id(),
            self.name(),
            AgentStatus::new("reviewing"),
        )))
    }
    fn integration(
        &self,
        context: &IntegrationContext,
    ) -> Result<Option<AgentIntegration>, AgentError> {
        Ok(Some(AgentIntegration {
            version: "test-1".into(),
            resources: vec![IntegrationResource::File {
                path: context.directory.join("report.txt"),
                content: b"custom integration".to_vec(),
            }],
        }))
    }
}

#[test]
fn an_embedding_registers_a_driver_without_accessing_a_registry() {
    QUEUE.with(StartupQueue::reset);
    ADDED.with(|added| *added.borrow_mut() = AgentDrivers::new());
    let demo = crate::extension("agent-contract-example");
    demo.agent_driver(AddedDriver {
        programs: vec!["custom-contract-example".into()],
    });
    let other = crate::extension("agent-contract-second-example");
    other.agent_driver(AddedDriver {
        programs: vec!["second-contract-example".into()],
    });
    let mut config = AgentsConfig::default();
    let id = "agent-contract-example.custom";
    let mut drivers = AgentDrivers::new();
    append(&mut drivers, &config);
    assert!(
        drivers.recognize("custom-contract-example").is_none(),
        "declaring a driver does not enable it"
    );
    config.programs.insert(
        id.into(),
        AgentProgramConfig {
            enabled: Some(true),
            name: Some("Configured custom agent".into()),
            executables: Some(vec!["my-custom-command".into()]),
            ..Default::default()
        },
    );
    append(&mut drivers, &config);
    let report = drivers
        .read_status("my-custom-command", AgentInput::SideChannel(b"custom"))
        .unwrap()
        .unwrap();
    let mut expected =
        AgentState::new(id, "Configured custom agent", AgentStatus::new("reviewing"));
    expected.icon = Some(AgentIcon::svg("<svg/>"));
    assert_eq!(report, expected);
    config.programs.get_mut(id).unwrap().icon = Some("/my-package/icons/override.png".into());
    let mut replaced = AgentDrivers::new();
    append(&mut replaced, &config);
    assert_eq!(
        replaced
            .read_status("my-custom-command", AgentInput::SideChannel(b"custom"))
            .unwrap()
            .unwrap()
            .icon,
        Some(AgentIcon::file("/my-package/icons/override.png"))
    );
    config.programs.get_mut(id).unwrap().enabled = Some(false);
    let mut disabled = AgentDrivers::new();
    append(&mut disabled, &config);
    assert!(disabled.recognize("my-custom-command").is_none());

    // Enabling a driver does not require repeating its declared executables.
    config.programs.insert(
        id.into(),
        toml::from_str("enabled = true\nname = 'Only a name override'").unwrap(),
    );
    let mut partial = AgentDrivers::new();
    append(&mut partial, &config);
    assert_eq!(
        partial.recognize("custom-contract-example").unwrap().name(),
        "Only a name override"
    );

    let directory = tempfile::tempdir().unwrap();
    let integrations = crate::agent::integrations_in(
        &config,
        IntegrationContext {
            reporter: heca_core::agent::reporting::ReportingHelper {
                executable: std::env::current_exe().unwrap(),
                max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes,
            },
            home: directory.path().to_owned(),
            directory: directory.path().join("integrations"),
            environment: Default::default(),
        },
        directory.path().join("heca-config.toml"),
    );
    let second_id = "agent-contract-second-example.custom";
    use crate::agent::integration::{InstallationState, IntegrationOperation};
    integrations
        .apply(id, IntegrationOperation::Install)
        .unwrap();
    integrations
        .apply(second_id, IntegrationOperation::Install)
        .unwrap();
    integrations
        .apply(id, IntegrationOperation::Uninstall)
        .unwrap();
    assert_eq!(
        integrations.inspect(second_id).unwrap().state,
        InstallationState::Installed,
        "two extensions with the same driver-local id must own independent installation files"
    );
    integrations
        .apply(second_id, IntegrationOperation::Uninstall)
        .unwrap();
    assert_eq!(
        integrations.inspect(second_id).unwrap().state,
        InstallationState::NotInstalled
    );

    demo.agent_driver(AddedDriver {
        programs: vec!["too-late".into()],
    });
    assert!(
        QUEUE.with(|queue| queue.peek(<[_]>::is_empty)),
        "late declarations are refused"
    );
}

struct Resumable;

impl AgentDriver for Resumable {
    fn id(&self) -> &str {
        "resumable"
    }
    fn name(&self) -> &str {
        "Resumable"
    }
    fn programs(&self) -> &[String] {
        &[]
    }
    fn prepare(&self, _: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
        Ok(AgentSetup::default())
    }
    fn read_status(&self, _: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        Ok(None)
    }
    fn resume(&self, original: &ProcessLaunch, session_id: &str) -> Option<ProcessLaunch> {
        let mut launch = original.clone();
        launch.arguments.push(session_id.into());
        Some(launch)
    }
}

/// **The scope an extension's driver is registered under hides nothing**: the host's resume reaches
/// the driver's own, and a driver without one stays without one.
#[test]
fn a_declared_drivers_resume_is_reached_through_its_scope() {
    let original = ProcessLaunch {
        executable: "/bin/resumable".into(),
        arguments: vec!["--flag".into()],
        cwd: None,
    };
    let scoped = |inner: Arc<dyn AgentDriver>| ConfiguredDriver {
        id: "demo.driver".into(),
        inner,
        config: None,
    };
    let launch = scoped(Arc::new(Resumable))
        .resume(&original, "native-1")
        .expect("the driver's resume");
    assert_eq!(launch.arguments, ["--flag", "native-1"]);
    assert_eq!(
        scoped(Arc::new(AddedDriver {
            programs: Vec::new()
        }))
        .resume(&original, "native-1"),
        None
    );
}
