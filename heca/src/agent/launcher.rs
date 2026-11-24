//! **The launcher installed integrations run** — a small file in heca's data folder that runs the
//! heca started last, and does nothing when that heca is gone.
//!
//! An integration (Claude's hooks, its MCP server) stores a command line in the agent's own
//! settings. Storing the path of the heca that installed it broke every hook the day that build went
//! away — a development build in a worktree, removed with the worktree: each hook then failed with
//! exit 127 on every tool call. The launcher's path never changes, heca rewrites what it points at
//! each time it starts, and a launcher whose heca is missing exits quietly, so an agent never sees a
//! failure and the next heca to start makes it work again.
//!
//! - macOS and Linux: `heca-report`, a `sh` script — run by the hooks and by the MCP server.
//! - Windows: `heca-report.cmd`, run by the hooks (Git Bash and `cmd.exe` both run a `.cmd`). A stdio
//!   MCP server cannot be a `.cmd` (it needs `cmd /c`, which is reported to break the server's
//!   pipes), so there the server keeps the executable's own path.

use std::path::{Path, PathBuf};

/// The launcher's file name.
#[cfg(not(windows))]
const NAME: &str = "heca-report";
#[cfg(windows)]
const NAME: &str = "heca-report.cmd";

/// **What the launcher says**: run `exe` with the arguments it was given, or do nothing when `exe`
/// is gone.
#[cfg(not(windows))]
pub(crate) fn script(exe: &Path) -> String {
    let quoted = format!("'{}'", exe.display().to_string().replace('\'', r"'\''"));
    format!(
        "#!/bin/sh\n\
         # Written by heca each time it starts: runs the heca started last.\n\
         # When that heca is gone (a removed build), it does nothing rather than fail.\n\
         heca={quoted}\n\
         [ -x \"$heca\" ] || exit 0\n\
         exec \"$heca\" \"$@\"\n"
    )
}

#[cfg(windows)]
pub(crate) fn script(exe: &Path) -> String {
    let exe = exe.display();
    format!(
        "@echo off\r\n\
         rem Written by heca each time it starts: runs the heca started last.\r\n\
         rem When that heca is gone (a removed build), it does nothing rather than fail.\r\n\
         if not exist \"{exe}\" exit /b 0\r\n\
         \"{exe}\" %*\r\n"
    )
}

/// **Point the launcher at `path` to `exe`** — written only when it says something else, so a heca
/// that starts again writes nothing.
pub(crate) fn write_at(path: &Path, exe: &Path) -> std::io::Result<()> {
    let wanted = script(exe);
    if std::fs::read_to_string(path).is_ok_and(|now| now == wanted) {
        return Ok(());
    }
    heca_config::state_file::write_bytes(path, wanted.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// **The launcher, pointed at this heca** — its path, for an integration to store.
pub(crate) fn refresh() -> std::io::Result<PathBuf> {
    let path = heca_config::state_file::data_path("integrations")
        .ok_or_else(|| std::io::Error::other("no data folder for heca's launcher"))?
        .join(NAME);
    write_at(&path, &std::env::current_exe()?)?;
    Ok(path)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn run(launcher: &Path) -> std::process::Output {
        std::process::Command::new(launcher)
            .arg("hello")
            .output()
            .expect("the launcher runs")
    }

    /// **The launcher runs the heca it points at, and does nothing once that heca is gone** — an
    /// agent's hook through it never fails because a build was removed.
    #[test]
    fn the_launcher_runs_its_heca_and_does_nothing_once_it_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("it's heca");
        std::fs::write(&exe, "#!/bin/sh\necho \"ran $1\"\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let launcher = dir.path().join(NAME);
        write_at(&launcher, &exe).unwrap();
        let out = run(&launcher);
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "ran hello\n",
            "it passes its arguments on"
        );

        std::fs::remove_file(&exe).unwrap();
        let out = run(&launcher);
        assert!(out.status.success(), "a missing heca is not a failure");
        assert!(
            out.stdout.is_empty() && out.stderr.is_empty(),
            "and says nothing"
        );
    }
}
