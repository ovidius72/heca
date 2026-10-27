//! **The local socket — the one door for clients and scripts.**
//!
//! The server listens on a local-only Unix socket in an owner-only runtime folder; it never
//! touches the network. A script connects with [`heca rpc <line>`](crate::server::socket::run_rpc_cli) and a client
//! (a window) attaches over the same socket. An RPC request is one line — an action by its catalog
//! name, exactly what [`crate::rpc`] already reads — and the answer is one line back.
//!
//! The listener sends requests through the server's bounded inbox. Attached clients receive framed
//! state messages, while scripts receive a line reply. No window event proxy owns the socket.
//!
//! Windows (named pipes) is a later step; this is `cfg(unix)`.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

mod listener;
mod runtime;
pub(crate) use runtime::RuntimeListener;

/// The owner-only runtime folder the socket lives in: `$XDG_RUNTIME_DIR/heca/` where the platform
/// has one (Linux), else a per-user temp folder (macOS has no runtime dir; its temp dir is already
/// per-user). Created `0700` so the socket inside it is reachable only by its owner. `None` when
/// the platform gives no writable answer — the socket is then simply not served and heca still runs
/// standalone.
fn runtime_dir() -> Option<PathBuf> {
    let base = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("heca");
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {},
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
        Err(_) => return None,
    }
    if !std::fs::symlink_metadata(&dir).ok()?.file_type().is_dir() { return None; }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok()?;
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
    let mut stream = UnixStream::connect(&path)?;
    let limits = heca_config::loader::AppConfig::load().config.settings.connection;
    let timeout = Some(std::time::Duration::from_millis(limits.request_timeout_ms.get()));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    writeln!(stream, "{line}")?;
    // Done writing; let the server see end-of-request without closing the read half.
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut out = String::new();
    stream.read_to_string(&mut out)?;
    Ok(out)
}

/// **`heca rpc <line>`** — the script door. `Some(output)` when `args` is an `rpc` invocation (the
/// caller prints it and does **not** open a window); `None` otherwise, so the normal CLI runs.
///
/// One way in: the line is whatever [`crate::rpc`] reads — any action in the catalog, by name —
/// so a script reaches every registered action with nothing added here per action.
pub(crate) fn run_rpc_cli(args: &[String]) -> Option<String> {
    run_rpc_cli_with(args, request)
}

/// **`heca --kill-server`** — connect to a running server and stop it, and with it every pane and
/// terminal (tmux's `kill-server`). Nothing is spawned: if no server is listening, it says so. The
/// request is the one protocol word [`KILL_SERVER`](crate::server::run::KILL_SERVER); the server
/// answers one line and then ends.
pub(crate) fn kill_server() -> String {
    match request(crate::server::run::KILL_SERVER) {
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

fn run_rpc_cli_with(args: &[String], request: impl FnOnce(&str) -> std::io::Result<String>) -> Option<String> {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("rpc") {
        return None;
    }
    let line = it.cloned().collect::<Vec<_>>().join(" ");
    if line.trim().is_empty() {
        return Some(
            "usage: heca rpc <action> [args…]\nRun `heca --list-actions` to see every action.\n"
                .to_string(),
        );
    }
    Some(match request(line.trim()) {
        Ok(answer) => answer,
        Err(e) => format!("could not reach a running heca: {e}\n(is heca running?)\n"),
    })
}

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
        let out = run_rpc_cli_with(&argv(&["rpc", "split_horizontal"]), |_| Err(std::io::Error::from(std::io::ErrorKind::NotFound))).expect("rpc is handled");
        assert!(out.contains("could not reach a running heca"), "got: {out}");
    }

    #[test]
    fn the_socket_lives_in_a_runtime_folder_named_heca() {
        let path = socket_path().expect("a runtime folder on this platform");
        assert!(path.ends_with("heca/default.sock"), "got: {}", path.display());
    }
}
