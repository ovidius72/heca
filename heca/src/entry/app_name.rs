//! **The name of the program running heca** — `heca`, or a program built on it (heca-pro is
//! "Heca Pro"). Fixed by the program, given once to [`run`](crate::run); not a setting.
//!
//! It is the window's title before it attaches to a session, and the `{app}` of the title formats
//! (`[settings.session_text] window_title`).

use std::sync::OnceLock;

static NAME: OnceLock<String> = OnceLock::new();

/// Remember the program's name — once, from [`run`](crate::run); a second call is ignored.
pub(crate) fn set(name: String) {
    let _ = NAME.set(name);
}

/// The program's name, as given to [`run`](crate::run). Empty only before `run` (tests).
pub(crate) fn get() -> &'static str {
    NAME.get().map_or("", String::as_str)
}
