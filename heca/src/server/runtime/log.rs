//! **Where the background server's messages go.** It is started with no terminal — its output went
//! to `/dev/null`, so every warning it printed reached nobody. It now writes them to `heca.log` in its
//! own folder, and keeps the previous run's as `heca.log.1`: one run back is what a report needs, and
//! the files never grow past two runs.

use std::path::Path;

/// Send this process's standard output and error to `log`, keeping the previous file as
/// `<log>.1`. Nothing changes when the file cannot be opened: the messages then go where they went.
#[cfg(unix)]
pub(super) fn send_output_to(log: &Path) {
    use std::os::fd::AsRawFd;
    let previous = log.with_extension("log.1");
    let _ = std::fs::rename(log, previous);
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(log) else { return };
    // SAFETY: `dup2` on two valid descriptors this process owns; it only redirects 1 and 2.
    unsafe {
        libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO);
        libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO);
    }
}

/// A Windows server shares its window's process and console, so its messages already have a home.
#[cfg(not(unix))]
pub(super) fn send_output_to(_log: &Path) {}
