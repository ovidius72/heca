use super::*;
use crate::agent::tests::ExternalDriver;

#[test]
fn a_disabled_session_has_no_integration_and_enables_in_the_same_connection() {
    let mut options = AgentOptions {
        drivers: AgentDrivers::new(),
        max_report_bytes: 1024,
        channel_capacity: 8,
        launch_timeout: Duration::from_secs(5),
        launcher: std::env::current_exe().unwrap(),
        launched_status: String::new(),
    };
    let mut session = AgentSession::prepare(options.clone(), None).unwrap();
    let bin = session.bin();
    assert!(session.channel.is_none(), "no watcher runs before opt-in");
    assert_eq!(
        fs::read_dir(&bin).unwrap().count(),
        0,
        "no program is intercepted before opt-in"
    );
    options
        .drivers
        .register(ExternalDriver::new("demo"))
        .unwrap();
    session.configure(options).unwrap();
    assert_eq!(session.bin(), bin);
    assert!(session.channel.is_some());
    assert!(bin.join("demo-agent").is_file());
    session.disable().unwrap();
    assert!(session.channel.is_none());
    assert_eq!(fs::read_dir(&bin).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn the_wrapper_preserves_literal_user_arguments_for_the_private_launcher() {
    use std::os::unix::fs::PermissionsExt;
    let original = tempfile::tempdir().unwrap();
    let executable = original.path().join("demo-agent");
    fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let launcher = original.path().join("launcher");
    fs::write(&launcher, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n").unwrap();
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let session = AgentSession::prepare(
        AgentOptions {
            drivers,
            max_report_bytes: 1024,
            channel_capacity: 8,
            launch_timeout: Duration::from_secs(5),
            launcher,
            launched_status: String::new(),
        },
        None,
    )
    .unwrap();
    let output = std::process::Command::new(session.bin().join("demo-agent"))
        .env("HECA_AGENT_ORIGINAL_PATH", "/no-longer-the-current-path")
        .env(
            "PATH",
            std::env::join_paths([session.bin(), original.path().to_owned()]).unwrap(),
        )
        .args(["two words", "a'b", "$(exit 1)", "--existing-flag"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The launcher itself tells the terminal which program runs (OSC 133;E), for a typed command
    // and a program heca starts alike — the wrapper only hands it the words.
    assert!(text.starts_with("<--heca-agent-launch>\n"), "{text}");
    assert!(
        text.contains(&format!("<{}>", executable.display())),
        "resolve through the command's current PATH: {text}"
    );
    assert!(
        text.ends_with("<two words>\n<a'b>\n<$(exit 1)>\n<--existing-flag>\n"),
        "{text}"
    );
    let root = session.directory.path().to_path_buf();
    drop(session);
    assert!(
        !root.exists(),
        "temporary integration assets must be removed"
    );
}

/// **An agent heca starts itself goes through the launcher**, spelt exactly as the PATH wrapper
/// spells it — so a resumed agent reports like a typed one — while any other program starts as it
/// is.
#[test]
fn a_recognized_program_heca_starts_goes_through_the_launcher_like_a_typed_one() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let launcher = std::path::PathBuf::from("/opt/heca/heca");
    let session = AgentSession::prepare(
        AgentOptions {
            drivers,
            max_report_bytes: 1024,
            channel_capacity: 8,
            launch_timeout: Duration::from_secs(5),
            launcher: launcher.clone(),
            launched_status: String::new(),
        },
        None,
    )
    .unwrap();
    let agent = crate::backend::ProcessLaunch {
        executable: "/opt/tools/demo-agent".into(),
        arguments: vec!["--resume".into(), "abc".into()],
        cwd: Some("/tmp".into()),
    };
    let started = session
        .through_launcher(&agent)
        .expect("a driver recognizes it");
    assert_eq!(started.executable, launcher);
    assert_eq!(started.cwd, agent.cwd);
    let words: Vec<String> = started
        .arguments
        .iter()
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        &words[words.len() - 4..],
        ["demo-agent", "/opt/tools/demo-agent", "--resume", "abc"]
    );
    let wrapper = fs::read_to_string(session.bin().join("demo-agent")).unwrap();
    for word in &words[..words.len() - 3] {
        assert!(
            wrapper.contains(&super::quote(word)),
            "the wrapper spells {word} the same way"
        );
    }
    let other = crate::backend::ProcessLaunch {
        executable: "/bin/ls".into(),
        arguments: Vec::new(),
        cwd: None,
    };
    assert!(
        session.through_launcher(&other).is_none(),
        "not an agent: started as it is"
    );
}
