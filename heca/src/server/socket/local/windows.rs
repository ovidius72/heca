use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::{size_of, zeroed};
use std::net::Shutdown;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, AsRawSocket};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileAttributeTagInfo,
    GetFileInformationByHandle, GetFileInformationByHandleEx,
};
use windows_sys::Win32::System::IO::CancelIoEx;
use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};

use super::FileIdentity;

/// `std`'s `UnixStream`, on Windows. It differs from `uds_windows`' in two ways, both so the wire link
/// can use it as it uses `std`'s.
///
/// * A clone is the same socket, not a duplicate of it. That is what lets one thread end a read that
///   another is blocked in: the pending call is cancelled by the socket it is waiting on.
/// * [`shutdown`](Self::shutdown) does that. Winsock's `shutdown` on a Unix socket leaves a read that
///   is already waiting to wait on; cancelling the socket's pending I/O ends it.
#[derive(Debug)]
pub(crate) struct UnixStream(Arc<uds_windows::UnixStream>);

impl UnixStream {
    pub(crate) fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        uds_windows::UnixStream::connect(path).map(Self::from)
    }

    #[cfg(test)]
    pub(crate) fn pair() -> io::Result<(Self, Self)> {
        uds_windows::UnixStream::pair().map(|(a, b)| (Self::from(a), Self::from(b)))
    }

    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(Arc::clone(&self.0)))
    }

    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }

    pub(crate) fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }

    pub(crate) fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        // Shut first: a read that starts after this returns at once, and one already waiting is
        // ended by the cancel below. Nothing falls between the two.
        let shut = self.0.shutdown(how);
        if how != Shutdown::Write {
            unsafe { CancelIoEx(self.0.as_raw_socket() as HANDLE, ptr::null()) };
        }
        shut
    }
}

impl From<uds_windows::UnixStream> for UnixStream {
    fn from(stream: uds_windows::UnixStream) -> Self {
        Self(Arc::new(stream))
    }
}

impl Read for UnixStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buffer)
    }
}

impl Write for UnixStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

#[derive(Debug)]
pub(crate) struct UnixListener(uds_windows::UnixListener);

impl UnixListener {
    pub(crate) fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
        uds_windows::UnixListener::bind(path).map(Self)
    }

    pub(crate) fn incoming(&self) -> impl Iterator<Item = io::Result<UnixStream>> + '_ {
        self.0.incoming().map(|stream| stream.map(UnixStream::from))
    }
}

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
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            (&raw mut info).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        && info.ReparseTag == REPARSE_TAG_AF_UNIX)
}

/// The volume serial number and the file index: the pair Windows gives a file for as long as it
/// exists, as a device and inode do on Unix. Times or sizes would change under a live socket.
pub(crate) fn identity(path: &Path) -> io::Result<FileIdentity> {
    let file = entry(path)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
    ))
}

/// Nothing to do: Windows has no file mode. The socket sits in the folder `private_folder` made, whose
/// access list admits the current user alone and which the socket file inherits.
pub(crate) fn restrict_to_owner(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Start `command` with no console and in a process group of its own, so closing the terminal or
/// pressing Ctrl+C there does not reach it.
pub(crate) fn detach(command: &mut Command) -> &mut Command {
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
}
