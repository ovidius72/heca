use std::{collections::BTreeMap, fs, path::Path};

use heca_core::agent::{AgentDriver, AgentError, AgentInput, AgentLaunchContext, AgentSetup, AgentState};
use serde_json::json;

use super::*;

struct Fixture { home: PathBuf, version: String }

impl AgentDriver for Fixture {
    fn id(&self) -> &str { "demo.agent" }
    fn name(&self) -> &str { "Custom program" }
    fn programs(&self) -> &[String] { &[] }
    fn prepare(&self, _: &AgentLaunchContext) -> Result<AgentSetup, AgentError> { Ok(AgentSetup::default()) }
    fn read_status(&self, _: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> { Ok(None) }
    fn integration(&self, _: &IntegrationContext) -> Result<Option<AgentIntegration>, AgentError> {
        Ok(Some(AgentIntegration {
            version: self.version.clone(), resources: vec![
                IntegrationResource::File { path: self.home.join("native/report.js"), content: self.version.as_bytes().to_vec() },
                IntegrationResource::JsonArrayEntry {
                    path: self.home.join("native/settings.json"), keys: vec!["hooks".into(), "Stop".into()],
                    value: json!({"hooks":[{"type":"command","command":"heca-report"}]}),
                },
                IntegrationResource::JsonArrayEntry {
                    path: self.home.join("native/settings.json"), keys: vec!["hooks".into(), "Start".into()],
                    value: json!({"hooks":[{"type":"command","command":"heca-start"}]}),
                },
                IntegrationResource::TomlBool {
                    path: self.home.join("native/config.toml"), keys: vec!["features".into(), "hooks".into()], value: true,
                },
                IntegrationResource::TomlEntry {
                    path: self.home.join("native/config.toml"), keys: vec!["connections".into(), "demo".into()],
                    value: json!({"command":"reporter", "version":self.version, "args":["--report"], "env":{"destination":"private"}}),
                },
            ],
        }))
    }
}

#[test]
fn install_preserves_an_existing_registration_and_refuses_to_replace_a_different_one() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("native")).unwrap();
    let native = root.path().join("native/config.toml");
    let original = "# owned by the user\n[connections.demo]\ncommand = 'mine'\n";
    fs::write(&native, original).unwrap();
    let service = fixture(root.path(), "1");
    assert!(service.apply("demo.agent", IntegrationOperation::Install).is_err());
    assert_eq!(fs::read_to_string(&native).unwrap(), original);
    assert!(!root.path().join("native/report.js").exists(), "the failed batch must write nothing");
    let original = "[connections.demo]\ncommand = 'reporter'\nversion = '1'\nargs = ['--report']\n[connections.demo.env]\ndestination = 'private'\n";
    fs::write(&native, original).unwrap();
    service.apply("demo.agent", IntegrationOperation::Install).unwrap();
    service.apply("demo.agent", IntegrationOperation::Uninstall).unwrap();
    let restored: toml::Value = toml::from_str(&fs::read_to_string(&native).unwrap()).unwrap();
    assert_eq!(restored, toml::from_str::<toml::Value>(original).unwrap());
}

fn fixture(root: &Path, version: &str) -> Integrations {
    let mut drivers = AgentDrivers::new();
    drivers.register(Fixture { home: root.to_owned(), version: version.into() }).unwrap();
    Integrations::new(drivers, IntegrationContext {
        reporter: heca_core::agent::reporting::ReportingHelper { executable: std::env::current_exe().unwrap(), max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes },
        home: root.to_owned(), directory: root.join("heca-data/integrations"), environment: BTreeMap::new(),
    }, root.join("heca-config/config.toml"))
}

#[test]
fn custom_driver_install_repeat_update_and_uninstall_preserve_user_settings() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("native")).unwrap();
    let settings = root.path().join("native/settings.json");
    let original = json!({"theme":"mine","hooks":{"Stop":[{"hooks":[{"command":"my-hook"}]}]}});
    fs::write(&settings, original.to_string()).unwrap();
    let config = root.path().join("native/config.toml");
    fs::write(&config, "# keep my comment\nmodel = 'mine'\n[features]\nhooks = false\n").unwrap();
    let service = fixture(root.path(), "1");
    assert_eq!(service.inspect("demo.agent").unwrap().state, InstallationState::NotInstalled);
    assert!(!service.context.directory.exists(), "inspection must not create data or install anything");
    assert_eq!(service.apply("demo.agent", IntegrationOperation::Install).unwrap().state, InstallationState::Installed);
    service.apply("demo.agent", IntegrationOperation::Install).unwrap();
    let installed: serde_json::Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
    assert_eq!(installed["hooks"]["Stop"].as_array().unwrap().len(), 2, "repeat install must not duplicate registrations");
    let updated = fixture(root.path(), "2");
    assert_eq!(updated.inspect("demo.agent").unwrap().state, InstallationState::Outdated);
    updated.apply("demo.agent", IntegrationOperation::Update).unwrap();
    assert_eq!(fs::read(root.path().join("native/report.js")).unwrap(), b"2");
    updated.apply("demo.agent", IntegrationOperation::Uninstall).unwrap();
    assert!(!root.path().join("native/report.js").exists());
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&fs::read(&settings).unwrap()).unwrap(), original);
    let preserved = fs::read_to_string(config).unwrap();
    assert!(preserved.contains("# keep my comment"));
    assert!(preserved.contains("model = 'mine'"));
    assert!(preserved.contains("hooks = false"));
}

#[test]
fn modified_files_block_the_entire_update_without_changing_other_files() {
    let root = tempfile::tempdir().unwrap();
    let service = fixture(root.path(), "1");
    service.apply("demo.agent", IntegrationOperation::Install).unwrap();
    let path = root.path().join("native/report.js");
    fs::write(&path, "my changes").unwrap();
    let status = service.inspect("demo.agent").unwrap();
    assert_eq!(status.state, InstallationState::Modified);
    let config_before = fs::read(root.path().join("native/settings.json")).unwrap();
    assert!(fixture(root.path(), "2").apply("demo.agent", IntegrationOperation::Update).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"my changes");
    assert_eq!(fs::read(root.path().join("native/settings.json")).unwrap(), config_before);
    assert!(service.apply("demo.agent", IntegrationOperation::Uninstall).is_err());
}

#[test]
fn installer_refuses_unowned_files_and_malformed_native_settings_before_writing() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("native")).unwrap();
    let path = root.path().join("native/report.js");
    fs::write(&path, "mine").unwrap();
    let service = fixture(root.path(), "1");
    assert_eq!(service.inspect("demo.agent").unwrap().state, InstallationState::Unmanaged);
    assert!(service.apply("demo.agent", IntegrationOperation::Install).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"mine");
    fs::remove_file(&path).unwrap();
    fs::write(root.path().join("native/settings.json"), "not json").unwrap();
    assert!(service.apply("demo.agent", IntegrationOperation::Install).is_err());
    assert!(!path.exists(), "preparing a batch must not partially install it");
}

#[test]
fn updates_preserve_disabled_reporting_and_uninstall_keeps_unowned_registrations() {
    let root = tempfile::tempdir().unwrap();
    let service = fixture(root.path(), "1");
    fs::create_dir_all(root.path().join("native")).unwrap();
    let (_, bundle) = service.bundle("demo.agent").unwrap();
    let mut files = storage::Files::default();
    document::add(&mut files, &bundle.resources[1]).unwrap();
    files.commit(&service.journal()).unwrap();
    let original = fs::read(root.path().join("native/settings.json")).unwrap();
    service.apply("demo.agent", IntegrationOperation::Install).unwrap();
    fs::write(&service.config, "# my choice\n[settings.agents.programs.'demo.agent']\nenabled = false\n").unwrap();
    fixture(root.path(), "2").apply("demo.agent", IntegrationOperation::Update).unwrap();
    assert!(fs::read_to_string(&service.config).unwrap().contains("enabled = false"));
    fixture(root.path(), "2").apply("demo.agent", IntegrationOperation::Uninstall).unwrap();
    assert_eq!(fs::read(root.path().join("native/settings.json")).unwrap(), original);
    assert!(!root.path().join("native/config.toml").exists());
}

#[test]
fn interrupted_batches_are_reported_then_recovered_before_the_next_change() {
    let root = tempfile::tempdir().unwrap();
    let service = fixture(root.path(), "1");
    fs::create_dir_all(&service.context.directory).unwrap();
    let path = root.path().join("native/report.js");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "partial").unwrap();
    heca_config::state_file::write_json(&service.journal(), &json!([
        {"path":path,"before":null,"after":b"partial".to_vec()}
    ])).unwrap();
    assert_eq!(service.inspect("demo.agent").unwrap().state, InstallationState::Incomplete);
    service.apply("demo.agent", IntegrationOperation::Install).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"1");
    assert!(!service.journal().exists());
}

#[test]
fn a_second_operation_cannot_enter_while_the_installer_is_locked() {
    let root = tempfile::tempdir().unwrap();
    let service = fixture(root.path(), "1");
    let _lock = service.lock().unwrap();
    assert!(service.apply("demo.agent", IntegrationOperation::Install).is_err());
    assert!(!root.path().join("native/report.js").exists());
}

#[cfg(unix)]
#[test]
fn a_failed_write_restores_files_already_changed_in_the_batch() {
    use std::os::unix::fs::PermissionsExt;
    struct RestoreDirectory { path: PathBuf, permissions: fs::Permissions }
    impl Drop for RestoreDirectory {
        fn drop(&mut self) { let _ = fs::set_permissions(&self.path, self.permissions.clone()); }
    }
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("a.txt");
    let blocked = root.path().join("z");
    fs::create_dir(&blocked).unwrap();
    let last = blocked.join("file.txt");
    fs::write(&first, "original first").unwrap();
    fs::write(&last, "original last").unwrap();
    let mut files = storage::Files::default();
    files.set(&first, Some(b"changed first".to_vec())).unwrap();
    files.set(&last, Some(b"changed last".to_vec())).unwrap();
    let _restore = RestoreDirectory { path: blocked.clone(), permissions: fs::metadata(&blocked).unwrap().permissions() };
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).unwrap();
    // Root can write through permission bits, so this fault cannot be exercised when run as root.
    if let Ok(probe) = fs::File::create(blocked.join("probe")) { drop(probe); return }
    let journal = root.path().join("pending.json");
    assert!(files.commit(&journal).is_err());
    assert_eq!(fs::read(&first).unwrap(), b"original first");
    assert_eq!(fs::read(&last).unwrap(), b"original last");
    assert!(!journal.exists(), "a successful rollback must clear recovery data");
}
