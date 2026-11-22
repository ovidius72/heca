//! **Where a file was dropped**, asked of the system.
//!
//! winit 0.30 says which files were dropped on the window but not where (idea I026: winit 0.31,
//! once stable, carries the position with the event and this module can go). The drop lands where
//! the pointer is when it is released, so the system is asked for the pointer's position in the
//! window at that moment — in logical pixels, the unit every widget measures in.
//!
//! macOS, Windows and X11 answer. Wayland is not asked: winit 0.30 delivers no file drop there at
//! all.

use winit::window::Window;

/// **The pointer's position in `window`**, in logical pixels — `None` when the system cannot say.
pub(crate) fn pointer_in_window(window: &Window, scale_factor: f64) -> Option<(f32, f32)> {
    let scale = if scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    platform::pointer_in_window(window, scale)
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    /// The window reports the pointer in its own coordinates, in points (logical pixels already),
    /// with the origin at the bottom left; the content view turns it into its own, top-left origin.
    pub(super) fn pointer_in_window(window: &Window, _scale: f64) -> Option<(f32, f32)> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return None;
        };
        // SAFETY: `h.ns_view` is winit's live content view; it is only read here, on the main
        // thread (the event loop), while winit keeps it alive.
        unsafe {
            let view: &NSView = h.ns_view.cast().as_ref();
            let ns_window = view.window()?;
            let in_window = ns_window.mouseLocationOutsideOfEventStream();
            let in_view = view.convertPoint_fromView(in_window, None);
            let y = if view.isFlipped() {
                in_view.y
            } else {
                view.bounds().size.height - in_view.y
            };
            Some((in_view.x as f32, y as f32))
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    use winit::window::Window;

    /// The cursor on the screen, turned into the window's client area, in physical pixels.
    pub(super) fn pointer_in_window(window: &Window, scale: f64) -> Option<(f32, f32)> {
        let handle = window.window_handle().ok()?;
        let RawWindowHandle::Win32(h) = handle.as_raw() else {
            return None;
        };
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: `point` is a valid out-parameter and `h.hwnd` is winit's live window.
        let ok = unsafe {
            GetCursorPos(&mut point) != 0 && ScreenToClient(h.hwnd.get() as _, &mut point) != 0
        };
        ok.then(|| {
            (
                (f64::from(point.x) / scale) as f32,
                (f64::from(point.y) / scale) as f32,
            )
        })
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use winit::window::Window;

    /// X11 is asked where the pointer is relative to the window, in physical pixels. Wayland has no
    /// such question, and winit 0.30 delivers it no drop to ask about.
    pub(super) fn pointer_in_window(window: &Window, scale: f64) -> Option<(f32, f32)> {
        let (RawWindowHandle::Xlib(w), RawDisplayHandle::Xlib(d)) = (
            window.window_handle().ok()?.as_raw(),
            window.display_handle().ok()?.as_raw(),
        ) else {
            return None;
        };
        let display = d.display?.as_ptr().cast::<x11_dl::xlib::Display>();
        let xlib = x11_dl::xlib::Xlib::open().ok()?;
        let (mut root, mut child) = (0, 0);
        let (mut root_x, mut root_y, mut x, mut y, mut mask) = (0, 0, 0, 0, 0);
        // SAFETY: `display` and `w.window` are winit's live X11 connection and window; every
        // out-parameter is a valid local.
        let found = unsafe {
            (xlib.XQueryPointer)(
                display,
                w.window,
                &mut root,
                &mut child,
                &mut root_x,
                &mut root_y,
                &mut x,
                &mut y,
                &mut mask,
            ) != 0
        };
        found.then(|| ((f64::from(x) / scale) as f32, (f64::from(y) / scale) as f32))
    }
}
