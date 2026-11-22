//! **The local socket — the one door for clients and scripts.**
//!
//! The server listens on a local-only Unix socket in an owner-only runtime folder; it never
//! touches the network. Windows serves the same kind of socket, and [`local`](crate::server::socket::local) is the only place the
//! two differ. A script connects with [`heca rpc <line>`](crate::server::socket::run_rpc_cli) and a
//! client (a window) attaches over the same socket. An RPC request is one line — an action by its
//! catalog name, exactly what [`crate::rpc`] already reads — and the answer comes back as text, ended
//! by a newline.
//!
//! The listener sends requests through the server's bounded inbox. Attached clients receive framed
//! state messages, while scripts receive a line reply. No window event proxy owns the socket.

use std::io::{BufRead, Read, Write};
use std::path::PathBuf;

mod connect;
mod listener;
pub(crate) mod local;
mod runtime;
pub(crate) use connect::{Start, connect};
use local::UnixStream;
pub(crate) use runtime::RuntimeListener;

/// The owner-only runtime folder the socket lives in: `$XDG_RUNTIME_DIR/heca/` where the platform
/// has one (Linux), else a per-user temp folder (macOS and Windows have no runtime dir; their temp dir
/// is already per-user). Created owner-only (`0700`; on Windows an access list for the user alone) so
/// the socket inside it is reachable only by its owner. `None` when
/// the platform gives no writable answer — the socket is then simply not served and heca still runs
/// standalone.
fn runtime_dir() -> Option<PathBuf> {
    let base = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("heca");
    super::private_fs::private_folder(&dir).ok()?;
    Some(dir)
}

/// Where the server listens and a client connects. One socket for now; sessions give it a name
/// later (P100).
pub(crate) fn socket_path() -> Option<PathBuf> {
    Some(runtime_dir()?.join("default.sock"))
}

/// Connect to the running server and run one RPC line, returning its answer.
fn request(line: &str) -> std::io::Result<String> {
    let path =
        socket_path().ok_or_else(|| std::io::Error::other("no runtime folder for the socket"))?;
    request_to(&path, line)
}

fn request_to(path: &std::path::Path, line: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(path)?;
    let limits = heca_config::loader::AppConfig::load()
        .config
        .settings
        .connection;
    let timeout = Some(std::time::Duration::from_millis(
        limits.request_timeout_ms.get(),
    ));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    writeln!(stream, "{line}")?;
    // Done writing; let the server see end-of-request without closing the read half.
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut out = String::new();
    stream.read_to_string(&mut out)?;
    Ok(out)
}

/// **The `rpc` command lines, as `--help` lists them** — written here, beside the code that answers
/// them, because `rpc` is read before the command table (its arguments are a whole action line,
/// not a fixed list).
pub(crate) const RPC_COMMANDS: &[(&str, &str)] = &[
    (
        "rpc <action> [name=value …]",
        "Run an action on the running server — any action `--list-actions` prints.",
    ),
    (
        "rpc --events",
        "Print the server's events, one per line, until it stops.",
    ),
];

/// **`heca rpc <line>`** — the script door. `Some(output)` when `args` is an `rpc` invocation (the
/// caller prints it and does **not** open a window); `None` otherwise, so the normal CLI runs.
///
/// One way in: the line is whatever [`crate::rpc`] reads — any action in the catalog, by name —
/// so a script reaches every registered action with nothing added here per action. One argument is
/// a whole line and goes out as it is; several are the action and one value each, written into a
/// line by [`crate::rpc::line::from_argv`], which puts quotes where a value needs them: the
/// arguments `say` and `text=ciao mondo` go out as `say text="ciao mondo"`.
pub(crate) fn run_rpc_cli(args: &[String]) -> Option<String> {
    // `heca rpc --events` is a streaming subscription, not a one-shot line: it prints each event as
    // it arrives and blocks until the socket closes. Intercept it before the one-shot path, which
    // would otherwise send `--events` to the server as an unknown action.
    if args.first().map(String::as_str) == Some("rpc")
        && args.get(1).map(String::as_str) == Some("--events")
        && args.len() == 2
    {
        return Some(match stream_events() {
            Ok(()) => String::new(),
            Err(error) => format!("could not reach a running heca: {error}\n(is heca running?)\n"),
        });
    }
    run_rpc_cli_with(args, request)
}

/// Subscribe to the server's event stream and print each event as a line, until the socket closes.
/// The events are the existing `Change`s, serialized by the wire model — one per line.
fn stream_events() -> std::io::Result<()> {
    let path =
        socket_path().ok_or_else(|| std::io::Error::other("no runtime folder for the socket"))?;
    let mut stream = UnixStream::connect(&path)?;
    stream.write_all(runtime::EVENTS_PREFACE)?;
    let stdout = std::io::stdout();
    for line in std::io::BufReader::new(stream).lines() {
        let mut out = stdout.lock();
        if writeln!(out, "{}", line?).is_err() {
            break;
        }
    }
    Ok(())
}

/// **`heca --kill-server`** — connect to a running server and stop it, and with it every pane and
/// terminal (tmux's `kill-server`). Nothing is spawned: if no server is listening, it says so. The
/// request is the `kill_server` action, the same one the palette and a key binding run; the server
/// answers one line and then ends, or says why it will not.
pub(crate) fn kill_server() -> String {
    match request("kill_server") {
        Ok(answer) => answer,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            "heca: no server is running\n".to_string()
        }
        Err(error) => format!("heca: could not reach the server: {error}\n"),
    }
}

fn run_rpc_cli_with(
    args: &[String],
    request: impl FnOnce(&str) -> std::io::Result<String>,
) -> Option<String> {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("rpc") {
        return None;
    }
    let line = match crate::rpc::line::from_argv(&it.cloned().collect::<Vec<_>>()) {
        Ok(line) => line,
        Err(error) => return Some(format!("heca rpc: {error}\n")),
    };
    if line.trim().is_empty() {
        return Some(format!(
            "usage: heca {}\nRun `heca --list-actions` to see every action.\n",
            RPC_COMMANDS[0].0
        ));
    }
    Some(match request(&line) {
        Ok(answer) => answer,
        Err(e) => format!("could not reach a running heca: {e}\n(is heca running?)\n"),
    })
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_an_rpc_invocation_is_handled_here() {
        // Not `rpc` → None, so the ordinary CLI (flags) runs.
        assert!(run_rpc_cli(&argv(&["--help"])).is_none());
        assert!(run_rpc_cli(&[]).is_none());
        // `rpc` with no line → usage, handled (does not open a window).
        let out = run_rpc_cli(&argv(&["rpc"])).expect("rpc is handled");
        assert!(out.contains("usage: heca rpc"), "got: {out}");
    }

    #[test]
    fn an_rpc_line_with_no_server_says_so_rather_than_opening_a_window() {
        // No server is listening in a unit test, so the request fails — but it is still *handled*
        // (Some), with a sentence a script can read, never a silent window launch.
        let out = run_rpc_cli_with(&argv(&["rpc", "split_horizontal"]), |_| {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        })
        .expect("rpc is handled");
        assert!(out.contains("could not reach a running heca"), "got: {out}");
    }

    /// The line the server would be sent for `words`, or what `heca rpc` printed instead.
    fn sent_for(words: &[&str]) -> String {
        let sent = std::cell::RefCell::new(None);
        let out = run_rpc_cli_with(&argv(words), |line| {
            *sent.borrow_mut() = Some(line.to_string());
            Ok("ok".to_string())
        })
        .expect("rpc is handled");
        sent.into_inner().unwrap_or(out)
    }

    #[test]
    fn one_argument_is_sent_as_the_whole_line() {
        assert_eq!(
            sent_for(&["rpc", r#"say text="ciao mondo""#]),
            r#"say text="ciao mondo""#
        );
        assert_eq!(sent_for(&["rpc", "focus_pane 42"]), "focus_pane 42");
    }

    #[test]
    fn several_arguments_are_one_value_each() {
        assert_eq!(
            sent_for(&["rpc", "say", "text=ciao mondo", "C:\\my dir\\"]),
            "say text=\"ciao mondo\" \"C:\\\\my dir\\\\\""
        );
        assert_eq!(
            sent_for(&["rpc", "place_pane", "5", "0", "1", "pane_idx=0"]),
            "place_pane 5 0 1 pane_idx=0"
        );
        assert_eq!(
            sent_for(&["rpc", "act", "--dock", "docker.right"]),
            "act --dock docker.right"
        );
    }

    #[test]
    fn a_line_break_or_nul_in_an_argument_is_refused_before_sending() {
        for words in [
            &["rpc", "say\nx"][..],
            &["rpc", "say", "a\nb"],
            &["rpc", "say", "k=a\r"],
            &["rpc", "say", "\0"],
        ] {
            let out = sent_for(words);
            assert!(
                out.starts_with("heca rpc: ") && out.contains("not allowed"),
                "got: {out}"
            );
        }
    }

    #[test]
    fn the_socket_lives_in_a_runtime_folder_named_heca() {
        let path = socket_path().expect("a runtime folder on this platform");
        assert!(
            path.ends_with("heca/default.sock"),
            "got: {}",
            path.display()
        );
    }
}
