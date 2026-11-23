//! **The program's icon** — the window's icon (Windows, Linux) and the Dock's (macOS) while heca
//! runs, so an unpackaged run or a program built on heca is shown as itself. Where it comes from,
//! first to last, as the configuration files are found:
//!
//! 1. the user's own `icon.png` beside `config.toml` (optional);
//! 2. the icon the program gives [`App::icon`](crate::App::icon) — heca-pro's, for instance;
//! 3. heca's own logo, shipped inside it.
//!
//! The installed app's icon in the Finder or a launcher is the package's (`heca/Cargo.toml`
//! `[package.metadata.packager] icons`), which this does not change.

use std::borrow::Cow;
use std::sync::OnceLock;

/// heca's own logo — the same file the package is built from.
const HECA_ICON: &[u8] = include_bytes!("../../assets/icon/heca.png");

/// The size a window icon is given at: a system draws it far smaller, so the full logo is waste.
const WINDOW_ICON_PX: u32 = 256;

static PROGRAM_ICON: OnceLock<&'static [u8]> = OnceLock::new();

/// Remember the program's icon — once, from [`App::run`](crate::App::run).
pub(crate) fn set(png: &'static [u8]) {
    let _ = PROGRAM_ICON.set(png);
}

/// The icon's encoded image, by the order in the module docs.
fn encoded() -> Cow<'static, [u8]> {
    if let Some(user) =
        heca_config::loader::user_icon_path().and_then(|path| std::fs::read(path).ok())
    {
        return Cow::Owned(user);
    }
    Cow::Borrowed(PROGRAM_ICON.get().copied().unwrap_or(HECA_ICON))
}

/// **The window's icon** — `None` when the image cannot be read, which leaves the system's own.
pub(crate) fn window_icon() -> Option<winit::window::Icon> {
    let image = match image::load_from_memory(&encoded()) {
        Ok(image) => image,
        Err(error) => {
            eprintln!("[heca] the app icon could not be read: {error}");
            return None;
        }
    };
    let image = image.thumbnail(WINDOW_ICON_PX, WINDOW_ICON_PX).to_rgba8();
    let (width, height) = image.dimensions();
    winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
}

/// **macOS: the Dock shows the icon** while heca runs — a window icon is not used there, and an
/// unpackaged run would otherwise show the generic one. Main thread only, after the app launched.
#[cfg(target_os = "macos")]
pub(crate) fn show_in_dock() {
    use objc2::ClassType;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(&encoded());
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        eprintln!("[heca] the app icon could not be read for the Dock");
        return;
    };
    // SAFETY: on the main thread (checked above), with an image that lives as long as the app
    // keeps it.
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **heca's own logo reads as an icon**, at the window icon's size — a broken or missing image
    /// would fail here instead of leaving every window without an icon.
    #[test]
    fn hecas_logo_reads_as_a_window_icon() {
        let image = image::load_from_memory(HECA_ICON).expect("heca's logo is a readable image");
        let icon = image.thumbnail(WINDOW_ICON_PX, WINDOW_ICON_PX);
        assert_eq!(
            (icon.width(), icon.height()),
            (WINDOW_ICON_PX, WINDOW_ICON_PX)
        );
        assert!(
            winit::window::Icon::from_rgba(
                icon.to_rgba8().into_raw(),
                WINDOW_ICON_PX,
                WINDOW_ICON_PX
            )
            .is_ok()
        );
    }
}
