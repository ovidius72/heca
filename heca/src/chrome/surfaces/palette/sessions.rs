//! Session rows and the folder chooser, composed with the shared command palette.

use super::*;
use crate::server::sessions::{SessionInfo, SessionStatus};
use std::path::{Path, PathBuf};

pub(super) fn entries(
    sessions: &[SessionInfo],
    current: &SessionInfo,
    words: &heca_config::settings::SessionText,
) -> Vec<PaletteEntry> {
    sessions
        .iter()
        .filter(|session| session.id != current.id || current.status != SessionStatus::Terminated)
        .map(|session| {
            let session = if session.id == current.id { current } else { session };
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
                current: session.id == current.id,
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
    let palette = CommandPalette::new()
        .placeholder(&words.folder_placeholder)
        .search(search_model(state, "session-folder", state.settings.folder_search_case))
        .source(move |query| folder_commands(query, &base, &home, &words, &emit));
    open_command_palette(state, None, None, Some(palette));
}

fn folder_commands(query: &str, base: &Path, home: &Path,
    words: &heca_config::settings::SessionText, emit: &ChromeIntentEmitter) -> Vec<Command> {
    folders(query, base, home).into_iter().map(|(path, create)| {
        let path = path.to_string_lossy().into_owned();
        let shown = if query.starts_with('~') {
            home_relative_path(Path::new(&path))
        } else if !Path::new(query).is_absolute() {
            Path::new(&path).strip_prefix(base).unwrap_or(Path::new(&path)).display().to_string()
        } else {
            path.clone()
        };
        let shown = if shown.is_empty() { path.clone() } else { shown };
        let completion = format!("{}/", shown.trim_end_matches('/'));
        let label = if create { words.create_folder.replace("{path}", &shown) } else { shown };
        let emit = emit.clone();
        Command::new(label, move || {
            emit.fire(InteractionIntent::ActivateAction(WmAction::CreateSession {
                folder: path.clone(), name: None, label: None,
            }))
        }).completion(completion).fallback(create)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_rows_keep_name_path_and_configured_counts_separate() {
        use heca_core::layout::SessionId;
        let words = heca_config::settings::SessionText {
            counts: "views={windows}; agents={agents}".into(),
            unavailable: "offline".into(),
            ..Default::default()
        };
        let session = SessionInfo {
            id: SessionId(17), name: "A free session name".into(),
            folder: Some("/workspace/long/project/path".into()), label: Some("extension label".into()),
            status: SessionStatus::Active, windows: 2, agents: 3,
        };
        let rows = entries(std::slice::from_ref(&session), &session, &words);
        let row = &rows[0];
        assert_eq!(row.label, session.name);
        assert_eq!(row.description, "/workspace/long/project/path");
        assert!(row.description_path);
        assert_eq!(row.detail.as_deref(), Some("views=2; agents=3"));
        assert_eq!(row.rank_id.as_deref(), Some(session.name.as_str()));
        assert!(row.current);
        assert!(matches!(&row.intent, InteractionIntent::ActivateAction(WmAction::SwitchSession { name }) if name == &session.name));

        let stopped = SessionInfo { status: SessionStatus::Terminated, ..session };
        let other = SessionInfo { id: SessionId(99), ..stopped.clone() };
        let rows = entries(&[stopped], &other, &words);
        assert_eq!(rows[0].detail.as_deref(), Some("views=2; agents=3 / offline"));
        assert!(!rows[0].current);
    }

    #[test]
    fn a_terminated_current_session_is_not_offered_as_its_own_alternative() {
        use heca_core::layout::SessionId;
        let current = SessionInfo { id: SessionId(1), name: "Stopped".into(), folder: None,
            label: None, status: SessionStatus::Terminated, windows: 1, agents: 0 };
        let words = heca_config::settings::SessionText::default();
        assert!(entries(std::slice::from_ref(&current), &current, &words).is_empty());
        let stale = SessionInfo { status: SessionStatus::Active, ..current.clone() };
        assert!(entries(&[stale], &current, &words).is_empty(), "current state wins over a stale directory");
        let active = SessionInfo { id: SessionId(2), name: "Active".into(), status: SessionStatus::Active, ..current.clone() };
        let stopped = SessionInfo { id: SessionId(3), name: "Another stopped session".into(), ..current.clone() };
        let rows = entries(&[current.clone(), active.clone(), stopped.clone()], &current, &words);
        assert_eq!(rows.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(), [active.name.as_str(), stopped.name.as_str()]);
        let rows = entries(std::slice::from_ref(&active), &active, &words);
        assert_eq!(rows.len(), 1, "an active session can still be reattached after Detach");
    }

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

    #[test]
    fn a_new_folder_with_spaces_and_a_trailing_slash_remains_selectable_in_the_real_palette() {
        use heca_grid_ui::{Event, WidgetIntent};
        use std::{cell::RefCell, rc::Rc};
        let root = tempfile::tempdir().unwrap();
        let base = root.path().to_owned();
        let chosen: Rc<RefCell<Vec<InteractionIntent>>> = Default::default();
        let result = chosen.clone();
        let emit = ChromeIntentEmitter::of(InteractionSource::Keyboard, move |_, intent| result.borrow_mut().push(intent));
        let words = heca_config::settings::SessionText::default();
        let palette_base = base.clone();
        let mut palette = CommandPalette::new().source(move |query|
            folder_commands(query, &palette_base, &palette_base, &words, &emit)).default_open(true);
        heca_grid_ui::dispatch(&mut palette, &Event::TextInput("new project/".into()));
        heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Complete));
        assert!(chosen.borrow().is_empty());
        assert!(!base.join("new project").exists());
        heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
        assert!(matches!(&chosen.borrow()[0], InteractionIntent::ActivateAction(WmAction::CreateSession { folder, .. })
            if std::path::Path::new(folder) == base.join("new project")));
    }

    #[test]
    fn folder_completion_fuzzily_finds_existing_names_despite_completed_capitals() {
        use heca_grid_ui::{Event, WidgetIntent, search::{MatchCase, SearchModel}};
        use std::{cell::RefCell, rc::Rc};
        let root = tempfile::tempdir().unwrap();
        let base = root.path().to_owned();
        std::fs::create_dir_all(base.join("Projects/HeCa_Project")).unwrap();
        std::fs::create_dir_all(base.join("Projects/Other")).unwrap();
        for query in ["Projects/hcp", "Projects/HCP"] {
            let chosen: Rc<RefCell<Vec<InteractionIntent>>> = Default::default();
            let result = chosen.clone();
            let emit = ChromeIntentEmitter::of(InteractionSource::Keyboard, move |_, intent| result.borrow_mut().push(intent));
            let words = heca_config::settings::SessionText::default();
            let palette_base = base.clone();
            let mut palette = CommandPalette::new().search(SearchModel::detached("folder").case(MatchCase::Insensitive))
                .source(move |query| folder_commands(query, &palette_base, &palette_base, &words, &emit)).default_open(true);
            heca_grid_ui::dispatch(&mut palette, &Event::TextInput(query.into()));
            heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Complete));
            heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
            assert!(matches!(&chosen.borrow()[0], InteractionIntent::ActivateAction(WmAction::CreateSession { folder, .. })
                if std::path::Path::new(folder) == base.join("Projects/HeCa_Project")), "query {query}");
        }
    }
}
