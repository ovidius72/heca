//! Real binary, real socket and real PTY: the session must survive client teardown.
#![cfg(unix)]

use std::{io::{Read, Write}, os::unix::net::UnixStream, path::Path,
    process::{Child, Command, Stdio}, time::{Duration, Instant}};
use serde_json::{Value, json};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}

fn send(stream: &mut UnixStream, value: Value) {
    let data = serde_json::to_vec(&value).unwrap();
    stream.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
    stream.write_all(&data).unwrap();
}

fn receive(stream: &mut UnixStream) -> Value {
    let mut header = [0; 4];
    stream.read_exact(&mut header).unwrap();
    let length = u32::from_be_bytes(header) as usize;
    assert!(length <= heca_config::settings::ConnectionConfig::default().max_frame_bytes.get());
    let mut data = vec![0; length];
    stream.read_exact(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}

fn attach(path: &Path) -> (UnixStream, Value) {
    attach_named(path, json!("Attach"))
}

fn attach_named(path: &Path, request: Value) -> (UnixStream, Value) {
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(b"@heca-attach 1\n").unwrap();
    send(&mut stream, request);
    let attached = receive(&mut stream);
    assert!(attached.get("Attached").is_some(), "{attached}");
    (stream, attached)
}

fn screen_lines(frame: &Value) -> Vec<String> {
    frame["screen"]["lines"].as_array().unwrap().iter().map(|line| {
        line["cells"].as_array().unwrap().iter().filter_map(|cell| cell["text"].as_str()).collect()
    }).collect()
}

fn until_frame(stream: &mut UnixStream, mut accept: impl FnMut(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "terminal did not reach the expected state");
        let message = receive(stream);
        if let Some(frame) = message.get("Terminal")
            && accept(frame) { return frame.clone(); }
    }
}

fn isolated_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_heca"));
    command.env("HOME", root.join("home")).env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_RUNTIME_DIR", root).env("TMPDIR", root).env("SHELL", "/bin/sh").current_dir(root);
    command
}

fn start_server() -> (tempfile::TempDir, Server, std::path::PathBuf) {
    start_server_with("/bin/sh", "[settings]\nshell_integration = false\nwindow_width = 640\nwindow_height = 280\n")
}

fn start_server_with(shell: &str, settings: &str) -> (tempfile::TempDir, Server, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let xdg = dir.path().join("config");
    let config = if cfg!(target_os = "macos") { home.join("Library/Application Support/heca") } else { xdg.join("heca") };
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), settings).unwrap();
    let path = dir.path().join("heca/default.sock");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut server = Server(isolated_command(dir.path()).env("SHELL", shell).arg("--server").arg(&path)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(server.0.try_wait().unwrap().is_none(), "headless server exited at startup");
        assert!(Instant::now() < deadline, "headless server did not bind");
        if UnixStream::connect(&path).is_ok() { break; }
        std::thread::sleep(Duration::from_millis(5));
    }
    (dir, server, path)
}

#[test]
fn a_real_headless_session_keeps_its_process_and_history_after_detach() {
    let (_dir, mut server, path) = start_server();
    // The text RPC protocol and framed attach protocol share this exact listener.
    let mut rpc = UnixStream::connect(&path).unwrap();
    rpc.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    rpc.write_all(b"describe-action split_horizontal\n").unwrap();
    let mut answer = String::new();
    rpc.read_to_string(&mut answer).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&answer).unwrap()["name"], "split_horizontal");

    let (mut client, attached) = attach(&path);
    let pane = attached["Attached"]["content"]["workspaces"][0]["columns"][0]["panes"][0]["id"].clone();
    let owner = json!({"Pane": pane});
    // Split the printed markers so terminal echo cannot satisfy these assertions.
    let command = "printf '__T544_%s__%s\\n' PID \"$$\"; sleep 1; i=0; while [ \"$i\" -lt 90 ]; do printf '__T544_ROW_%s__\\n' \"$i\"; i=$((i+1)); done; sleep 1; printf '__T544_%s__\\n' FINISH\r";
    send(&mut client, json!({"Terminal":{"owner":owner,"input":{"Bytes":command.as_bytes()}}}));
    let first = until_frame(&mut client, |frame| screen_lines(frame).iter().any(|line| line.trim().starts_with("__T544_PID__")));
    let process = screen_lines(&first).into_iter().find_map(|line| line.trim().strip_prefix("__T544_PID__").map(|s| s.trim().to_owned())).unwrap();
    assert!(process.parse::<u32>().is_ok(), "not a process id: {process}");
    let terminal = first["id"].clone();
    send(&mut client, json!("Detach"));
    drop(client);
    assert!(server.0.try_wait().unwrap().is_none(), "detach ended the server");

    let (mut client, again) = attach(&path);
    assert_eq!(again["Attached"]["content"], attached["Attached"]["content"]);
    let finished = until_frame(&mut client, |frame| screen_lines(frame).iter().any(|line| line.trim() == "__T544_FINISH__"));
    assert_eq!(finished["id"], terminal, "reattach started another terminal");
    assert!(finished["screen"]["scrollback_rows"].as_u64().unwrap() > finished["screen"]["rows"].as_u64().unwrap());
    send(&mut client, json!({"Viewport":{"owner":owner,"offset":usize::MAX}}));
    let history = until_frame(&mut client, |frame| frame["screen"]["viewport_offset"].as_u64().unwrap() > 0);
    assert!(screen_lines(&history).iter().any(|line| line.contains(&format!("__T544_PID__{process}"))), "original process output was lost from history");
    assert!(server.0.try_wait().unwrap().is_none());
}

fn cli(root: &Path, arguments: &[&str]) -> String {
    let output = isolated_command(root).args(arguments).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

fn until(stream: &mut UnixStream, variant: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "server did not send {variant}");
        let message = receive(stream);
        if let Some(value) = message.get(variant) { return value.clone(); }
    }
}

fn control(path: &Path, command: Value) -> Value {
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(b"@heca-attach 1\n").unwrap();
    send(&mut stream, json!({"Session":{"request_id":1,"command":command}}));
    let reply = receive(&mut stream);
    assert_eq!(reply["Sessions"]["request_id"], 1);
    assert!(reply["Sessions"]["result"].get("Ok").is_some(), "{reply}");
    reply["Sessions"]["result"]["Ok"].clone()
}

#[test]
fn named_sessions_switch_terminate_restart_and_delete_over_the_real_socket() {
    let (root, _server, path) = start_server();
    assert_eq!(serde_json::from_str::<Value>(&cli(root.path(), &["--sessions", "--json"])).unwrap(), json!([]));
    let a_folder = root.path().join("A");
    let b_folder = root.path().join("B");
    control(&path, json!({"Create":{"folder":a_folder,"name":"A","label":"extension-owned"}}));
    control(&path, json!({"Create":{"folder":b_folder,"name":"B","label":null}}));
    std::fs::write(a_folder.join("keep.txt"), "project data").unwrap();
    let (mut a, attached) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let (mut mirror, _) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let (mut b, _) = attach_named(&path, json!({"AttachTo":{"name":"B"}}));
    let original = attached["Attached"]["session"]["id"].clone();
    let list: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
    assert_eq!(list[0]["windows"], 2);
    assert_eq!(list[1]["windows"], 1);
    assert_eq!(list[0]["label"], "extension-owned");
    let rpc_list: Value = serde_json::from_str(&cli(root.path(), &["rpc", "list_sessions"])).unwrap();
    assert_eq!(rpc_list, list, "RPC queries must not be forwarded to a client window");
    assert_eq!(cli(root.path(), &["rpc", "rename_session_to", "session=B", "name=B-renamed"]), "ok\n");
    assert_eq!(cli(root.path(), &["rpc", "rename_session_to", "session=B-renamed", "name=B"]), "ok\n");
    let command = "printf '__REPLAY_%s__\\n' 'two words'; sleep 10";
    send(&mut a, json!({"EnsureTerminal":{"owner":{"Named":"test.original"},
        "spec":{"program":{"Command":command},"cwd":a_folder,"grid":[80,24]},"cell":[8.0,14.0]}}));
    let original_process = until_frame(&mut a, |frame| frame["owner"]["Named"] == "test.original"
        && screen_lines(frame).iter().any(|line| line.contains("__REPLAY_two words__")));
    assert_eq!(cli(root.path(), &["--kill", "A"]), "ok\n");
    assert_eq!(until(&mut a, "SessionTerminated")["session"]["status"], "terminated");
    assert_eq!(until(&mut mirror, "SessionTerminated")["session"]["id"], original);
    control(&path, json!({"Restart":{"name":"A"}}));
    assert_eq!(until(&mut a, "Attached")["session"]["id"], original);
    assert_eq!(until(&mut mirror, "Attached")["session"]["status"], "active");
    let restarted_process = until_frame(&mut a, |frame| frame["owner"]["Named"] == "test.original"
        && screen_lines(frame).iter().any(|line| line.contains("__REPLAY_two words__")));
    assert_ne!(restarted_process["id"], original_process["id"], "restart must launch a new process from the original command");
    send(&mut a, json!({"AttachTo":{"name":"B"}}));
    assert_eq!(until(&mut a, "Attached")["session"]["name"], "B");
    control(&path, json!({"Delete":{"name":"A"}}));
    assert_eq!(until(&mut mirror, "SessionDeleted")["id"], original);
    send(&mut mirror, json!("Detach"));
    let list: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "B");
    assert_eq!(list[0]["windows"], 2);
    assert_eq!(std::fs::read_to_string(a_folder.join("keep.txt")).unwrap(), "project data");
    send(&mut b, json!({"AttachTo":{"name":"B"}}));
    assert_eq!(until(&mut b, "Attached")["session"]["name"], "B");
}

fn until_feedback(stream: &mut UnixStream, mut accept: impl FnMut(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(Instant::now() < deadline, "feedback did not reach the expected revision");
        let message = receive(stream);
        if let Some(feedback) = message.get("Feedback")
            && accept(feedback) { return feedback.clone(); }
    }
}

#[test]
fn real_shell_completions_and_acknowledgements_are_shared_by_all_clients() {
    let (_root, _server, path) = start_server_with("/bin/bash", "[settings]\nshell_integration = true\n");
    let (mut first, attached) = attach(&path);
    let session = attached["Attached"]["session"]["id"].clone();
    let pane = attached["Attached"]["content"]["workspaces"][0]["columns"][0]["panes"][0]["id"].clone();
    let input = |command: &str| json!({"Terminal": {
        "owner": {"Pane": pane}, "input": {"Bytes": command.as_bytes()}
    }});
    // The split marker cannot be satisfied by the terminal's input echo.
    send(&mut first, input("printf '__FEEDBACK_%s__\\n' READY\r"));
    until_frame(&mut first, |frame| screen_lines(frame).iter().any(|line| line.trim() == "__FEEDBACK_READY__"));
    let (mut second, _) = attach(&path);
    let baseline = until(&mut second, "Feedback")["panes"].as_array().unwrap().iter()
        .filter_map(|pane| pane["feedback_revision"].as_u64()).max().unwrap_or(0);
    send(&mut first, input("sleep 0.05; false\r"));
    let latest = until_feedback(&mut first, |feedback| feedback["panes"].as_array().unwrap().iter()
        .any(|pane| pane["feedback_revision"].as_u64().unwrap() > baseline && pane["status"] == "error"));
    let event = latest["panes"].as_array().unwrap().iter().find(|item| item["pane_id"] == pane).unwrap();
    let revision = event["feedback_revision"].as_u64().unwrap();
    let mirrored = until_feedback(&mut second, |feedback| feedback["panes"].as_array().unwrap().iter()
        .any(|pane| pane["feedback_revision"] == revision));
    assert_eq!(mirrored, latest);
    send(&mut first, json!({"RequestFeedbackSound": {
        "session_id": session, "pane_id": pane, "revision": revision
    }}));
    assert_eq!(until(&mut first, "FeedbackSound")["revision"], revision);
    send(&mut second, json!({"RequestFeedbackSound": {
        "session_id": session, "pane_id": pane, "revision": revision
    }}));
    send(&mut second, json!({"Session": {"request_id": 55, "command": "List"}}));
    loop {
        let message = receive(&mut second);
        assert!(message.get("FeedbackSound").is_none(), "another window must not repeat the sound");
        if message["Sessions"]["request_id"] == 55 { break; }
    }
    send(&mut first, json!({"AcknowledgeFeedback": {
        "session_id": session, "pane_id": pane, "revision": revision
    }}));
    for client in [&mut first, &mut second] {
        let seen = until_feedback(client, |feedback| feedback["panes"].as_array().unwrap().iter()
            .any(|pane| pane["seen_through_revision"] == revision));
        assert_eq!(seen["panes"][0]["feedback_revision"], revision);
    }
    let (mut third, _) = attach(&path);
    assert_eq!(until(&mut third, "Feedback")["panes"][0]["seen_through_revision"], revision);
    send(&mut first, input("false\r"));
    let repeated = until_feedback(&mut first, |feedback| feedback["panes"][0]["feedback_revision"].as_u64().unwrap() > revision);
    assert_eq!(repeated["panes"][0]["status"], "error");
}
