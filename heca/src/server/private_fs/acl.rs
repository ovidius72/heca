//! Windows has no folder mode: who may use a folder is its access list. This writes the owner-only
//! one — the current user, and nobody else — where Unix writes `0700`.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
    SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn wide(text: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

/// The current user's security identifier as text (`S-1-5-21-…`): the one name Windows never
/// reuses for another account.
pub(super) fn current_user() -> io::Result<String> {
    let mut token: HANDLE = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token as RawHandle) };
    let mut needed = 0;
    // The first call only says how much room the answer takes.
    unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, ptr::null_mut(), 0, &mut needed) };
    // `u64`s, so the `TOKEN_USER` at the front of the buffer is aligned.
    let mut user = vec![0u64; (needed as usize).div_ceil(size_of::<u64>()).max(1)];
    let room = u32::try_from(user.len() * size_of::<u64>()).map_err(io::Error::other)?;
    if unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, user.as_mut_ptr().cast(), room, &mut needed) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // The SID lives in the same buffer, after the `TOKEN_USER` that points at it.
    sid_text(unsafe { (*user.as_ptr().cast::<TOKEN_USER>()).User.Sid })
}

pub(super) fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 { return Err(io::Error::last_os_error()); }
    let length = (0..).take_while(|&at| unsafe { *text.add(at) } != 0).count();
    let string = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    unsafe { LocalFree(text.cast()) };
    Ok(string)
}

/// Make `folder` the current user's alone: one entry in its access list — full control for them,
/// inherited by everything made inside — and no entries taken from the folder above.
pub(super) fn restrict_to_current_user(folder: &Path) -> io::Result<()> {
    // D:P — a protected list, so nothing is inherited into it; OICI — its one entry passes to files
    // and folders made inside (the socket, the lock, a server's own folders).
    let sddl = format!("D:P(A;OICI;FA;;;{})", current_user()?);
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(wide(&sddl).as_ptr(), SDDL_REVISION_1, &mut descriptor, ptr::null_mut()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let (mut present, mut defaulted, mut acl) = (0, 0, ptr::null_mut());
        if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut path = wide(folder);
        let code = unsafe {
            SetNamedSecurityInfoW(path.as_mut_ptr(), SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, ptr::null_mut(), ptr::null_mut(), acl, ptr::null())
        };
        if code == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(code as i32)) }
    })();
    unsafe { LocalFree(descriptor) };
    result
}

/// What the folder's access list says, for a test: whether it is protected from inheritance, and the
/// account (as text) of each entry, all of which must be "allow".
#[cfg(test)]
pub(super) fn entries(folder: &Path) -> io::Result<(bool, Vec<String>)> {
    use windows_sys::Win32::Security::Authorization::GetNamedSecurityInfoW;
    use windows_sys::Win32::Security::{
        ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, AclSizeInformation, GetAce, GetAclInformation,
        GetSecurityDescriptorControl, SE_DACL_PROTECTED,
    };
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
    let (mut acl, mut descriptor): (*mut ACL, PSECURITY_DESCRIPTOR) = (ptr::null_mut(), ptr::null_mut());
    let code = unsafe {
        GetNamedSecurityInfoW(wide(folder).as_ptr(), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
            ptr::null_mut(), ptr::null_mut(), &mut acl, ptr::null_mut(), &mut descriptor)
    };
    if code != 0 { return Err(io::Error::from_raw_os_error(code as i32)); }
    let result = (|| {
        let (mut control, mut revision) = (0, 0);
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut size: ACL_SIZE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetAclInformation(acl, (&raw mut size).cast(), size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut accounts = Vec::new();
        for at in 0..size.AceCount {
            let mut ace = ptr::null_mut();
            if unsafe { GetAce(acl, at, &mut ace) } == 0 { return Err(io::Error::last_os_error()); }
            let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
            if unsafe { (*ace).Header.AceType } != ACCESS_ALLOWED_ACE_TYPE {
                accounts.push("(not an allow entry)".to_owned());
                continue;
            }
            accounts.push(sid_text(unsafe { (&raw mut (*ace).SidStart).cast() })?);
        }
        Ok((control & SE_DACL_PROTECTED != 0, accounts))
    })();
    unsafe { LocalFree(descriptor) };
    result
}
