//! **The local socket — the one door for clients and scripts.**
//!
//! The server listens on a local-only Unix socket in an owner-only runtime folder; it never
//! touches the network. A script connects with [`heca rpc <line>`](run_rpc_cli) and a client
//! (a window) will attach over the same socket. A request is one line — an action by its catalog
//! name, exactly what [`crate::rpc`] already reads — and the answer is one line back.
//!
//! The listener runs on its own thread (the app's event loop is single-threaded and owns the
//! state), so a request is handed to the main thread through the [`EventLoopProxy`] and the answer
//! comes back the same way. Each connection is served on its own thread, so one slow client never
//! blocks another or the accept loop.
//!
//! Windows (named pipes) is a later step; this is `cfg(unix)`.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;

use winit::event_loop::EventLoopProxy;

use crate::app::events::AppEvent;

/// The owner-only runtime folder the socket lives in: `$XDG_RUNTIME_DIR/heca/` where the platform
/// has one (Linux), else a per-user temp folder (macOS has no runtime dir; its temp dir is already
/// per-user). Created `0700` so the socket inside it is reachable only by its owner. `None` when
/// the platform gives no writable answer — the socket is then simply not served and heca still runs
/// standalone.
fn runtime_dir() -> Option<PathBuf> {
    let base = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("heca");
    std::fs::create_dir_all(&dir).ok()?;
    // Best-effort: a platform that refuses the mode still has a per-user base dir above.
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    Some(dir)
}

/// Where the server listens and a client connects. One socket for now; sessions give it a name
/// later (P100).
pub(crate) fn socket_path() -> Option<PathBuf> {
    Some(runtime_dir()?.join("default.sock"))
}

/// Start serving the local socket on a background thread. Returns immediately; the thread runs for
/// the life of the process. Does nothing (and says why on stderr) when the socket cannot be opened,
/// so a failure here never stops the window opening.
pub(crate) fn serve(proxy: EventLoopProxy<AppEvent>) {
    let Some(path) = socket_path() else {
        return;
    };
    // A stale socket left by a crashed server refuses `bind`; remove it first. It is ours alone —
    // it lives in our owner-only runtime folder.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!(
                "[heca] could not open the control socket at {}: {e}",
                path.display()
            );
            return;
        }
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(stream) = conn else { continue };
            let proxy = proxy.clone();
            // One thread per connection: the accept loop only accepts, and a slow or stuck client
            // waits on its own thread without holding up the next one.
            std::thread::spawn(move || serve_one(stream, proxy));
        }
    });
}

/// Serve one connection: read the request line, run it on the main thread through `proxy`, write
/// the one-line answer back.
fn serve_one(stream: UnixStream, proxy: EventLoopProxy<AppEvent>) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let line = line.trim().to_string();
    if line.is_empty() {
        return;
    }
    let (reply, answer_rx) = mpsc::channel();
    if proxy.send_event(AppEvent::Rpc { line, reply }).is_err() {
        // The event loop is gone (the window is closing): nothing can answer.
        return;
    }
    let answer = answer_rx
        .recv()
        .unwrap_or_else(|_| "error: heca is not running".to_string());
    let mut stream = stream;
    let _ = writeln!(stream, "{answer}");
}

/// Connect to the running server and run one RPC line, returning its answer.
fn request(line: &str) -> std::io::Result<String> {
    let path =
        socket_path().ok_or_else(|| std::io::Error::other("no runtime folder for the socket"))?;
    let mut stream = UnixStream::connect(&path)?;
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
        let out = run_rpc_cli(&argv(&["rpc", "split_horizontal"])).expect("rpc is handled");
        assert!(out.contains("could not reach a running heca"), "got: {out}");
    }

    #[test]
    fn the_socket_lives_in_a_runtime_folder_named_heca() {
        let path = socket_path().expect("a runtime folder on this platform");
        assert!(path.ends_with("heca/default.sock"), "got: {}", path.display());
    }
}
