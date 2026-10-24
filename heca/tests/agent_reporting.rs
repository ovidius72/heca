//! Exercise the shared reporting helper through the actual host executable.

use std::{collections::BTreeMap, fs, io::Write, process::{Child, Command, Stdio}};

use heca::agent::{AgentState, AgentStatus, ReportingHelper};
use serde_json::{Value, json};

fn helper(root: &std::path::Path, name: &str, active: bool) -> Child {
    let endpoint = root.join(name);
    fs::write(&endpoint, []).unwrap();
    let report = AgentState::new("example", "Example program", AgentStatus::new("finished"));
    let reports: BTreeMap<String, AgentState> = [("Stop".into(), report.clone())].into();
    let helper = ReportingHelper {
        executable: env!("CARGO_BIN_EXE_heca").into(),
        max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes,
    }.mcp_server(active.then_some(endpoint.as_path()), &reports).unwrap();
    // A shared native server may already have these stale values. The connection must override
    // every reporting value, including when the installed helper runs outside Heca.
    let mut command = Command::new(helper["command"].as_str().unwrap());
    command.args(helper["args"].as_array().unwrap().iter().map(|value| value.as_str().unwrap()))
        .env("HECA_AGENT_REPORT", &endpoint).env("HECA_AGENT_DRIVER", "example")
        .env("HECA_AGENT_EVENTS", json!({"Stop":report}).to_string());
    for (key, value) in helper["env"].as_object().unwrap() { command.env(key, value.as_str().unwrap()); }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap()
}

fn finish(mut child: Child, turn: &str) {
    let mut input = child.stdin.take().unwrap();
    for request in [json!({"jsonrpc":"2.0", "id":1, "method":"initialize"}),
        json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0", "id":2, "method":"tools/call", "params":{
            "name":ReportingHelper::TOOL, "arguments":{"event":"Stop", "turn_id":turn,
                "model":format!("model-{turn}"), "session_id":format!("session-{turn}")}}})] {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let replies: Vec<Value> = String::from_utf8(output.stdout).unwrap().lines()
        .map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(replies.len(), 2);
    assert!(replies[1]["result"]["isError"].is_null());
}

#[test]
fn concurrent_helpers_override_stale_server_environment_and_keep_reports_separate() {
    let root = tempfile::tempdir().unwrap();
    let first = helper(root.path(), "first", true);
    let second = helper(root.path(), "second", true);
    let outside = helper(root.path(), "outside", false);
    finish(second, "turn-b");
    finish(first, "turn-a");
    finish(outside, "outside");
    for (name, turn) in [("first", "turn-a"), ("second", "turn-b")] {
        let report: AgentState = serde_json::from_slice(&fs::read(root.path().join(name)).unwrap()).unwrap();
        assert_eq!(report.turn_id.as_deref(), Some(turn));
        assert_eq!(report.model, Some(format!("model-{turn}")));
        assert_eq!(report.session_id, Some(format!("session-{turn}")));
    }
    assert!(fs::read(root.path().join("outside")).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn installed_command_hooks_forward_native_metadata_through_the_actual_host() {
    use heca::agent::{IntegrationContext, integrations_in, integration::IntegrationOperation};

    let root = tempfile::tempdir().unwrap();
    let endpoint = root.path().join("reports.jsonl");
    fs::write(&endpoint, []).unwrap();
    // Exercise spaces and apostrophes through the exact command installed in native settings.
    let executable = root.path().join("Heca's reporting host");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_heca"), &executable).unwrap();
    let config = heca_config::settings::AgentsConfig::default();
    let reporter = ReportingHelper { executable, max_report_bytes: config.max_report_bytes };
    let installer = integrations_in(&config, IntegrationContext {
        reporter, home: root.path().to_owned(), directory: root.path().join("receipts"), environment: Default::default(),
    }, root.path().join("heca.toml"));
    installer.apply("claude", IntegrationOperation::Install).unwrap();
    let settings: Value = serde_json::from_slice(&fs::read(root.path().join(".claude/settings.json")).unwrap()).unwrap();
    let hook = settings["hooks"]["Stop"][0]["hooks"][0]["command"].as_str().unwrap();
    let report = AgentState::new("claude", "Claude Code", AgentStatus::new("finished"));
    let reports = json!({"Stop":report}).to_string();
    let invoke = |driver: &str, events: &str, payload: &[u8]| {
        let mut child = Command::new("/bin/sh").args(["-c", hook]).env_clear()
            .env("HECA_AGENT_DRIVER", driver).env("HECA_AGENT_EVENTS", events).env("HECA_AGENT_REPORT", &endpoint)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(payload).unwrap();
        child.wait_with_output().unwrap()
    };
    let native = json!({"model":"claude-model", "session_id":"native-session", "session_title":"Native title",
        "prompt":"do not retain", "permission_mode":"auto", "transcript_path":"/must/not/read"}).to_string();
    let output = invoke("claude", &reports, native.as_bytes());
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty(), "reporting must not supply native hook decisions");
    let bytes = fs::read(&endpoint).unwrap();
    let state: AgentState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state.model.as_deref(), Some("claude-model"));
    assert_eq!(state.session_id.as_deref(), Some("native-session"));
    assert_eq!(state.session_title.as_deref(), Some("Native title"));
    assert!(!String::from_utf8_lossy(&bytes).contains("do not retain"));
    for (driver, events) in [("", "invalid"), ("other", &reports), ("claude", "{}"),
        ("claude", r#"{"Stop":{"driver":"other","name":"Other","status":"working","task":null,"detail":null,"turn_id":null}}"#)] {
        let output = invoke(driver, events, b"not-json");
        assert!(output.status.success(), "inactive or foreign hooks must not read native input");
    }
    assert_eq!(fs::read(&endpoint).unwrap(), bytes);
    assert!(!invoke("claude", &reports, br#"{"session_id":7}"#).status.success());
    assert_eq!(fs::read(&endpoint).unwrap(), bytes);
    fs::remove_file(&endpoint).unwrap();
    assert!(invoke("claude", &reports, b"{}").status.success());
    assert!(!endpoint.exists(), "closing a pane must not recreate its reporting file");
}
