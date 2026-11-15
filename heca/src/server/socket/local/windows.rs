use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FileAttributeTagInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
};
use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};

pub(crate) use uds_windows::{UnixListener, UnixStream};

use super::FileIdentity;

/// What Windows writes in a socket file's reparse tag (`IO_REPARSE_TAG_AF_UNIX`).
const REPARSE_TAG_AF_UNIX: u32 = 0x8000_0023;

/// The directory entry at `path` itself — a socket file is a reparse point, so it is opened as one
/// instead of being followed — for reading its attributes only, and without locking anyone out.
fn entry(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

/// Whether `path` is a socket file: a reparse point whose tag says `AF_UNIX`.
pub(crate) fn is_socket(path: &Path) -> io::Result<bool> {
    let file = entry(path)?;
    let mut info: FILE_ATTRIBUTE_TAG_INFO = unsafe { zeroed() };
    let read = unsafe {
        GetFileInformationByHandleEx(file.as_raw_handle(), FileAttributeTagInfo,
            (&raw mut info).cast(), size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32)
    };
    if read == 0 { return Err(io::Error::last_os_error()); }
    Ok(info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && info.ReparseTag == REPARSE_TAG_AF_UNIX)
}

/// The volume serial number and the file index: the pair Windows gives a file for as long as it
/// exists, as a device and inode do on Unix. Times or sizes would change under a live socket.
pub(crate) fn identity(path: &Path) -> io::Result<FileIdentity> {
    let file = entry(path)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((u64::from(info.dwVolumeSerialNumber), u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow)))
}

/// Nothing to do: Windows has no file mode. The socket sits in the folder `private_folder` made, whose
/// access list admits the current user alone and which the socket file inherits.
pub(crate) fn restrict_to_owner(_: &Path) -> io::Result<()> { Ok(()) }

/// Start `command` with no console and in a process group of its own, so closing the terminal or
/// pressing Ctrl+C there does not reach it.
pub(crate) fn detach(command: &mut Command) -> &mut Command {
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
}
