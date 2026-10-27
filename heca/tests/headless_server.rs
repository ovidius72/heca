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
    let mut stream = UnixStream::connect(path).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(b"@heca-attach 1\n").unwrap();
    send(&mut stream, json!("Attach"));
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

#[test]
fn a_real_headless_session_keeps_its_process_and_history_after_detach() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let xdg = dir.path().join("config");
    let config = if cfg!(target_os = "macos") { home.join("Library/Application Support/heca") } else { xdg.join("heca") };
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), "[settings]\nshell_integration = false\nwindow_width = 640\nwindow_height = 280\n").unwrap();
    let path = dir.path().join("server.sock");
    let mut server = Server(Command::new(env!("CARGO_BIN_EXE_heca"))
        .arg("--server").arg(&path).env("HOME", &home).env("XDG_CONFIG_HOME", &xdg)
        .env("SHELL", "/bin/sh").current_dir(dir.path())
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(8);
    while !path.exists() {
        assert!(server.0.try_wait().unwrap().is_none(), "headless server exited at startup");
        assert!(Instant::now() < deadline, "headless server did not bind");
        std::thread::sleep(Duration::from_millis(5));
    }
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
