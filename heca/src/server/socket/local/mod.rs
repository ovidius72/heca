//! **Where the platforms differ under the local socket — and nowhere else.**
//!
//! heca serves one kind of socket on every system: a Unix domain socket. Unix has it in `std`;
//! Windows 10 (build 1809) and later have it in the system and the `uds_windows` crate gives it the
//! same API, so [`UnixListener`] and [`UnixStream`] are the same code to the listener, the wire link,
//! `heca rpc` and the client that starts the background server. What `std` does not carry over is
//! four small things about the socket *file*, and they are the whole of this folder:
//!
//! | | Unix | Windows |
//! | --- | --- | --- |
//! | [`is_socket`] | the file type | a reparse point tagged `AF_UNIX` |
//! | [`identity`] — is this file still the one I made? | device and inode | volume serial and file index |
//! | [`restrict_to_owner`] | mode `0600` | nothing: the folder's access list holds it (`private_fs`) |
//! | [`detach`] — the background server leaves the terminal | its own process group | no console, its own process group |

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{UnixListener, UnixStream, detach, identity, is_socket, restrict_to_owner};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{UnixListener, UnixStream, detach, identity, is_socket, restrict_to_owner};

/// Names a file for as long as it exists; two paths that give the same one are the same file.
pub(crate) type FileIdentity = (u64, u64);

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> tempfile::TempDir { tempfile::tempdir().unwrap() }

    #[test]
    fn a_socket_file_is_a_socket_and_a_plain_file_is_not() {
        let folder = folder();
        let socket = folder.path().join("a.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        let plain = folder.path().join("plain");
        std::fs::write(&plain, b"x").unwrap();
        assert!(is_socket(&socket).unwrap());
        assert!(!is_socket(&plain).unwrap());
        assert!(is_socket(&folder.path().join("missing")).is_err());
    }

    #[test]
    fn a_file_keeps_its_identity_and_no_two_live_files_share_one() {
        let folder = folder();
        let (a, b) = (folder.path().join("a"), folder.path().join("b"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        assert_eq!(identity(&a).unwrap(), identity(&a).unwrap());
        assert_ne!(identity(&a).unwrap(), identity(&b).unwrap());
        std::fs::write(&a, b"changed and longer").unwrap();
        assert_eq!(identity(&a).unwrap(), identity(&a).unwrap());
        assert!(identity(&folder.path().join("missing")).is_err());
    }

    #[test]
    fn a_socket_file_is_restricted_to_its_owner_without_failing() {
        let folder = folder();
        let socket = folder.path().join("a.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        restrict_to_owner(&socket).unwrap();
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&socket).unwrap().permissions()) & 0o777, 0o600);
    }

    #[test]
    fn a_detached_command_still_runs_and_reports_its_exit() {
        let exit_3 = if cfg!(windows) { ("cmd", ["/C", "exit 3"]) } else { ("/bin/sh", ["-c", "exit 3"]) };
        let status = detach(std::process::Command::new(exit_3.0).args(exit_3.1)).status().unwrap();
        assert_eq!(status.code(), Some(3));
    }
}
