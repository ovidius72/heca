//! **The sidebar's views** — which list of lines its pane cards show, how a user switches between
//! them, and how the choice is kept between starts.
//!
//! - **Switched** by one action, `sidebar_pane_view`: `view=compact|normal|extended` sets one, no
//!   argument goes to the next. A key, the command palette and `heca rpc` reach it alike.
//! - **Heard** as `sidebar.pane_view.changed` (the store raises it), by a plugin or a script.
//! - **Remembered** in the data folder (`sidebar.json`): the view chosen last wins at the next start;
//!   with none, `[appearance.sidebar] pane_view` does. Changing `pane_view` in the file and
//!   reloading switches to it.

use heca_config::appearance::PaneView;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::app_state::AppState;

/// The name of the line that is the pane's title — the pip, the icon and the name.
pub(crate) const TITLE_LINE: &str = "title";

const FILE: &str = "sidebar.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Persisted {
    version: u32,
    pane_view: PaneView,
}

/// **`sidebar_pane_view`** — the named view, or the next one.
pub(crate) fn handle_sidebar_pane_view(
    state: &mut AppState,
    action: &crate::input::WmAction,
) -> crate::actions::ActionResult {
    let crate::input::WmAction::SidebarPaneView { view } = action else {
        return Ok(());
    };
    let view = view.unwrap_or_else(|| state.chrome_state.workspaces.pane_view().next());
    choose(state, view);
    Ok(())
}

/// **Show `view`, keep it** for the next start, **and say which it is** — one line in the bottom bar,
/// gone at the next key: three views that differ by a line or two are not told apart by eye.
fn choose(state: &mut AppState, view: PaneView) {
    state.chrome_state.workspaces.set_pane_view(view);
    state.status_note = Some(format!("Sidebar view: {}", view.name()));
    if let Some(path) = crate::state_file::data_path(FILE) {
        save(&path, view);
    }
    state.mark_full_redraw();
}

/// **Follow `[appearance.sidebar] pane_view`** — every sync. See [`decide`].
pub(crate) fn follow_configured_pane_view(state: &mut AppState) {
    let configured = state.appearance.sidebar.pane_view;
    let store = &state.chrome_state.workspaces;
    let kept = || crate::state_file::data_path(FILE).and_then(|path| load(&path));
    match decide(store.configured_pane_view(), configured, kept) {
        Follow::Start(view) => {
            store.set_configured_pane_view(configured);
            store.set_pane_view(view);
        }
        Follow::Switch(view) => {
            store.set_configured_pane_view(configured);
            choose(state, view);
        }
        Follow::Stay => {}
    }
}

/// What following the configured view does now.
#[derive(Debug, PartialEq)]
enum Follow {
    /// The first sync: show this view.
    Start(PaneView),
    /// `pane_view` changed in the file: switch to it, and keep it.
    Switch(PaneView),
    /// Nothing changed in the file: leave the view the user chose.
    Stay,
}

/// **The rule**: at the first sync the view kept from the last run wins over `pane_view`, and
/// `pane_view` stands in when none was kept. After that only a `pane_view` that changed in the file
/// (a reload) moves the cards — so a view chosen with the action is not undone every frame.
fn decide(
    before: Option<PaneView>,
    configured: PaneView,
    kept: impl FnOnce() -> Option<PaneView>,
) -> Follow {
    match before {
        None => Follow::Start(kept().unwrap_or(configured)),
        Some(before) if before != configured => Follow::Switch(configured),
        Some(_) => Follow::Stay,
    }
}

/// The view kept at `path`; `None` for a first run or a file heca cannot read.
fn load(path: &Path) -> Option<PaneView> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Persisted>(&text).ok())
        .filter(|kept| kept.version == VERSION)
        .map(|kept| kept.pane_view)
}

/// Keep `view` at `path`. A write that fails (a read-only disk) only costs the next start's view.
fn save(path: &Path, view: PaneView) {
    let _ = crate::state_file::write_json(
        path,
        &Persisted {
            version: VERSION,
            pane_view: view,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **At start the kept view wins, then only a changed `pane_view` moves the cards.**
    #[test]
    fn the_kept_view_wins_at_start_and_a_changed_config_switches() {
        use PaneView::*;
        assert_eq!(
            decide(None, Extended, || Some(Compact)),
            Follow::Start(Compact),
            "kept wins"
        );
        assert_eq!(
            decide(None, Normal, || None),
            Follow::Start(Normal),
            "none kept: the file's"
        );
        assert_eq!(
            decide(Some(Normal), Normal, || Some(Compact)),
            Follow::Stay,
            "unchanged"
        );
        assert_eq!(
            decide(Some(Normal), Extended, || None),
            Follow::Switch(Extended),
            "reload"
        );
    }

    /// **A view chosen is kept and read back**; no file, or one heca cannot read, is a first run.
    #[test]
    fn the_chosen_view_is_kept_and_read_back() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().join(FILE);
        assert_eq!(load(&path), None, "first run");
        save(&path, PaneView::Compact);
        assert_eq!(load(&path), Some(PaneView::Compact));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load(&path), None, "an unreadable file is a first run");
    }
}
