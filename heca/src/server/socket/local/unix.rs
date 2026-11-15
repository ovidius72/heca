use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

pub(crate) use std::os::unix::net::{UnixListener, UnixStream};

use super::FileIdentity;

/// Whether `path` is a socket itself, not a link to one.
pub(crate) fn is_socket(path: &Path) -> io::Result<bool> {
    Ok(std::fs::symlink_metadata(path)?.file_type().is_socket())
}

pub(crate) fn identity(path: &Path) -> io::Result<FileIdentity> {
    let metadata = std::fs::symlink_metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

/// Mode `0600`: the owner reads and writes, nobody else.
pub(crate) fn restrict_to_owner(path: &Path) -> io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// Start `command` in a process group of its own, so the terminal's job control and its hang-up
/// signal do not reach it.
pub(crate) fn detach(command: &mut Command) -> &mut Command {
    command.process_group(0)
}
