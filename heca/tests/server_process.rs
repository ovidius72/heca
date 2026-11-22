//! The real binary, on every platform: `heca --server`, `heca rpc`, `heca --kill-server`.

use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `heca` with its runtime and temp folders inside `root`, so it finds only the server of this test.
fn heca(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_heca"));
    command
        .env("XDG_RUNTIME_DIR", root)
        .env("TMPDIR", root)
        .env("TMP", root)
        .env("TEMP", root)
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .current_dir(root);
    command
}

fn output(command: &mut Command) -> String {
    String::from_utf8_lossy(&command.stdin(Stdio::null()).output().unwrap().stdout).into_owned()
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_script_reaches_the_server_and_kill_server_ends_it() {
    let root = tempfile::tempdir().unwrap();
    let nobody = output(heca(root.path()).args(["rpc", "describe-action", "split_horizontal"]));
    assert!(
        nobody.contains("could not reach a running heca"),
        "got: {nobody}"
    );

    let mut server = Server(
        heca(root.path())
            .arg("--server")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let mut answer = String::new();
    until("the server did not answer", || {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "the server exited at startup"
        );
        answer = output(heca(root.path()).args(["rpc", "describe-action", "split_horizontal"]));
        !answer.contains("could not reach")
    });
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&answer).unwrap()["name"],
        "split_horizontal",
        "got: {answer}"
    );

    // The old raw line is gone: it is an unknown command now, and the server stays up.
    assert!(output(heca(root.path()).args(["rpc", "kill-server"])).contains("unknown command"));
    assert!(
        server.0.try_wait().unwrap().is_none(),
        "the old line stopped the server"
    );

    // `heca rpc kill_server` and `heca --kill-server` are the same action.
    assert!(output(heca(root.path()).arg("--kill-server")).contains("server stopped"));
    until("the server did not end", || {
        server.0.try_wait().unwrap().is_some()
    });
    let gone = output(heca(root.path()).args(["rpc", "describe-action", "split_horizontal"]));
    assert!(
        gone.contains("could not reach a running heca"),
        "got: {gone}"
    );
}

/// A server that is alive keeps its socket: a second one fails to start and the first still answers.
#[test]
fn a_second_server_does_not_take_the_socket_of_a_live_one() {
    let root = tempfile::tempdir().unwrap();
    let first = Server(
        heca(root.path())
            .arg("--server")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    until("the first server did not answer", || {
        !output(heca(root.path()).args(["rpc", "describe-action", "split_horizontal"]))
            .contains("could not reach")
    });
    let second = heca(root.path())
        .arg("--server")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!second.status.success(), "a second server started");
    assert!(
        !output(heca(root.path()).args(["rpc", "describe-action", "split_horizontal"]))
            .contains("could not reach")
    );
    drop(first);
}

/// **A command that needs the server's sessions starts the server when none runs** — and the sessions
/// come back with it. `--attach` asks the server whether the session exists before it opens a window,
/// so after `--kill-server` it must start the background server rather than fail to reach it.
#[cfg(unix)]
#[test]
fn listing_and_attaching_start_the_server_and_find_the_restored_session() {
    /// Stops the server the commands below start, whatever the test does.
    struct Daemon<'a>(&'a Path);
    impl Drop for Daemon<'_> {
        fn drop(&mut self) {
            let _ = heca(self.0)
                .arg("--kill-server")
                .stdin(Stdio::null())
                .output();
        }
    }
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let _daemon = Daemon(root.path());
    let mut first = Server(
        heca(root.path())
            .arg("--server")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    until("the server did not answer", || {
        output(heca(root.path()).args([
            "rpc",
            "create_session",
            &format!("folder={}", project.display()),
            "name=A",
        ]))
        .trim()
            == "ok"
    });
    assert!(output(heca(root.path()).arg("--kill-server")).contains("server stopped"));
    until("the server did not end", || {
        first.0.try_wait().unwrap().is_some()
    });
    assert!(
        output(heca(root.path()).args(["rpc", "list_sessions"])).contains("could not reach"),
        "no server runs"
    );

    // `--kill` acts on a running server: with none, it fails and starts nothing.
    let kill = heca(root.path())
        .args(["--kill", "A"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&kill.stderr).contains("could not reach")
            || String::from_utf8_lossy(&kill.stdout).contains("could not reach")
    );
    assert!(
        output(heca(root.path()).args(["rpc", "list_sessions"])).contains("could not reach"),
        "--kill started a server"
    );

    // `--attach` to a session that is not there says so — it reached a server it had to start, and
    // opened no window.
    let missing = heca(root.path())
        .args(["--attach", "nope"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&missing.stderr).into_owned()
        + &String::from_utf8_lossy(&missing.stdout);
    assert!(said.contains("session 'nope' not found"), "got: {said}");
    let listed = output(heca(root.path()).arg("--sessions"));
    assert!(
        listed.starts_with("A\t"),
        "the saved session came back with the server it started: {listed}"
    );
    assert!(
        output(heca(root.path()).args(["rpc", "list_sessions"])).contains("\"A\""),
        "and the server stays running"
    );
}
