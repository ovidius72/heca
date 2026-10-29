//! Session rows and the folder chooser, composed with the shared command palette.

use super::*;
use crate::server::sessions::{SessionInfo, SessionStatus};
use std::path::{Path, PathBuf};

pub(super) fn entries(
    sessions: &[SessionInfo],
    current: heca_core::layout::SessionId,
    words: &heca_config::settings::SessionText,
) -> Vec<PaletteEntry> {
    sessions
        .iter()
        .map(|session| {
            let counts = words
                .counts
                .replace("{windows}", &session.windows.to_string())
                .replace("{agents}", &session.agents.to_string());
            let folder = session
                .folder
                .as_deref()
                .map(home_relative_path)
                .unwrap_or_default();
            PaletteEntry {
                id: format!("session:{}", session.id.0),
                rank_id: Some(session.name.clone()),
                pane: None,
                current: session.id == current,
                preselect: false,
                label: session.name.clone(),
                description: folder,
                description_path: true,
                detail: Some(format!(
                    "{counts}{}",
                    if session.status == SessionStatus::Terminated {
                        format!(" / {}", words.unavailable)
                    } else {
                        String::new()
                    }
                )),
                icon: None,
                shortcuts: Vec::new(),
                intent: InteractionIntent::ActivateAction(WmAction::SwitchSession {
                    name: session.name.clone(),
                }),
            }
        })
        .collect()
}

/// Candidate collection is separate from matching: the palette owns fuzzy ranking and input.
fn folders(query: &str, base: &Path, home: &Path) -> Vec<(PathBuf, bool)> {
    if query.is_empty() {
        return vec![(home.to_owned(), false)];
    }
    let Ok(target) = crate::server::sessions::resolve_folder(Some(query), base) else {
        return Vec::new();
    };
    let parent = if query.ends_with('/') {
        target.as_path()
    } else {
        target.parent().unwrap_or(base)
    };
    let mut rows = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| (path, false))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    if target.is_dir() && !rows.iter().any(|(path, _)| *path == target) {
        rows.insert(0, (target, false));
    } else if !target.exists() {
        rows.push((target, true));
    }
    rows
}

pub(crate) fn open_session_folder(state: &mut AppState) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let base = state
        .layout()
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .and_then(|pane| pane.runtime.cwd.clone())
        .unwrap_or_else(|| home.clone());
    let words = state.settings.session_text.clone();
    let emit = ChromeIntentEmitter::new(&state.event_proxy, InteractionSource::Keyboard);
    let id = OverlayId(state.layers.reserve_id());
    let palette = CommandPalette::new()
        .placeholder(&words.folder_placeholder)
        .search(heca_grid_ui::search::SearchModel::new(
            "session-folder",
            state.search_store.clone(),
        ))
        .source(move |query| {
            folders(query, &base, &home)
                .into_iter()
                .map(|(path, create)| {
                    let path = path.to_string_lossy().into_owned();
                    let shown = if query.starts_with('~') {
                        home_relative_path(Path::new(&path))
                    } else if !Path::new(query).is_absolute() {
                        Path::new(&path)
                            .strip_prefix(&base)
                            .unwrap_or(Path::new(&path))
                            .display()
                            .to_string()
                    } else {
                        path.clone()
                    };
                    let shown = if shown.is_empty() {
                        path.clone()
                    } else {
                        shown
                    };
                    let label = if create {
                        words.create_folder.replace("{path}", &shown)
                    } else {
                        shown
                    };
                    let emit = emit.clone();
                    Command::new(label, move || {
                        emit.fire(InteractionIntent::ActivateAction(WmAction::CreateSession {
                            folder: path.clone(),
                            name: None,
                            label: None,
                        }))
                    })
                })
                .collect()
        })
        .default_open(true);
    let open = palette.open_signal();
    let close_emit = ChromeIntentEmitter::new(&state.event_proxy, InteractionSource::Keyboard);
    let close = id.closer(&close_emit);
    create_effect(move |_| {
        if !open.get() {
            close();
        }
    });
    state.layers.insert(
        id.0,
        state.layers.current(),
        LayerKind::OnDemand,
        Box::new(palette),
        &mut state.window_root,
    );
    state.overlays.on_resolve(id, |_, _, _| {});
    state.needs_redraw = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_missing_folder_is_offered_explicitly_and_never_created_by_completion() {
        let root = tempfile::tempdir().unwrap();
        let rows = folders("new project", root.path(), root.path());
        assert!(rows.contains(&(root.path().join("new project"), true)));
        assert!(!root.path().join("new project").exists());
        assert_eq!(
            folders("", root.path(), root.path()),
            [(root.path().to_owned(), false)]
        );
    }
}
