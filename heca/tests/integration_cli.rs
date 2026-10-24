//! The installed-program CLI exercises the shared installer through the real host, without a GUI.

use std::{fs, path::Path, process::{Command, Output}};

use serde_json::{Value, json};

fn invoke(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_heca")).args(args).current_dir(root)
        .env("HOME", root).env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data")).env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("CLAUDE_CONFIG_DIR", root.join("native/claude"))
        .env("CODEX_HOME", root.join("native/codex"))
        .env("PI_CODING_AGENT_DIR", root.join("native/pi"))
        .env("OPENCODE_CONFIG_DIR", root.join("native/opencode"))
        .output().unwrap()
}

fn success(root: &Path, args: &[&str]) -> Output {
    let output = invoke(root, args);
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    output
}

fn result(root: &Path, args: &[&str]) -> Value {
    serde_json::from_slice(&success(root, args).stdout).unwrap()
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join(".config/heca/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(config, "[settings.agents.programs.claude]\nname = 'Isolated Claude'\n").unwrap();
    // Prove that normal config discovery points at this isolated home before any write command.
    let statuses = result(root.path(), &["list", "integrations", "--json"]);
    let claude = statuses.as_array().unwrap().iter().find(|status| status["driver"] == "claude").unwrap();
    assert_eq!(claude["name"], "Isolated Claude");
    assert!(statuses.as_array().unwrap().iter().all(|status| status["state"] == "NotInstalled"));
    root
}

#[test]
fn the_cli_lists_installs_inspects_updates_and_removes_each_registered_bundle() {
    let root = fixture();
    let available = result(root.path(), &["--json", "list", "integrations"]);
    for program in available.as_array().unwrap() {
        let id = program["driver"].as_str().unwrap();
        let installed = result(root.path(), &["install", "integration", id, "--json"]);
        assert_eq!(installed["state"], "Installed");
        assert_eq!(installed["driver"], id);
        assert_eq!(installed["installed_version"], installed["bundled_version"]);
        let inspected = result(root.path(), &["inspect", "integration", "--json", id]);
        assert_eq!(inspected, installed);
        assert_eq!(result(root.path(), &["install", "integration", id, "-j"]), installed);
        assert_eq!(result(root.path(), &["update", "integration", id, "--json"]), installed);
        let removed = result(root.path(), &["uninstall", "integration", id, "--json"]);
        assert_eq!(removed["state"], "NotInstalled");
        assert!(removed["installed_version"].is_null());
    }
}

#[test]
fn command_output_preserves_native_settings_and_disabled_reporting_during_update() {
    let root = fixture();
    let settings = root.path().join("native/claude/settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let original = json!({"permissions":{"defaultMode":"default"}, "hooks":{"Stop":[{
        "hooks":[{"type":"command", "command":"printf 'user hook'"}],
    }]}});
    fs::write(&settings, original.to_string()).unwrap();
    let output = success(root.path(), &["install", "integration", "claude"]);
    assert!(String::from_utf8(output.stdout).unwrap().contains("Isolated Claude (claude): installed"));
    let installed: Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
    assert_eq!(installed["permissions"], original["permissions"]);
    assert_eq!(installed["hooks"]["Stop"][0], original["hooks"]["Stop"][0]);
    let config = root.path().join(".config/heca/config.toml");
    let mut document: toml_edit::DocumentMut = fs::read_to_string(&config).unwrap().parse().unwrap();
    document["settings"]["agents"]["programs"]["claude"]["enabled"] = toml_edit::value(false);
    fs::write(&config, document.to_string()).unwrap();
    success(root.path(), &["update", "integration", "claude"]);
    let document: toml::Value = toml::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(document["settings"]["agents"]["programs"]["claude"]["enabled"].as_bool(), Some(false));
    success(root.path(), &["uninstall", "integration", "claude"]);
    assert_eq!(serde_json::from_slice::<Value>(&fs::read(&settings).unwrap()).unwrap(), original);
}

#[test]
fn malformed_commands_unknown_programs_and_invalid_settings_fail_without_a_window() {
    let root = fixture();
    for args in [&["install", "integration"][..], &["install", "integration", "missing", "--json"],
        &["install", "integration", "claude", "extra"], &["list", "integrations", "--typo"],
        &["instal", "integration", "claude"], &["--json"]] {
        let output = invoke(root.path(), args);
        assert!(!output.status.success(), "{args:?}");
        assert!(output.stdout.is_empty(), "failure must not masquerade as a JSON status");
        assert!(!output.stderr.is_empty());
    }
    fs::write(root.path().join(".config/heca/config.toml"), "invalid = [").unwrap();
    let output = invoke(root.path(), &["list", "integrations", "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("integration settings"));
    assert!(!root.path().join("native").exists());
}

#[test]
fn help_comes_from_the_command_declarations() {
    let root = fixture();
    let help = String::from_utf8(success(root.path(), &["--help"]).stdout).unwrap();
    for command in ["install integration", "update integration", "uninstall integration", "inspect integration", "list integrations"] {
        assert!(help.contains(command), "missing {command}");
    }
    let help = String::from_utf8(success(root.path(), &["install", "integration", "--help"]).stdout).unwrap();
    assert!(help.contains("Usage: heca install integration <program> [--json]"));
    assert!(!root.path().join("native").exists());
}
