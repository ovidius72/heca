//! **A by-id action names a target the keyboard is not on.**
//!
//! `close_pane_by_id`, `rename_pane_by_id`, `delete_column`, `delete_workspace` exist precisely so
//! a *surface* can act on something other than what is focused: the sidebar lists every workspace,
//! a context menu is opened on the row under the pointer, the exposé shows the whole session, and
//! RPC names whatever it likes. An implementation that searches only the active workspace answers
//! correctly for the one case where the by-id form was not needed, and silently does nothing for
//! every case where it was.
//!
//! That is what happened: `handle_close_pane_by_id` looked in `active_workspace_mut()` alone, so
//! deleting a pane from the exposé's first row worked and from its second row did nothing at all —
//! no error, no log, the confirm dialog accepted and the pane still there (Antonio, driving,
//! 2026-08-11).
//!
//! A **lint**, because the thing it guards cannot be unit-tested: every one of these handlers takes
//! `&mut AppState`, which needs a window, so there is no headless call to make. It reads the source
//! the way `pointer_funnel.rs` reads the event loop.

use std::path::{Path, PathBuf};

fn handlers_rs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers.rs")
}

/// The body of `fn <name>(`, up to the closing brace at column 0.
fn fn_body(src: &str, name: &str) -> Option<String> {
    let at = src.find(&format!("fn {name}("))?;
    let rest = &src[at..];
    let end = rest.find("\n}\n").map(|e| e + 2).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

#[test]
fn closing_a_pane_by_id_searches_every_workspace() {
    let src = std::fs::read_to_string(handlers_rs()).expect("read the handlers");
    let body = fn_body(&src, "handle_close_pane_by_id").expect("the handler is still there");

    assert!(
        !body.contains("active_workspace_mut()") && !body.contains("active_workspace()"),
        "`handle_close_pane_by_id` only looks in the ACTIVE workspace.\n\
         A by-id action exists to name a pane the keyboard is not on — from the sidebar, a context \
         menu, the exposé or RPC — so restricting it to the active workspace makes it do nothing, \
         silently, for exactly the callers that need it.\n\
         Use `crate::app::mutations::close_pane_by_id_anywhere`, which already searches every \
         workspace and tidies up behind it.",
    );
    assert!(
        body.contains("close_pane_by_id_anywhere"),
        "the handler no longer delegates to the one implementation of this act — if that moved, \
         move this guard with it rather than deleting it",
    );
}

/// The same act must not grow a second implementation beside the shared one. Two of these existed:
/// one for a shell exiting on its own (correct, searched everywhere) and one for the user asking
/// (searched the active workspace). The user-facing one was the poorer, and nothing failed.
#[test]
fn there_is_one_implementation_of_closing_a_pane_by_id() {
    let src = std::fs::read_to_string(handlers_rs()).expect("read the handlers");
    let body = fn_body(&src, "handle_close_pane_by_id").expect("the handler is still there");

    assert!(
        !body.contains("floating_panes.iter().position"),
        "`handle_close_pane_by_id` has grown its own pane search again.\n\
         Removing a pane — tiled or floating — plus its backend, its search state and the workspace \
         it empties, is one act with one implementation (`close_pane_by_id_anywhere`). A second copy \
         drifts from the first where no test can see it.",
    );
}
