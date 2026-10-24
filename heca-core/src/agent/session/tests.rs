use super::*;
use crate::agent::tests::ExternalDriver;

#[test]
fn a_disabled_session_has_no_integration_and_enables_in_the_same_connection() {
    let mut options = AgentOptions { drivers: AgentDrivers::new(), max_report_bytes: 1024,
        channel_capacity: 8, launch_timeout: Duration::from_secs(5), launcher: std::env::current_exe().unwrap() };
    let mut session = AgentSession::prepare(options.clone(), None).unwrap();
    let bin = session.bin();
    assert!(session.channel.is_none(), "no watcher runs before opt-in");
    assert_eq!(fs::read_dir(&bin).unwrap().count(), 0, "no program is intercepted before opt-in");
    options.drivers.register(ExternalDriver::new("demo")).unwrap();
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
    let session = AgentSession::prepare(AgentOptions {
        drivers, max_report_bytes: 1024, channel_capacity: 8,
        launch_timeout: Duration::from_secs(5), launcher,
    }, None).unwrap();
    let output = std::process::Command::new(session.bin().join("demo-agent"))
        .env("HECA_AGENT_ORIGINAL_PATH", "/no-longer-the-current-path")
        .env("PATH", std::env::join_paths([session.bin(), original.path().to_owned()]).unwrap())
        .args(["two words", "a'b", "$(exit 1)", "--existing-flag"])
        .output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(text.starts_with("\x1b]133;E;demo-agent\x07<--heca-agent-launch>\n"), "{text}");
    assert!(text.contains(&format!("<{}>", executable.display())), "resolve through the command's current PATH: {text}");
    assert!(text.ends_with("<two words>\n<a'b>\n<$(exit 1)>\n<--existing-flag>\n"), "{text}");
    let root = session.directory.path().to_path_buf();
    drop(session);
    assert!(!root.exists(), "temporary integration assets must be removed");
}
