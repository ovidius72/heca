//! Opening a link, and the check on which links may be opened.

use super::pick::begin_pick;
use crate::app::selection_model::SelectionOwner;
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

pub fn handle_follow_link(state: &mut AppState, _action: &WmAction) {
    let candidates = crate::app::terminal_host::collect_link_hints(state);
    begin_pick(state, InputMode::FollowLink { candidates });
}

/// Schemes we are willing to hand to the OS opener. OSC 8 links come from
/// terminal output, so we refuse anything that isn't a plain web/file/mail
/// resource (e.g. no `javascript:` / `data:`).
fn link_scheme_allowed(url: &str) -> bool {
    let scheme = url
        .trim()
        .split_once(':')
        .map(|(s, _)| s.to_ascii_lowercase());
    matches!(
        scheme.as_deref(),
        Some("http" | "https" | "mailto" | "file" | "ftp" | "ftps")
    )
}

/// Open `url` in the OS default handler, detached. Best-effort: a missing opener
/// or a spawn failure is logged (debug) and otherwise ignored.
fn open_url_in_os(url: &str) {
    use std::process::{Command, Stdio};
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd");
        // Empty title arg so a quoted URL isn't treated as the window title.
        c.args(["/C", "start", "", url]);
        c
    };
    let result = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if let Err(_e) = result {
        #[cfg(debug_assertions)]
        eprintln!("[heca] failed to open link {url}: {_e}");
    }
}

/// Open an OSC 8 hyperlink target. Shared by every surface (HintKey follow-link,
/// Cmd/Ctrl+click, selection-mode `O`, context menu, RPC).
pub fn handle_open_link(_state: &mut AppState, action: &WmAction) {
    let WmAction::OpenLink { url } = action else {
        return;
    };
    if link_scheme_allowed(url) {
        open_url_in_os(url);
    } else {
        #[cfg(debug_assertions)]
        eprintln!("[heca] refusing to open link with disallowed scheme: {url}");
    }
}

pub fn handle_open_link_at_caret(state: &mut AppState, _action: &WmAction) {
    let Some((owner, stable_row, col)) = state.selection.cursor_cell() else {
        return;
    };
    let SelectionOwner(terminal) = owner;
    let Some(url) =
        crate::app::terminal_host::hyperlink_uri_at_stable_cell(state, terminal, stable_row, col)
    else {
        return;
    };
    handle_open_link(state, &WmAction::OpenLink { url });
}

#[cfg(test)]
mod open_link_tests {
    use super::link_scheme_allowed;

    #[test]
    fn allows_web_file_mail_schemes() {
        for url in [
            "https://example.com",
            "http://example.com/path",
            "file:///Users/me/x.txt",
            "mailto:a@b.com",
            "ftp://host/f",
            "FTPS://host/f",
        ] {
            assert!(link_scheme_allowed(url), "should allow {url}");
        }
    }

    #[test]
    fn rejects_dangerous_or_schemeless() {
        for url in [
            "javascript:alert(1)",
            "data:text/html,<script>",
            "vbscript:x",
            "example.com",
            "",
        ] {
            assert!(!link_scheme_allowed(url), "should reject {url}");
        }
    }
}
