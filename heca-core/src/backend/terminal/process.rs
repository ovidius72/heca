//! OS-native foreground-process detection for a PTY (pane-runtime Phase 2).
//!
//! Event-first per `pane-runtime-state-plan.md` §0.2: this shim is called from
//! [`super::TerminalBackend::update`] on the terminal-output / reader-EOF wake,
//! debounced to [`FOREGROUND_DEBOUNCE`]. It does **not** poll on a periodic
//! timer — the deferred periodic-poll fallback is intentionally absent.
//!
//! OS specifics are `#[cfg(unix)]`; non-Unix builds get a no-op stub so the
//! backend compiles everywhere. macOS is the primary target (libproc via
//! libSystem); Linux uses `/proc`.

use std::path::PathBuf;

/// A foreground re-sample result.
///
/// `program == None` means the **shell itself** is foreground (`Idle`); the
/// backend maps that to the shell's basename for display. `Some(name)` is the
/// resolved foreground program **basename** (e.g. `"nvim"`).
#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ForegroundInfo {
    pub program: Option<String>,
    pub cwd: Option<PathBuf>,
}

/// Maximum foreground re-sample rate on the output-wake path (debounce, not a
/// periodic timer). Heavy output bursts don't hammer `tcgetpgrp`/libproc.
pub(super) const FOREGROUND_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(250);

/// Whether a foreground re-sample is due given the time elapsed since the last
/// one. Pure (time-only) so the debounce threshold is unit-testable independently
/// of `Instant`/PTY plumbing.
pub(super) fn fg_re_sample_due(elapsed: std::time::Duration) -> bool {
    elapsed >= FOREGROUND_DEBOUNCE
}

/// Re-sample the foreground process of the PTY whose master has fd `master_fd`
/// and whose shell's process-group leader is `shell_pgrp`.
///
/// Returns `None` if the foreground pgrp cannot be read (fd unavailable /
/// `tcgetpgrp` fails). `program == None` ⇒ the shell is foreground (`Idle`).
#[cfg(unix)]
pub(super) fn detect_foreground(
    master_fd: std::os::unix::io::RawFd,
    shell_pgrp: libc::pid_t,
) -> Option<ForegroundInfo> {
    // `tcgetpgrp` returns the foreground process group of the terminal, or -1
    // (errno set) on error. A non-positive result means we can't determine it.
    let fg_pgrp = unsafe { libc::tcgetpgrp(master_fd) };
    if fg_pgrp <= 0 {
        return None;
    }
    let (program, cwd) = resolve_foreground(
        fg_pgrp,
        shell_pgrp,
        process_name as fn(libc::pid_t) -> Option<String>,
        cwd_of as fn(libc::pid_t) -> Option<PathBuf>,
    );
    Some(ForegroundInfo { program, cwd })
}

/// Pure foreground-resolution logic, split from the OS calls so it is unit-
/// testable with mock resolvers.
///
/// - `fg_pgrp == shell_pgrp` ⇒ the shell is foreground ⇒ `Idle` ⇒ `program = None`
///   (the backend maps `None` to the shell basename); `cwd` is the shell's cwd.
/// - otherwise ⇒ a program is running ⇒ `program = name_of(fg_pgrp)`,
///   `cwd = cwd_of(fg_pgrp)`.
fn resolve_foreground(
    fg_pgrp: i32,
    shell_pgrp: i32,
    name_of: impl Fn(i32) -> Option<String>,
    cwd_of: impl Fn(i32) -> Option<PathBuf>,
) -> (Option<String>, Option<PathBuf>) {
    if fg_pgrp == shell_pgrp {
        (None, cwd_of(shell_pgrp))
    } else {
        (name_of(fg_pgrp), cwd_of(fg_pgrp))
    }
}

// ── macOS: libproc (resolved via libSystem; no explicit `#[link]` needed) ──

/// Resolve a pid's executable **basename** via `proc_pidpath`.
#[cfg(target_os = "macos")]
fn process_name(pid: libc::pid_t) -> Option<String> {
    // `proc_pidpath` writes the executable's full path into `buffer` and returns
    // the byte count written (excluding the NUL), or 0 on failure. The max buffer
    // size is `PROC_PIDPATHINFO_MAXSIZE` (4 * MAXPATHLEN).
    const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
    unsafe extern "C" {
        fn proc_pidpath(
            pid: libc::pid_t,
            buffer: *mut libc::c_void,
            buffersize: u32,
        ) -> libc::c_int;
    }
    let mut buf = vec![0u8; PROC_PIDPATHINFO_MAXSIZE];
    let n = unsafe { proc_pidpath(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    let path = std::str::from_utf8(&buf[..n as usize]).ok()?;
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Resolve a pid's cwd. TODO(macOS): implement via `proc_pidinfo`
/// (`PROC_PIDVNODEPATHINFO`) from libproc. Deferred — the **preferred** cwd source
/// is OSC 7 (Phase 3 shell integration); the OS fallback lands in a follow-up so
/// Phase 2 ships without risky struct-layout FFI.
#[cfg(target_os = "macos")]
fn cwd_of(_pid: libc::pid_t) -> Option<PathBuf> {
    None
}

// ── Linux: /proc filesystem (no FFI) ──

#[cfg(target_os = "linux")]
fn process_name(pid: libc::pid_t) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "linux")]
fn cwd_of(pid: libc::pid_t) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

// ── Other Unix (BSDs, etc.): not yet supported — report no foreground detail. ──
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "linux")))]
fn process_name(_pid: libc::pid_t) -> Option<String> {
    None
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "linux")))]
fn cwd_of(_pid: libc::pid_t) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_foreground_shell_is_idle_with_shell_cwd() {
        let (program, cwd) = resolve_foreground(
            100,
            100,
            |_| Some("unused".to_string()),
            |p| Some(PathBuf::from(format!("/cwd/{p}"))),
        );
        assert_eq!(program, None, "shell foreground ⇒ program None (Idle)");
        assert_eq!(cwd, Some(PathBuf::from("/cwd/100")));
    }

    #[test]
    fn resolve_foreground_program_when_not_shell() {
        let (program, cwd) = resolve_foreground(
            200,
            100,
            |p| Some(format!("prog{p}")),
            |p| Some(PathBuf::from(format!("/cwd/{p}"))),
        );
        assert_eq!(program, Some("prog200".to_string()));
        assert_eq!(cwd, Some(PathBuf::from("/cwd/200")));
    }

    #[test]
    fn resolve_foreground_unknown_name_and_cwd_degrade_to_none() {
        let (program, cwd) = resolve_foreground(200, 100, |_| None, |_| None);
        assert_eq!(program, None);
        assert_eq!(cwd, None);
    }

    #[test]
    fn fg_re_sample_due_respects_debounce_threshold() {
        assert!(!fg_re_sample_due(std::time::Duration::from_millis(0)));
        assert!(!fg_re_sample_due(std::time::Duration::from_millis(249)));
        assert!(fg_re_sample_due(std::time::Duration::from_millis(250)));
        assert!(fg_re_sample_due(std::time::Duration::from_millis(500)));
    }
}
