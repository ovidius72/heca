use std::{fs, process::Command};

use super::*;

fn context(directory: &std::path::Path) -> AgentLaunchContext {
    AgentLaunchContext {
        reporter: heca_core::agent::reporting::ReportingHelper { executable: std::env::current_exe().unwrap(), max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes },
        directory: directory.to_owned(), cwd: directory.to_owned(),
        endpoint: Some(directory.join("reports.jsonl")),
        environment: Default::default(),
    }
}

fn enabled() -> AgentsConfig {
    let mut config = AgentsConfig::default();
    for program in config.programs.values_mut() { program.enabled = Some(true); }
    config
}

#[test]
fn reporting_is_off_until_the_user_chooses_a_program() {
    let mut config = AgentsConfig::default();
    assert!(super::super::options(&config).unwrap().drivers.iter().next().is_none(), "no wrappers or hooks before a program is enabled");
    config.programs.get_mut("codex").unwrap().enabled = Some(true);
    let options = super::super::options(&config).unwrap();
    assert_eq!(options.drivers.iter().map(|driver| driver.id()).collect::<Vec<_>>(), ["codex"]);
    config.enabled = false;
    assert!(super::super::options(&config).unwrap().drivers.iter().next().is_none(), "the master switch disables every integration");
}

#[test]
fn claude_defaults_report_completion_on_stop_rather_than_session_exit() {
    let config = AgentsConfig::default();
    let reports = &config.programs["claude"].reports;
    assert_eq!(reports["Stop"], "finished");
    assert_eq!(reports["StopFailure"], "error");
    assert!(!reports.contains_key("SessionEnd"), "a failed or closed session is not a successful task");
}

#[test]
fn the_builtins_use_one_contract_with_configured_names_and_open_statuses() {
    let mut config = enabled();
    config.programs.get_mut("claude").unwrap().name = Some("My Claude".into());
    let drivers = configured(&config);
    assert_eq!(drivers.iter().count(), config.programs.len());
    for driver in drivers.iter() {
        let report = AgentState::new(driver.id(), driver.name(), AgentStatus::new("custom_phase"));
        let notification = TerminalNotification { code: 9, payload: format!("heca-agent;{}", json(&report).unwrap()) };
        assert_eq!(driver.read_status(AgentInput::Notification(&notification)).unwrap(), Some(report));
    }
    assert_eq!(drivers.recognize("/usr/local/bin/claude").unwrap().name(), "My Claude");
}

#[test]
fn every_builtin_supplies_its_mark_and_accepts_a_configured_asset_override() {
    let mut config = enabled();
    let drivers = configured(&config);
    for driver in drivers.iter() {
        let raw = AgentState::new(driver.id(), driver.name(), AgentStatus::new("working"));
        let payload = json(&raw).unwrap();
        let report = drivers.read_status(&driver.programs()[0], AgentInput::SideChannel(payload.as_bytes()))
            .unwrap().unwrap();
        let Some(AgentIcon::Svg(svg)) = report.icon else { panic!("missing mark for {}", driver.id()) };
        assert!(svg.contains("<svg"));
        assert!(!svg.contains("currentColor"), "brand fills must not inherit text color: {}", driver.id());
        assert!(raw.icon.is_none(), "native templates need not repeat the registered artwork");
        config.programs.get_mut(driver.id()).unwrap().icon = Some("/package/custom-logo.svg".into());
    }
    let customized = configured(&config);
    for driver in customized.iter() {
        assert_eq!(driver.icon(), Some(AgentIcon::file("/package/custom-logo.svg")));
    }
}

#[test]
fn the_codex_mark_has_transparent_corners_and_visible_brand_artwork() {
    use heca_grid_ui::{Color, frame_waker, frame_served, install_frame_request};
    use heca_grid_ui::image::ImageSource;
    use std::{sync::mpsc, time::Duration};
    let (tx, rx) = mpsc::channel();
    install_frame_request(move || { let _ = tx.send(()); });
    let source = ImageSource::bytes(include_bytes!("assets/icons/codex.svg").as_slice());
    let handle = source.load(frame_waker());
    let image = loop {
        frame_served();
        if let Some(result) = handle.result() { break result.unwrap() }
        rx.recv_timeout(Duration::from_secs(5)).expect("decode wakes the UI");
    };
    let raster = image.raster(64, 64, Color::rgb(255, 0, 0), frame_waker());
    let pixels = loop {
        frame_served();
        if let Some(result) = raster.result() { break result.unwrap() }
        rx.recv_timeout(Duration::from_secs(5)).expect("raster wakes the UI");
    };
    for (x, y) in [(0, 0), (63, 0), (0, 63), (63, 63)] {
        assert_eq!(pixels.rgba[(y * 64 + x) * 4 + 3], 0, "the mark must not have a background tile");
    }
    assert!(pixels.rgba.chunks_exact(4).any(|pixel| pixel[3] == 255),
        "the brand artwork must remain visible");
    assert!(pixels.rgba.chunks_exact(4).all(|pixel| pixel[3] == 0 || pixel[..3] == [0, 0, 0]),
        "the requested black mark must not inherit text tint or keep the old gradient");
}

fn installed(directory: &std::path::Path, config: &AgentsConfig) -> super::super::integration::Integrations {
    super::super::integration::Integrations::new(available(config), IntegrationContext {
        reporter: heca_core::agent::reporting::ReportingHelper { executable: std::env::current_exe().unwrap(), max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes },
        home: directory.to_owned(), directory: directory.join("heca-data"), environment: Default::default(),
    }, directory.join("heca-config.toml"))
}

fn hooks(directory: &std::path::Path, native: &str) -> serde_json::Value {
    serde_json::from_slice(&fs::read(directory.join(native)).unwrap()).unwrap()
}

#[test]
fn installed_claude_hooks_use_the_shared_reporter_and_preserve_native_matchers() {
    let directory = tempfile::tempdir().unwrap();
    let config = enabled();
    installed(directory.path(), &config).apply("claude", super::super::integration::IntegrationOperation::Install).unwrap();
    let driver = claude::configured(&config).unwrap();
    let context = context(directory.path());
    let setup = driver.prepare(&context).unwrap();
    assert!(setup.arguments.is_empty(), "launch must not inject temporary settings");
    let settings = hooks(directory.path(), ".claude/settings.json");
    let handler = &settings["hooks"]["Stop"][0]["hooks"][0];
    assert_eq!(handler["type"], "command");
    assert_eq!(handler["command"], context.reporter.command_hook("claude", "Stop").unwrap());
    for group in settings["hooks"]["Notification"].as_array().unwrap() {
        let matcher = group["matcher"].as_str().unwrap();
        assert_eq!(group["hooks"][0]["command"], context.reporter.command_hook("claude", &format!("Notification:{matcher}")).unwrap());
    }
    assert!(!setup.environment.keys().any(|key| key.starts_with("HECA_AGENT_EVENT_")));
}

fn codex_connection(setup: &AgentSetup) -> serde_json::Value {
    assert_eq!(setup.arguments.len(), 2);
    assert_eq!(setup.arguments[0], "-c");
    let config: toml::Value = toml::from_str(&setup.arguments[1]).unwrap();
    serde_json::to_value(&config["mcp_servers"]["heca_status"]).unwrap()
}

#[test]
fn installed_codex_hooks_use_session_local_mcp_without_changing_hook_trust() {
    let directory = tempfile::tempdir().unwrap();
    let config = enabled();
    installed(directory.path(), &config).apply("codex", super::super::integration::IntegrationOperation::Install).unwrap();
    let driver = codex::configured(&config).unwrap();
    let setup = driver.prepare(&context(directory.path())).unwrap();
    let connection = codex_connection(&setup);
    assert_eq!(connection["env"]["HECA_AGENT_REPORT"], setup.environment["HECA_AGENT_REPORT"]);
    assert_eq!(connection["env"]["HECA_AGENT_EVENTS"], setup.environment["HECA_AGENT_EVENTS"]);
    assert!(!setup.arguments.join(" ").contains("bypass"));
    assert!(!setup.arguments.join(" ").contains("no-daemon"));
    let settings = hooks(directory.path(), ".codex/hooks.json");
    let handler = &settings["hooks"]["Stop"][0]["hooks"][0];
    assert_eq!(handler["type"], "mcp_tool");
    assert_eq!(handler["server"], "heca_status");
    assert_eq!(handler["tool"], ReportingHelper::TOOL);
    assert_eq!(handler["input"]["turn_id"], "${turn_id}");
    for event in settings["hooks"].as_object().unwrap().values() {
        for group in event.as_array().unwrap() {
            let input = &group["hooks"][0]["input"];
            assert_eq!(input["session_id"], "${session_id}");
            assert_eq!(input["model"], "${model}");
            assert!(input.get("session_title").is_none(), "Codex does not promise a hook title");
        }
    }
    assert!(settings["hooks"].get("SessionEnd").is_none());
    let native: toml::Value = toml::from_str(&fs::read_to_string(directory.path().join(".codex/config.toml")).unwrap()).unwrap();
    for key in ["HECA_AGENT_REPORT", "HECA_AGENT_DRIVER", "HECA_AGENT_EVENTS"] {
        assert_eq!(native["mcp_servers"]["heca_status"]["env"][key].as_str(), Some(""));
    }
}

#[test]
fn event_selection_changes_reporting_without_rewriting_installed_hooks() {
    let mut config = enabled();
    let directory = tempfile::tempdir().unwrap();
    let installer = installed(directory.path(), &config);
    installer.apply("codex", super::super::integration::IntegrationOperation::Install).unwrap();
    let before = fs::read(directory.path().join(".codex/hooks.json")).unwrap();
    config.programs.get_mut("codex").unwrap().enabled_events = Some(vec!["UserPromptSubmit".into(), "Stop".into()]);
    let driver = codex::configured(&config).unwrap();
    let setup = driver.prepare(&context(directory.path())).unwrap();
    let events: serde_json::Value = serde_json::from_str(codex_connection(&setup)["env"]["HECA_AGENT_EVENTS"].as_str().unwrap()).unwrap();
    assert_eq!(events.as_object().unwrap().len(), 2);
    assert_eq!(events["UserPromptSubmit"]["status"], "working");
    assert_eq!(events["Stop"]["status"], "finished");
    assert_eq!(fs::read(directory.path().join(".codex/hooks.json")).unwrap(), before);
    config.programs.get_mut("codex").unwrap().enabled_events = Some(Vec::new());
    assert!(codex::configured(&config).is_none());
    assert!(available(&config).driver("codex").is_some(), "disabled reporting must not hide uninstall or inspect");
}

#[test]
fn builtin_launches_leave_no_temporary_integrations() {
    let directory = tempfile::tempdir().unwrap();
    for driver in configured(&enabled()).iter() {
        let setup = driver.prepare(&context(directory.path())).unwrap();
        assert!(!setup.environment.contains_key("OPENCODE_CLI_CONFIG_CONTENT"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn disabled_builtins_remain_available_for_the_full_installation_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let installer = installed(directory.path(), &AgentsConfig::default());
    let native_config = directory.path().join(".config/opencode/cli.json");
    fs::create_dir_all(native_config.parent().unwrap()).unwrap();
    let original = br#"{"plugins":["mine","-other",{"package":"custom","options":{"enabled":true}}]}"#;
    fs::write(&native_config, original).unwrap();
    let programs = installer.list().unwrap();
    assert_eq!(programs.len(), AgentsConfig::default().programs.len());
    for program in &programs {
        installer.apply(&program.driver, super::super::integration::IntegrationOperation::Install).unwrap();
        installer.apply(&program.driver, super::super::integration::IntegrationOperation::Update).unwrap();
        assert_eq!(installer.inspect(&program.driver).unwrap().state, super::super::integration::InstallationState::Installed);
    }
    for program in programs.iter().rev() {
        installer.apply(&program.driver, super::super::integration::IntegrationOperation::Uninstall).unwrap();
        assert_eq!(installer.inspect(&program.driver).unwrap().state, super::super::integration::InstallationState::NotInstalled);
    }
    assert_eq!(fs::read(native_config).unwrap(), original, "OpenCode plugin choices must stay byte-for-byte intact");
}

#[test]
fn codex_keeps_existing_inline_hooks_in_the_same_config_representation() {
    for hooks in [
        "[hooks]\nStop = [{hooks = [{type = 'command', command = 'mine'}]}]\n",
        "[[hooks.Stop]]\n[[hooks.Stop.hooks]]\ntype = 'command'\ncommand = 'mine'\n",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join(".codex");
        fs::create_dir(&native).unwrap();
        let original = format!("# my settings\nmodel = 'mine'\n{hooks}");
        fs::write(native.join("config.toml"), &original).unwrap();
        let installer = installed(directory.path(), &enabled());
        installer.apply("codex", super::super::integration::IntegrationOperation::Install).unwrap();
        assert!(!native.join("hooks.json").exists(), "do not create a competing hook source");
        let config = fs::read_to_string(native.join("config.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&config).unwrap();
        assert_eq!(parsed["hooks"]["Stop"].as_array().unwrap().len(), 2);
        installer.apply("codex", super::super::integration::IntegrationOperation::Install).unwrap();
        installer.apply("codex", super::super::integration::IntegrationOperation::Uninstall).unwrap();
        let restored = fs::read_to_string(native.join("config.toml")).unwrap();
        assert!(restored.contains("# my settings"));
        assert_eq!(toml::from_str::<toml::Value>(&restored).unwrap(), toml::from_str::<toml::Value>(&original).unwrap());
    }
}

#[test]
#[ignore = "requires installed Pi; uses an isolated configuration and makes no model request"]
fn native_pi_discovers_the_persistently_installed_extension() {
    use std::{process::Stdio, time::{Duration, Instant}};
    let directory = tempfile::tempdir().unwrap();
    let config = enabled();
    installed(directory.path(), &config).apply("pi", super::super::integration::IntegrationOperation::Install).unwrap();
    fs::write(directory.path().join("reports.jsonl"), []).unwrap();
    let setup = pi::configured(&config).unwrap().prepare(&context(directory.path())).unwrap();
    let mut child = Command::new("pi").args(["--mode", "json", "--no-session"])
        .env("PI_OFFLINE", "1").env("PI_CODING_AGENT_DIR", directory.path().join(".pi/agent"))
        .envs(&setup.environment).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn().expect("Pi must be installed for this manual check");
    let deadline = Instant::now() + Duration::from_secs(20);
    let report = loop {
        if let Ok(report) = fs::read_to_string(directory.path().join("reports.jsonl"))
            && !report.is_empty() { break Some(report) }
        if child.try_wait().unwrap().is_some() || Instant::now() >= deadline { break None }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    let report = report.unwrap_or_else(|| panic!("no Pi report: {}", String::from_utf8_lossy(&output.stderr)));
    let report: AgentState = serde_json::from_str(report.lines().next().unwrap()).unwrap();
    assert_eq!(report.status.as_str(), "waiting_input");
    assert!(report.session_id.is_some_and(|id| !id.is_empty()));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires installed OpenCode; starts an isolated TUI and stops after its first report"]
fn native_opencode_discovers_the_installed_cli_plugin() {
    use std::{os::unix::process::CommandExt, process::Stdio, time::{Duration, Instant}};
    let directory = tempfile::tempdir().unwrap();
    let config = enabled();
    installed(directory.path(), &config).apply("opencode", super::super::integration::IntegrationOperation::Install).unwrap();
    fs::write(directory.path().join("reports.jsonl"), []).unwrap();
    let setup = opencode::configured(&config).unwrap().prepare(&context(directory.path())).unwrap();
    let log = directory.path().join("native.log");
    let output = fs::File::create(&log).unwrap();
    let mut child = Command::new("/usr/bin/script")
        .args(["-q", "/dev/null", "opencode", "--standalone", "--prompt", "Reply with OK. Do not use tools or change files."])
        .current_dir(directory.path()).envs(&setup.environment)
        .env("OPENCODE_CONFIG_DIR", directory.path().join(".config/opencode"))
        .env("XDG_DATA_HOME", directory.path().join("data"))
        .env("XDG_STATE_HOME", directory.path().join("state"))
        .env("XDG_CACHE_HOME", directory.path().join("cache"))
        .stdin(Stdio::piped()).stdout(output.try_clone().unwrap()).stderr(output)
        .process_group(0).spawn().expect("OpenCode must be installed for this manual check");
    let deadline = Instant::now() + Duration::from_secs(30);
    let reports = loop {
        let reports: Vec<AgentState> = fs::read_to_string(directory.path().join("reports.jsonl"))
            .unwrap_or_default().lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
        if !reports.is_empty() { break reports }
        if child.try_wait().unwrap().is_some() || Instant::now() >= deadline { break reports }
        std::thread::sleep(Duration::from_millis(20));
    };
    // The private TUI/server share this process group. Never signal other running OpenCode instances.
    let _ = Command::new("/bin/kill").args(["-TERM", "--", &format!("-{}", child.id())]).status();
    let _ = child.wait();
    assert!(!reports.is_empty(), "installed plugin made no report: {}", fs::read_to_string(&log).unwrap());
    assert!(reports.iter().all(|report| report.driver == "opencode"));
    assert!(reports.iter().all(|report| report.session_id.as_ref().is_some_and(|id| !id.is_empty())));
}
