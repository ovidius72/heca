//! What each mode searches, and what entering a mode asks for — through the real palette, built the
//! way `standard_palette` builds it from the one list of modes.

use super::*;
use crate::notification::{NotificationDraft, NotificationRecord, NotificationSource, NotificationStore};
use crate::server::sessions::{SessionInfo, SessionStatus};
use heca_grid_ui::{Event, LayoutEngine, Size, WidgetIntent};
use std::{cell::RefCell, rc::Rc, time::{Duration, Instant, SystemTime}};

type Log<T> = Rc<RefCell<Vec<T>>>;

/// A palette over the given session and notification rows, plus the ids chosen and the actions the
/// modes fired on being entered. `answered` is as for [`entering`].
fn open(
    sessions: &[PaletteEntry],
    notifications: &[PaletteEntry],
    answered: bool,
) -> (CommandPalette, Log<String>, Log<WmAction>) {
    let session = heca_core::layout::testing::Windowed::new(Size::new(800.0, 600.0), 1.0);
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let owners = Default::default();
    let shortcuts = crate::chrome::ActionShortcuts::default();
    let programs = heca_config::programs::ProgramsConfig::default();
    let panes = crate::chrome::SharedChromeState::new(280.0, 260.0).panes;
    let sources = modes::RowSources {
        sessions,
        follow: &[],
        notifications,
        catalog: &catalog,
        owners: &owners,
        shortcuts: &shortcuts,
        layout: session.l(),
        programs: &programs,
        panes: &panes,
        add_process_name: false,
        last_focused: None,
        last_visited: None,
    };
    let chosen: Log<String> = Default::default();
    let fired: Log<WmAction> = Default::default();
    let emit = {
        let fired = fired.clone();
        ChromeIntentEmitter::of(InteractionSource::Keyboard, move |_, intent| {
            if let InteractionIntent::ActivateAction(action) = intent {
                fired.borrow_mut().push(action);
            }
        })
    };
    let (palette, _) = modes::populate(
        CommandPalette::new(),
        modes::MODES,
        &sources,
        |row| {
            let (chosen, id) = (chosen.clone(), row.id.clone());
            row_command(row, 0, move || chosen.borrow_mut().push(id.clone()))
        },
        entering(&emit, answered),
    );
    (palette.default_open(true), chosen, fired)
}

/// Type `text` the way a window does: the edit is picked up by the next layout.
fn type_into(palette: &mut CommandPalette, text: &str) {
    for letter in text.chars() {
        heca_grid_ui::dispatch(palette, &Event::TextInput(letter.to_string()));
        LayoutEngine::new().compute(palette, Size::new(1200.0, 800.0));
    }
}

fn choose(palette: &mut CommandPalette) {
    heca_grid_ui::dispatch(palette, &Event::Widget(WidgetIntent::Activate));
}

fn session(id: u64, name: &str, folder: &str) -> SessionInfo {
    SessionInfo {
        id: heca_core::layout::SessionId(id),
        name: name.into(),
        folder: Some(folder.into()),
        label: None,
        status: SessionStatus::Active,
        windows: 1,
        agents: 0,
    }
}

fn session_rows(sessions: &[SessionInfo]) -> Vec<PaletteEntry> {
    let current = session(99, "current", "/elsewhere");
    sessions::entries(sessions, &current, &heca_config::settings::SessionText::default())
}

/// Notifications as the server would list them: each `(title, body)` raised a millisecond apart.
fn history(raised: &[(&str, &str)]) -> Vec<NotificationRecord> {
    let start = Instant::now();
    let mut store = NotificationStore::with_capacity(10, 10);
    for (i, (title, body)) in raised.iter().enumerate() {
        store
            .push(
                NotificationDraft::new(*title).body(*body).source(NotificationSource::app("test")),
                start + Duration::from_millis(i as u64),
            )
            .unwrap();
    }
    store.history_records_for(1, start + Duration::from_secs(1), SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000))
}

#[test]
fn a_notification_is_found_by_a_word_only_its_body_has() {
    let rows = notifications::entries(&history(&[("Restore failed", "the folder moved away"), ("Saved", "ok")]), 1_000_000);
    let (mut palette, chosen, _) = open(&[], &rows, false);
    type_into(&mut palette, "!moved");
    choose(&mut palette);
    assert_eq!(*chosen.borrow(), ["notification:1"], "the title has no `moved`; the body does");
}

#[test]
fn a_title_match_ranks_above_a_body_only_match() {
    // `disk` is the title of the older notification and only the body of the newer one.
    let rows = notifications::entries(
        &history(&[("Disk full", "clear some space"), ("Backup", "the disk is almost full")]),
        1_000_000,
    );
    let (mut palette, chosen, _) = open(&[], &rows, false);
    type_into(&mut palette, "!disk");
    choose(&mut palette);
    assert_eq!(*chosen.borrow(), ["notification:1"], "newest first would pick the body match (2); the title match leads");
}

#[test]
fn a_session_is_found_by_its_folder_and_a_name_match_leads() {
    let rows = session_rows(&[session(1, "api", "/work/billing"), session(2, "billing", "/work/other")]);
    let (mut palette, chosen, _) = open(&rows, &[], false);
    type_into(&mut palette, "#work/bil");
    choose(&mut palette);
    assert_eq!(*chosen.borrow(), ["session:1"], "found by a folder the name does not contain");

    let (mut palette, chosen, _) = open(&rows, &[], false);
    type_into(&mut palette, "#billing");
    choose(&mut palette);
    assert_eq!(*chosen.borrow(), ["session:2"], "the name match leads the folder match");
}

#[test]
fn an_action_is_not_found_by_its_description() {
    // `notification_history` says "newest first" in its description and nowhere in its name.
    let (mut palette, chosen, _) = open(&[], &[], false);
    type_into(&mut palette, "newest first");
    choose(&mut palette);
    assert!(
        chosen.borrow().iter().all(|id| id != "notification_history"),
        "the command mode matches names only: {:?}",
        chosen.borrow()
    );
    let (mut palette, chosen, _) = open(&[], &[], false);
    type_into(&mut palette, "notification hist");
    choose(&mut palette);
    assert_eq!(*chosen.borrow(), ["notification_history"], "by its name it is found");
}

#[test]
fn typing_the_sigil_asks_the_server_for_a_fresh_history_every_time_it_is_entered() {
    let (mut palette, _, fired) = open(&[], &[], false);
    let palette = &mut palette;
    assert!(fired.borrow().is_empty(), "the command list is not the notifications mode");
    type_into(palette, "!");
    assert_eq!(*fired.borrow(), [WmAction::NotificationHistory], "typing `!` asks");
    type_into(palette, "x");
    assert_eq!(fired.borrow().len(), 1, "typing inside the mode does not ask again");
    heca_grid_ui::dispatch(palette, &Event::Widget(WidgetIntent::EditDeleteToLineStart));
    LayoutEngine::new().compute(palette, Size::new(1200.0, 800.0));
    type_into(palette, "!");
    assert_eq!(fired.borrow().len(), 2, "entering it again asks again");
}

#[test]
fn a_palette_opened_in_the_mode_asks_unless_it_was_built_from_the_answer() {
    for (answered, asks) in [(false, 1), (true, 0)] {
        let rows = notifications::entries(&history(&[("Saved", "ok")]), 1_000_000);
        let (palette, _, fired) = open(&[], &rows, answered);
        let _in_mode = palette.query_in(Some(modes::MODE_NOTIFICATION), "");
        assert_eq!(fired.borrow().len(), asks, "answered={answered}");
    }
}
