//! **Why heca could not open, said where the user will see it.**
//!
//! An error before the window opens (the session server is another version of heca, it cannot be
//! reached, …) used to go only to the terminal. Opened from the Dock or the Finder there is no
//! terminal, so the app just vanished and looked like a crash. Every start-up error comes here,
//! once: it is printed on stderr as before, and shown in the system's own alert where the system
//! has one (macOS, Windows). Linux keeps the terminal line.

/// Say `message`, under `title`, on stderr and in a system alert, and wait for the user to read it.
pub(crate) fn show(title: &str, message: &str) {
    eprintln!("[heca] {title}: {message}");
    platform::alert(title, message);
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSAlert, NSAlertStyle};
    use objc2_foundation::{MainThreadMarker, NSString};

    pub(super) fn alert(title: &str, message: &str) {
        // An alert needs the main thread; heca reaches here on it, after its event loop ended.
        let Some(mtm) = MainThreadMarker::new() else { return };
        // SAFETY: on the main thread (checked above); the alert is built, shown modally and dropped
        // here, with strings it owns.
        unsafe {
            let alert = NSAlert::new(mtm);
            alert.setAlertStyle(NSAlertStyle::Warning);
            alert.setMessageText(&NSString::from_str(title));
            alert.setInformativeText(&NSString::from_str(message));
            alert.runModal();
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};

    pub(super) fn alert(title: &str, message: &str) {
        let wide = |text: &str| text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (title, message) = (wide(title), wide(message));
        // SAFETY: both strings are NUL-terminated UTF-16 buffers alive for the call; no owner window.
        unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING) };
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    /// No system alert every Linux desktop has: the terminal line printed by `show` is the notice.
    pub(super) fn alert(_title: &str, _message: &str) {}
}
