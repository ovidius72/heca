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
    let (dir, path) = server_environment(settings);
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

fn server_environment(settings: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let xdg = dir.path().join("config");
    let config = if cfg!(target_os = "macos") { home.join("Library/Application Support/heca") } else { xdg.join("heca") };
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), settings).unwrap();
    let path = dir.path().join("heca/default.sock");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    (dir, path)
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

fn cli_with_input(root: &Path, arguments: &[&str], input: &str) -> std::process::Output {
    let mut child = isolated_command(root).args(arguments).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn confirmed_termination(root: &Path, name: &str) -> String {
    let output = cli_with_input(root, &["--kill", name], "y\n");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn command_line_termination_requires_explicit_acceptance_and_honors_target_config() {
    let (root, _server, path) = start_server();
    let project = root.path().join("A");
    control(&path, json!({"Create":{"folder":project,"name":"A","label":null}}));
    control(&path, json!({"Create":{"folder":root.path().join("B"),"name":"B","label":null}}));
    for answer in ["", "\n", "n\n", "N\n", "no\n", "yes\n", "Terminate\n"] {
        let output = cli_with_input(root.path(), &["--kill", "A"], answer);
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        let prompt = String::from_utf8(output.stderr).unwrap();
        assert!(prompt.contains("session \"A\"") && prompt.contains("Confirm? [y/N]:"), "{prompt}");
        let sessions: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
        assert_eq!(sessions[0]["status"], "active", "{answer:?} must not terminate the session");
    }
    assert_eq!(confirmed_termination(root.path(), "A"), "ok\n");
    control(&path, json!({"Restart":{"name":"A"}}));
    std::fs::create_dir_all(project.join(".heca")).unwrap();
    std::fs::write(project.join(".heca/config.toml"), "[confirm]\nterminate_session=false\n").unwrap();
    let trusted = isolated_command(root.path()).current_dir(&project).arg("--trust").output().unwrap();
    assert!(trusted.status.success());
    let disabled = cli_with_input(root.path(), &["--kill", "A"], "");
    assert!(disabled.status.success(), "{}", String::from_utf8_lossy(&disabled.stderr));
    assert_eq!(disabled.stdout, b"ok\n");
    assert!(!String::from_utf8(disabled.stderr).unwrap().contains("Confirm? [y/N]:"));
    let sessions: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
    assert_eq!(sessions[0]["status"], "terminated");
    assert_eq!(sessions[1]["status"], "active");
}

#[test]
fn session_switch_projection_ends_after_its_terminal_frames_on_the_socket() {
    let (root, _server, path) = start_server();
    for name in ["A", "B"] {
        control(&path, json!({"Create":{"folder":root.path().join(name),"name":name,"label":null}}));
    }
    let (mut client, first) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let check = |client: &mut UnixStream, attached: &Value| {
        let pane = &attached["content"]["workspaces"][0]["columns"][0]["panes"][0]["id"];
        let mut snapshot = false;
        loop {
            let message = receive(client);
            if message["Terminal"]["owner"]["Pane"] == *pane { snapshot = true; }
            if let Some(ready) = message.get("ProjectionReady") {
                assert_eq!(ready["session_id"], attached["session"]["id"]);
                assert!(snapshot, "the renderer must have its pane snapshot before presentation resumes");
                break;
            }
        }
    };
    check(&mut client, &first["Attached"]);
    for name in ["B", "A", "B"] {
        send(&mut client, json!({"AttachTo":{"name":name}}));
        let attached = until(&mut client, "Attached");
        assert_eq!(attached["client_id"], first["Attached"]["client_id"]);
        assert_eq!(attached["session"]["name"], name);
        check(&mut client, &attached);
    }
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
    assert_eq!(confirmed_termination(root.path(), "A"), "ok\n");
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

#[test]
fn automatic_session_names_use_the_selected_projects_trusted_configuration() {
    let (root, _server, path) = start_server_with("/bin/sh",
        "[settings]\nshell_integration=false\nsession_name_format='root-{number}'\n");
    let project = root.path().join("project");
    std::fs::create_dir_all(project.join(".heca")).unwrap();
    std::fs::write(project.join(".heca/config.toml"),
        "[settings]\nsession_name_format='project-{number}'\n").unwrap();
    let trusted = isolated_command(root.path()).current_dir(&project).arg("--trust").output().unwrap();
    assert!(trusted.status.success(), "{}", String::from_utf8_lossy(&trusted.stderr));
    let created = control(&path, json!({"Create":{"folder":project,"name":null,"label":null}}));
    assert_eq!(created["Updated"]["name"], "project-1");
    assert_eq!(created["Updated"]["folder"], project.canonicalize().unwrap().to_str().unwrap());
    let other = control(&path, json!({"Create":{"folder":root.path().join("other"),"name":null,"label":null}}));
    assert_eq!(other["Updated"]["name"], "root-2", "a project's override must not leak into another session");
}

struct AutoStartedServer<'a>(&'a Path);
impl Drop for AutoStartedServer<'_> {
    fn drop(&mut self) {
        let _ = isolated_command(self.0).arg("--kill-server").output();
    }
}

#[track_caller]
fn wait_for_sessions(root: &Path, mut accept: impl FnMut(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let output = isolated_command(root).args(["--sessions", "--json"]).output().unwrap();
        if output.status.success()
            && let Ok(sessions) = serde_json::from_slice::<Value>(&output.stdout)
            && accept(&sessions) { return sessions; }
        assert!(Instant::now() < deadline, "session directory did not reach the expected state: {} {}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_window_exit(window: &mut Server) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = window.0.try_wait().unwrap() {
            assert!(status.success(), "window exited unsuccessfully: {status}");
            return;
        }
        assert!(Instant::now() < deadline, "window did not close");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn native_window(root: &Path, arguments: &[&str]) -> Server {
    Server(isolated_command(root).args(arguments).stdin(Stdio::null())
        .stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap())
}

#[test]
#[ignore = "opens a native window; run explicitly on a desktop"]
fn switching_a_native_session_does_not_move_its_shell_prompt() {
    check_native_prompt(|_| "PROMPT='%F{cyan}%~ on branch a-long-session-test-branch%f\n> '\n".into(), None);
}

#[test]
#[ignore = "opens a native window and requires Starship; run explicitly on a desktop"]
fn switching_a_starship_session_does_not_add_blank_prompt_rows() {
    let init = Command::new("starship").args(["init", "zsh"]).output().expect("Starship must be installed");
    assert!(init.status.success());
    let init = String::from_utf8(init.stdout).unwrap();
    check_native_prompt(move |root| {
        std::fs::write(root.join("home/starship.toml"),
            "add_newline = true\nformat = \"project with a long directory on branch session-switch-verification\\n$character\"\n[character]\nsuccess_symbol = '> '\n").unwrap();
        format!("export STARSHIP_CONFIG=$HOME/starship.toml\n{init}\n")
    }, Some(1));
}

fn check_native_prompt(zshrc: impl FnOnce(&Path) -> String, leading_rows: Option<usize>) {
    let (root, _server, path) = start_server_with("/bin/zsh", "[settings]\nshell_integration=true\nwindow_width=900\nwindow_height=900\n[font.size]\nterminal=13\nui=15\n[appearance.pane]\nborder_style='bracketed'\nborder_width=2\nborder_radius=4\n[appearance.terminal]\npadding=0\n");
    std::fs::write(root.path().join("home/.zshrc"), zshrc(root.path())).unwrap();
    for name in ["A", "B"] {
        control(&path, json!({"Create":{"folder":root.path().join(name),"name":name,"label":null}}));
    }
    let mut window = native_window(root.path(), &["--attach", "A"]);
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    std::thread::sleep(Duration::from_secs(2));
    let (mut observer, _) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let frame = until_frame(&mut observer, |frame| screen_lines(frame).iter().any(|line| !line.trim().is_empty()));
    send(&mut observer, json!({"Terminal":{"owner":frame["owner"],"input":{"Bytes":b"uno"}}}));
    let before = until_frame(&mut observer, |frame| screen_lines(frame).iter().any(|line| line.contains("uno")));
    if let Some(expected) = leading_rows {
        assert_eq!(screen_lines(&before).iter().position(|line| !line.trim().is_empty()), Some(expected),
            "initial attach added blank rows before the prompt");
    }
    send(&mut observer, json!("Detach"));
    drop(observer);
    for name in ["B", "A", "B", "A"] {
        cli(root.path(), &["rpc", "command_palette", "mode=session"]);
        let selected = format!("action=session:{}", if name == "A" { 1 } else { 2 });
        cli(root.path(), &["rpc", "submit_overlay", &selected]);
        wait_for_sessions(root.path(), |sessions| sessions[if name == "A" { 0 } else { 1 }]["windows"] == 1);
        std::thread::sleep(Duration::from_millis(200));
        if let Some(expected) = leading_rows {
            let (mut probe, _) = attach_named(&path, json!({"AttachTo":{"name":name}}));
            let frame = until_frame(&mut probe, |_| true);
            assert_eq!(screen_lines(&frame).iter().position(|line| !line.trim().is_empty()), Some(expected),
                "switching to {name} added blank rows");
            send(&mut probe, json!("Detach"));
        }
    }
    let (mut observer, _) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let after = until_frame(&mut observer, |_| true);
    let summary = |frame: &Value| json!({"cols":frame["screen"]["cols"],"rows":frame["screen"]["rows"],
        "cell":[frame["screen"]["cell_w"],frame["screen"]["cell_h"]],"cursor":frame["screen"]["cursor"],
        "text":screen_lines(frame).iter().enumerate().filter(|(_, line)| !line.trim().is_empty()).map(|(row,line)| (row,line.trim_end())).collect::<Vec<_>>()});
    assert_eq!(summary(&after), summary(&before));
    assert_eq!(screen_lines(&after), screen_lines(&before));
    send(&mut observer, json!("Detach"));
    drop(observer);
    cli(root.path(), &["rpc", "detach_session"]);
    wait_for_sessions(root.path(), |sessions| sessions.as_array().unwrap().iter().all(|session| session["windows"] == 0));
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut window);
}

#[test]
#[ignore = "opens native windows; run explicitly on a desktop"]
fn a_plain_window_starts_the_daemon_and_reattaches_then_follows_session_lifecycle() {
    let (root, path) = server_environment("[settings]\nshell_integration=false\nwindow_width=640\nwindow_height=280\nsession_name_format='native-{number}'\n");
    let _daemon = AutoStartedServer(root.path());
    assert!(!path.exists());
    let mut window = native_window(root.path(), &[]);
    let first = wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    assert_eq!(first[0]["name"], "native-1");
    assert_eq!(first[0]["folder"], root.path().join("home").canonicalize().unwrap().to_str().unwrap());
    let created = control(&path, json!({"Create":{"folder":root.path().join("B"),"name":"B","label":null}}));
    let duplicate = isolated_command(root.path()).args(["--new", "--name", "B"]).output().unwrap();
    assert_eq!(duplicate.status.code(), Some(1));
    let error = String::from_utf8(duplicate.stderr).unwrap();
    assert!(error.contains("a session already has that name"), "{error}");
    assert!(!error.contains("panicked"), "a rejected launch must return an error, not panic: {error}");
    assert_eq!(cli(root.path(), &["rpc", "command_palette", "mode=session"]), "ok\n");
    let selected = format!("action=session:{}", created["Updated"]["id"]);
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", &selected]), "ok\n");
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 0 && sessions[1]["windows"] == 1);
    assert_eq!(cli(root.path(), &["rpc", "detach_session"]), "ok\n");
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 0);
    assert!(window.0.try_wait().unwrap().is_none(), "logical detach leaves a usable window");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=new"]), "ok\n");
    assert_eq!(cli(root.path(), &["rpc", "close_overlay"]), "ok\n");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=select"]), "ok\n");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", &selected]), "ok\n");
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 1);
    cli(root.path(), &["rpc", "detach_session"]);
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 0);
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut window);
    let mut window = native_window(root.path(), &[]);
    let resumed = wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 1);
    assert_eq!(resumed.as_array().unwrap().len(), 2, "plain launch must not create another session");
    let id = resumed[1]["id"].clone();
    confirmed_termination(root.path(), "B");
    wait_for_sessions(root.path(), |sessions| sessions[1]["status"] == "terminated");
    assert!(window.0.try_wait().unwrap().is_none(), "termination must keep its window open");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=select"]), "ok\n");
    let first_session = format!("action=session:{}", resumed[0]["id"]);
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", &first_session]), "ok\n");
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1 && sessions[1]["windows"] == 0
        && sessions[1]["status"] == "terminated");
    assert_eq!(cli(root.path(), &["rpc", "switch_session", "name=B"]), "ok\n");
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 1 && sessions[1]["status"] == "terminated");
    cli(root.path(), &["rpc", "restart_session", "session=B"]);
    let restarted = wait_for_sessions(root.path(), |sessions| sessions[1]["status"] == "active");
    assert_eq!(restarted[1]["id"], id);
    assert_eq!(restarted[1]["windows"], 1);
    control(&path, json!({"Delete":{"name":"B"}}));
    let remaining = wait_for_sessions(root.path(), |sessions| sessions.as_array().is_some_and(|sessions| sessions.len() == 1));
    assert!(window.0.try_wait().unwrap().is_none(), "delete leaves the window available");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut window);
    assert_eq!(remaining[0]["name"], "native-1");
    assert!(root.path().join("B").is_dir(), "deletion must not remove the project folder");
}

#[test]
#[ignore = "opens native windows; run explicitly on a desktop"]
fn terminated_session_dialog_restarts_or_exits_without_affecting_another_session() {
    let (root, _server, path) = start_server();
    control(&path, json!({"Create":{"folder":root.path().join("A"),"name":"A","label":null}}));
    control(&path, json!({"Create":{"folder":root.path().join("B"),"name":"B","label":null}}));
    let mut other = native_window(root.path(), &["--attach", "A"]);
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    let mut first = native_window(root.path(), &["--attach", "B"]);
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 1);
    let mut second = native_window(root.path(), &["--attach", "B"]);
    let attached = wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 2);
    let original = attached[1]["id"].clone();

    confirmed_termination(root.path(), "B");
    let stopped = wait_for_sessions(root.path(), |sessions| sessions[1]["status"] == "terminated");
    assert_eq!(stopped[1]["windows"], 2);
    for window in [&mut first, &mut second, &mut other] {
        assert!(window.0.try_wait().unwrap().is_none());
    }
    // The same named action is submitted by the Dialog button and over RPC.
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=restart"]), "ok\n");
    let restarted = wait_for_sessions(root.path(), |sessions| sessions[1]["status"] == "active");
    assert_eq!(restarted[1]["id"], original);
    assert_eq!(restarted[1]["windows"], 2);
    // Close one of the test-owned windows; no untargeted RPC can choose among two clients.
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 1);

    confirmed_termination(root.path(), "B");
    wait_for_sessions(root.path(), |sessions| sessions[1]["status"] == "terminated");
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut second);
    let remaining = wait_for_sessions(root.path(), |sessions| sessions[1]["windows"] == 0);
    assert_eq!(remaining[1]["status"], "terminated", "Exit detaches; it does not restart or delete");
    assert_eq!(remaining[1]["id"], original);
    assert_eq!(remaining[0]["status"], "active");
    assert_eq!(remaining[0]["windows"], 1);
    assert!(other.0.try_wait().unwrap().is_none());
    control(&path, json!({"Delete":{"name":"A"}}));
    wait_for_sessions(root.path(), |sessions| sessions.as_array().is_some_and(|sessions| sessions.len() == 1));
    assert!(other.0.try_wait().unwrap().is_none());
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut other);
}

#[test]
#[ignore = "opens a native window; run explicitly on a desktop"]
fn session_action_confirmations_cancel_without_changes_and_proceed_only_when_chosen() {
    let (root, _server, path) = start_server();
    let a = root.path().join("A");
    control(&path, json!({"Create":{"folder":a,"name":"A","label":null}}));
    control(&path, json!({"Create":{"folder":root.path().join("B"),"name":"B","label":null}}));
    std::fs::write(a.join("keep.txt"), "project data").unwrap();
    let mut window = native_window(root.path(), &["--attach", "A"]);
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    for owner in ["terminate_session", "delete_session"] {
        let choose = format!("action={owner}");
        assert_eq!(cli(root.path(), &["rpc", "command_palette", "mode=command"]), "ok\n");
        assert_eq!(cli(root.path(), &["rpc", "submit_overlay", &choose]), "ok\n");
        let unchanged: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
        assert_eq!(unchanged[0]["status"], "active", "opening a confirmation must not run it");
        assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=cancel"]), "ok\n");
        let canceled: Value = serde_json::from_str(&cli(root.path(), &["--sessions", "--json"])).unwrap();
        assert_eq!(canceled, unchanged, "Cancel must leave the session and its window untouched");
        assert_eq!(cli(root.path(), &["rpc", "command_palette", "mode=command"]), "ok\n");
        assert_eq!(cli(root.path(), &["rpc", "submit_overlay", &choose]), "ok\n");
        assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=confirm"]), "ok\n");
        if owner == "terminate_session" {
            let stopped = wait_for_sessions(root.path(), |sessions| sessions[0]["status"] == "terminated");
            assert_eq!(stopped[0]["windows"], 1);
            assert_eq!(stopped[1]["status"], "active");
            assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=restart"]), "ok\n");
            wait_for_sessions(root.path(), |sessions| sessions[0]["status"] == "active");
        } else {
            let remaining = wait_for_sessions(root.path(), |sessions| sessions.as_array().is_some_and(|s| s.len() == 1));
            assert_eq!(remaining[0]["name"], "B");
        }
        assert_eq!(std::fs::read_to_string(a.join("keep.txt")).unwrap(), "project data");
        assert!(window.0.try_wait().unwrap().is_none());
    }
    assert_eq!(cli(root.path(), &["rpc", "submit_overlay", "action=exit"]), "ok\n");
    wait_for_window_exit(&mut window);
}

#[test]
#[ignore = "opens a native window; run explicitly on a desktop"]
fn losing_the_server_keeps_the_window_open_without_automatically_starting_another_daemon() {
    let (root, mut server, path) = start_server();
    control(&path, json!({"Create":{"folder":root.path().join("A"),"name":"A","label":null}}));
    let mut window = native_window(root.path(), &["--attach", "A"]);
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    assert_eq!(cli(root.path(), &["--kill-server"]).trim(), "heca: server stopped");
    let deadline = Instant::now() + Duration::from_secs(8);
    while server.0.try_wait().unwrap().is_none() || path.exists() {
        assert!(Instant::now() < deadline, "server did not shut down");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Let the event loop receive EOF; a disconnected window must not auto-start a daemon.
    std::thread::sleep(Duration::from_millis(
        heca_config::settings::ConnectionConfig::default().request_timeout_ms.get()));
    assert!(window.0.try_wait().unwrap().is_none());
    assert!(!path.exists());
    assert_eq!(cli(root.path(), &["--kill-server"]).trim(), "heca: no server is running");
}

#[test]
#[ignore = "opens a native window; run explicitly on a desktop"]
fn returning_to_a_session_types_into_its_previously_focused_pane() {
    let (root, _server, path) = start_server();
    for name in ["A", "B"] {
        control(&path, json!({"Create":{"folder":root.path().join(name),"name":name,"label":null}}));
    }
    let mut window = native_window(root.path(), &["--attach", "A"]);
    wait_for_sessions(root.path(), |sessions| sessions[0]["windows"] == 1);
    for _ in 0..2 { assert_eq!(cli(root.path(), &["rpc", "spawn_command", "command=/bin/sh"]), "ok\n"); }
    let (mut probe, attached) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    let panes: Vec<_> = attached["Attached"]["content"]["workspaces"][0]["columns"].as_array().unwrap()
        .iter().flat_map(|column| column["panes"].as_array().unwrap().iter().map(|pane| pane["id"].clone())).collect();
    assert_eq!(panes.len(), 3);
    let focused = panes.last().unwrap().clone();
    send(&mut probe, json!("Detach"));
    drop(probe);
    assert_eq!(cli(root.path(), &["rpc", "focus_pane", &format!("pane_id={focused}")]), "ok\n");
    for id in [2, 1] {
        cli(root.path(), &["rpc", "command_palette", "mode=session"]);
        cli(root.path(), &["rpc", "submit_overlay", &format!("action=session:{id}")]);
        wait_for_sessions(root.path(), |sessions| sessions[if id == 1 { 0 } else { 1 }]["windows"] == 1);
    }
    let (mut probe, _) = attach_named(&path, json!({"AttachTo":{"name":"A"}}));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(cli(root.path(), &["rpc", "terminal_run", "text=echo${IFS}__T522_RESTORED_FOCUS__"]), "ok\n");
    let frame = until_frame(&mut probe, |frame| screen_lines(frame).iter()
        .map(|line| line.trim_end()).collect::<String>().contains("__T522_RESTORED_FOCUS__"));
    assert_eq!(frame["owner"], json!({"Pane":focused}), "returning to A must restore its third pane, not its first");
    send(&mut probe, json!("Detach"));
    drop(probe);
    cli(root.path(), &["rpc", "detach_session"]);
    wait_for_sessions(root.path(), |sessions| sessions.as_array().unwrap().iter().all(|session| session["windows"] == 0));
    cli(root.path(), &["rpc", "submit_overlay", "action=exit"]);
    wait_for_window_exit(&mut window);
}
