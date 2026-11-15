//! The saved-sessions file on its own: what it reads, what it refuses, and when it is written.

use super::file::VERSION;
use super::*;
use crate::app::backend_store::{Program, TerminalOwner, TerminalSpec};
use crate::entry::service::SessionKey;
use crate::server::identity::{ServerIdentity, ServerKind};
use crate::server::sessions::{RecordedTerminal, SessionStatus};
use heca_core::layout::{Pane, PaneId, Session};

fn saved(id: u64, name: &str) -> SavedSession {
    let mut session = Session::new(SessionId(id), Default::default());
    let pane = PaneId(session.next_id());
    session.add_column_with(0, 0, Pane::new(pane, "shell"));
    SavedSession {
        id: SessionId(id),
        key: SessionKey::generate(),
        name: name.into(),
        folder: Some(std::env::temp_dir().join(name)),
        label: None,
        status: SessionStatus::Active,
        content: session.content(),
        focus: None,
        view: None,
        terminals: Vec::new(),
    }
}

fn file(sessions: Vec<SavedSession>) -> Saved {
    Saved { next_session: 9, recent: sessions.iter().map(|s| s.id).collect(), sessions }
}

fn terminal(owner: TerminalOwner) -> RecordedTerminal {
    RecordedTerminal {
        display_title: None,
        running: true,
        launch: None,
        owner,
        spec: TerminalSpec { program: Program::Shell, cwd: None, grid: (80, 24) },
        agent: None,
    }
}

#[test]
fn what_is_written_is_read_back_with_everything_the_server_keeps() {
    let (one, two) = (saved(1, "one"), saved(2, "two"));
    let (text, left_out) = file(vec![one.clone(), two.clone()]).encode();
    assert!(left_out.is_empty());
    let (back, problems) = Saved::decode(&text).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!((back.next_session, back.recent), (9, vec![SessionId(1), SessionId(2)]));
    let keys: Vec<_> = back.sessions.iter().map(|s| (s.id, s.key, s.name.clone(), s.folder.clone())).collect();
    assert_eq!(keys, [one, two].map(|s| (s.id, s.key, s.name, s.folder)));
    assert_eq!(back.sessions[0].content, saved(1, "one").content);
}

#[test]
fn a_file_that_is_not_json_or_is_another_version_is_refused_whole_with_the_reason() {
    assert!(Saved::decode("{ not json").unwrap_err().contains("not readable"));
    let newer = format!(r#"{{"version": {}, "next_session": 1, "sessions": []}}"#, VERSION + 1);
    assert!(Saved::decode(&newer).unwrap_err().contains(&format!("version {}", VERSION + 1)));
    assert!(Saved::decode(r#"{"version": 1}"#).unwrap_err().contains("not readable"), "no sessions list at all");
}

#[test]
fn a_session_that_cannot_be_rebuilt_is_left_out_and_the_others_are_kept() {
    let mut relative = saved(2, "relative");
    relative.folder = Some("not/absolute".into());
    let mut empty = saved(3, "empty");
    empty.content.workspaces.clear();
    let mut twice = saved(4, "twice");
    let again = twice.content.workspaces[0].columns[0].panes[0].clone();
    twice.content.workspaces[0].columns[0].panes.push(again);
    let nameless = saved(5, "  ");
    let same_number = saved(1, "same number");
    let same_name = saved(6, "good");
    let good = saved(1, "good");
    let (text, _) = file(vec![good, relative, empty, twice, nameless, same_number, same_name]).encode();
    let (back, problems) = Saved::decode(&text).unwrap();
    assert_eq!(back.sessions.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["good"]);
    assert_eq!(problems.len(), 6, "{problems:?}");
    for (problem, word) in problems.iter().zip(["absolute", "no workspace", "twice", "name", "repeats", "repeats"]) {
        assert!(problem.contains(word), "{problem} should say {word}");
    }
}

#[test]
fn an_unreadable_entry_is_reported_by_its_place_and_the_rest_still_come_back() {
    let (text, _) = file(vec![saved(1, "a"), saved(2, "b")]).encode();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["sessions"][0] = serde_json::json!({"id": "not a number"});
    let (back, problems) = Saved::decode(&value.to_string()).unwrap();
    assert_eq!(back.sessions.len(), 1);
    assert!(problems[0].starts_with("session 1 is not readable"), "{problems:?}");
}

#[test]
fn terminals_of_panes_the_content_no_longer_has_are_dropped_and_named_ones_kept() {
    let mut session = saved(1, "a");
    let pane = session.panes().next().unwrap();
    session.terminals = vec![
        terminal(TerminalOwner::Pane(pane)),
        terminal(TerminalOwner::Pane(PaneId(999))),
        terminal(TerminalOwner::Named("demo.logs".into())),
    ];
    let (text, _) = file(vec![session]).encode();
    let (back, _) = Saved::decode(&text).unwrap();
    let owners: Vec<_> = back.sessions[0].terminals.iter().map(|t| t.owner.clone()).collect();
    assert_eq!(owners, [TerminalOwner::Pane(pane), TerminalOwner::Named("demo.logs".into())]);
}

/// A folder that is not text has no JSON form: that one session is left out of the file, said once,
/// and the others are still written.
#[cfg(unix)]
#[test]
fn a_session_with_no_json_form_does_not_stop_the_others_being_written() {
    use std::os::unix::ffi::OsStringExt;
    let mut odd = saved(2, "odd");
    odd.folder = Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff])));
    let (text, left_out) = file(vec![saved(1, "fine"), odd]).encode();
    assert_eq!(left_out.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [SessionId(2)]);
    assert_eq!(Saved::decode(&text).unwrap().0.sessions.len(), 1);
}

fn settings(text: &str) -> heca_config::settings::SettingsConfig {
    heca_config::loader::config_from_layers(text).unwrap().settings
}

fn store(root: &Path, text: &str) -> Option<SessionStore> {
    let identity = ServerIdentity::claim_in(ServerKind::Background, Some(root.join("servers")));
    SessionStore::open(&identity, &settings(text))
}

#[test]
fn a_write_is_due_a_delay_after_the_first_change_and_only_then() {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let mut store = store(root.path(), "[settings]\nsession_save_delay_ms = 1000").unwrap();
    let start = Instant::now();
    assert!(!store.take_due(start), "nothing was said to have changed");
    assert_eq!(store.next_wake(), None);
    store.arm(start);
    store.arm(start + Duration::from_millis(900));
    assert_eq!(store.next_wake(), Some(start + Duration::from_secs(1)), "a later word does not move the time");
    assert!(!store.take_due(start + Duration::from_millis(999)));
    assert!(store.take_due(start + Duration::from_secs(1)));
    assert!(!store.take_due(start + Duration::from_secs(2)), "taking it disarms it");
}

#[test]
fn a_file_the_user_deleted_is_written_again_even_when_nothing_changed() {
    let root = tempfile::tempdir().unwrap();
    let mut store = store(root.path(), "").unwrap();
    let all = file(vec![saved(1, "a")]);
    store.save(&all);
    let path = store.path.clone();
    let first = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    store.save(&all);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
}

#[test]
fn a_server_with_no_folder_of_its_own_keeps_nothing_and_does_not_stop() {
    let held = tempfile::tempdir().unwrap();
    let first = ServerIdentity::claim_in(ServerKind::Background, Some(held.path().join("servers")));
    let second = ServerIdentity::claim_in(ServerKind::Background, Some(held.path().join("servers")));
    assert!(SessionStore::open(&first, &settings("")).is_some());
    assert!(SessionStore::open(&second, &settings("")).is_none(), "the folder is the first server's");
}
