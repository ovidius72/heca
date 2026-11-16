//! Notification-history rows: what the server remembers, newest first.

use super::*;
use crate::notification::NotificationRecord;

/// One row per remembered notification, and one more per action it **still** offers.
///
/// `records` arrive newest first and the order is kept. The row shows the level as the toast's own
/// glyph, the title as its label, the whole body as its description (the palette wraps the selected
/// row's), and the level, source, age and standing as the detail line. A notification that is still
/// on screen and has actions also gets a `"Title › Label"` row for each — the way an action with
/// several choices is offered elsewhere in this palette — which fires the same relay a toast's own
/// button does. A dismissed notification has no actions to offer (the server leaves them out), so it
/// only shows; choosing it closes the palette.
///
/// `now` is the wall clock in whole seconds since the Unix epoch, given by the caller so the rows
/// are a pure function of their inputs.
pub(super) fn entries(records: &[NotificationRecord], now: u64) -> Vec<PaletteEntry> {
    let mut rows = Vec::new();
    for record in records {
        let id = record.view.id.as_u64();
        let row = |id: String, label: String, description: String, intent| PaletteEntry {
            id,
            // A notification id lives for one server run and a title is not an identity, so
            // nothing is counted against it.
            rank_id: None,
            pane: None,
            current: false,
            preselect: false,
            label,
            description,
            description_path: false,
            detail: Some(detail_line(record, now)),
            icon: Some(record.view.severity.as_toast_severity().default_glyph()),
            shortcuts: Vec::new(),
            intent,
        };
        rows.push(row(
            format!("notification:{id}"),
            record.view.title.clone(),
            record.view.body.clone().unwrap_or_default(),
            None,
        ));
        for action in &record.view.actions {
            rows.push(row(
                format!("notification:{id}:{}", action.intent.action),
                format!("{} › {}", record.view.title, action.label),
                String::new(),
                Some(InteractionIntent::ActivateAction(WmAction::NotificationActionRelay {
                    notification_id: id,
                    key: action.intent.action.clone(),
                })),
            ));
        }
    }
    rows
}

/// `just now · dismissed`, `2 min ago · visible · ×3` — what the icon, title and body do not already
/// say. The level is the icon's job; the source is named only when it is **not heca** (a plugin, a
/// provider, a pane); `×N` appears once the same notification was raised twice or more.
fn detail_line(record: &NotificationRecord, now: u64) -> String {
    let mut parts = Vec::new();
    if !record.source.is_heca() {
        parts.push(match record.source.id() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => record.source.kind().as_label().to_string(),
        });
    }
    parts.push(age(now.saturating_sub(record.raised_at)));
    parts.push(record.standing.word().to_string());
    if record.repeats >= 2 {
        parts.push(format!("×{}", record.repeats));
    }
    parts.join(" · ")
}

/// How long ago, in the coarsest unit that is still at least one.
fn age(seconds: u64) -> String {
    match seconds {
        0..60 => "just now".to_string(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::{NotificationAction, NotificationDraft, NotificationSource, NotificationStore};
    use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Scene, Size, Theme, WidgetIntent};
    use std::{cell::RefCell, rc::Rc, time::{Duration, Instant, SystemTime}};

    const WALL: u64 = 1_000_000;

    /// A store holding: `config` (raised first, then dismissed), `restore` (still on screen, with a
    /// Retry and an Open log action) — read five minutes after they were raised, as the server would.
    fn history() -> Vec<NotificationRecord> {
        let start = Instant::now();
        let mut store = NotificationStore::with_capacity(10, 10);
        let old = store
            .push(
                NotificationDraft::new("Config reloaded")
                    .source(NotificationSource::app("config"))
                    .body("3 settings changed"),
                start,
            )
            .unwrap()
            .notification_id();
        store
            .push(
                NotificationDraft::new("Restore failed")
                    .source(NotificationSource::app("restore"))
                    .severity(crate::notification::NotificationSeverity::Warning)
                    .body("the folder moved\nand was not found")
                    .action(NotificationAction::new("Retry", heca_view::Intent::new("restore.retry")))
                    .action(NotificationAction::new("Open log", heca_view::Intent::new("restore.log"))),
                start + Duration::from_millis(1),
            )
            .unwrap();
        store.dismiss(old, start + Duration::from_secs(1));
        store.history_records_for(
            1,
            start + Duration::from_secs(300),
            SystemTime::UNIX_EPOCH + Duration::from_secs(WALL),
        )
    }

    #[test]
    fn a_row_shows_the_level_title_whole_body_and_when_with_the_newest_first() {
        let rows = entries(&history(), WALL);
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Restore failed", "Restore failed › Retry", "Restore failed › Open log", "Config reloaded"],
        );
        let restore = &rows[0];
        assert_eq!(restore.description, "the folder moved\nand was not found", "the whole body, not a cut");
        assert_eq!(restore.detail.as_deref(), Some("5 min ago · visible"), "no level word, no source for heca");
        assert!(restore.icon.is_some(), "the level is the toast's own glyph");
        assert_eq!(rows[3].detail.as_deref(), Some("5 min ago · dismissed"));
        assert_ne!(restore.icon, rows[3].icon, "a warning does not look like an info");
    }

    #[test]
    fn the_detail_line_names_a_source_only_when_it_is_not_heca_and_counts_repeats() {
        let start = Instant::now();
        let mut store = NotificationStore::with_capacity(10, 10);
        let raise = |store: &mut NotificationStore, draft: NotificationDraft, at: u64| {
            store.push(draft, start + Duration::from_millis(at)).unwrap();
        };
        raise(&mut store, NotificationDraft::new("heca"), 0);
        raise(&mut store, NotificationDraft::new("ext").source(NotificationSource::new(
            crate::notification::NotificationSourceKind::Plugin, "git-watch")), 1);
        raise(&mut store, NotificationDraft::new("bare").source(NotificationSource::of(
            crate::notification::NotificationSourceKind::Pane)), 2);
        for at in 3..6 {
            raise(&mut store, NotificationDraft::new("reload").dedup_key("reload"), at);
        }
        let records = store.history_records_for(
            1,
            start + Duration::from_secs(120),
            SystemTime::UNIX_EPOCH + Duration::from_secs(WALL),
        );
        let detail = |title: &str| {
            let rows = entries(&records, WALL);
            rows.iter().find(|row| row.label == title).and_then(|row| row.detail.clone()).unwrap()
        };
        assert_eq!(detail("heca"), "2 min ago · visible", "heca itself is not named");
        assert_eq!(detail("ext"), "git-watch · 2 min ago · visible", "an extension is");
        assert_eq!(detail("bare"), "pane · 2 min ago · visible", "with no name, its kind");
        assert_eq!(detail("reload"), "2 min ago · visible · ×3", "three raises, one entry");
        let rows = entries(&records, WALL);
        assert_eq!(rows.iter().filter(|row| row.label == "reload").count(), 1);
    }

    #[test]
    fn only_what_is_still_on_screen_offers_its_actions_and_choosing_one_fires_the_toast_relay() {
        let rows = entries(&history(), WALL);
        let offered = |label: &str| rows.iter().find(|row| row.label == label).unwrap();

        assert!(offered("Restore failed").intent.is_none(), "the entry itself only shows");
        assert!(matches!(
            &offered("Restore failed › Open log").intent,
            Some(InteractionIntent::ActivateAction(WmAction::NotificationActionRelay { key, .. })) if key == "restore.log"
        ));
        assert!(rows.iter().all(|row| !row.label.starts_with("Config reloaded ›")), "a dismissed one has none");
        assert!(offered("Config reloaded").intent.is_none());
    }

    #[test]
    fn ages_read_in_the_coarsest_unit() {
        assert_eq!(age(5), "just now");
        assert_eq!(age(60), "1 min ago");
        assert_eq!(age(3_599), "59 min ago");
        assert_eq!(age(7_300), "2 h ago");
        assert_eq!(age(3 * 86_400), "3 d ago");
    }

    /// The real palette, built the way `standard_palette` builds it, opened on the mode.
    fn palette(named: bool) -> (CommandPalette, Rc<RefCell<Vec<String>>>) {
        let session = heca_core::layout::testing::Windowed::new(Size::new(800.0, 600.0), 1.0);
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let owners = Default::default();
        let shortcuts = crate::chrome::ActionShortcuts::default();
        let programs = heca_config::programs::ProgramsConfig::default();
        let panes = crate::chrome::SharedChromeState::new(280.0, 260.0).panes;
        let rows = entries(&history(), WALL);
        let sources = modes::RowSources {
            sessions: &[],
            follow: &[],
            notifications: &rows,
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
        let chosen: Rc<RefCell<Vec<String>>> = Default::default();
        let (palette, _) = modes::populate(CommandPalette::new(), modes::MODES, &sources, |row| {
            let chosen = chosen.clone();
            let id = row.id.clone();
            row_command(row, 0, move || chosen.borrow_mut().push(id.clone()))
        }, |_| Box::new(|| {}));
        // Opened on the mode by name (what the action does), or left empty for the sigil to be typed.
        let palette = if named { palette.query_in(Some(modes::MODE_NOTIFICATION), "") } else { palette };
        (palette.default_open(true), chosen)
    }

    fn type_into(palette: &mut CommandPalette, text: &str) {
        for letter in text.chars() {
            heca_grid_ui::dispatch(palette, &Event::TextInput(letter.to_string()));
        }
    }

    /// Every text the palette draws, in paint order.
    fn drawn(palette: &mut CommandPalette) -> Vec<String> {
        let size = Size::new(1200.0, 800.0);
        LayoutEngine::new().compute(palette, size);
        let theme = Theme::default();
        let mut scene = Scene::new();
        heca_grid_ui::Component::paint(&*palette, &mut PaintCx::new(&mut scene, &theme).with_viewport(size));
        scene
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_mode_lists_the_notifications_and_not_the_commands() {
        let (mut open, _) = palette(true);
        let texts = drawn(&mut open);
        for shown in ["Restore failed", "Config reloaded", "Restore failed › Retry"] {
            assert!(texts.iter().any(|text| text == shown), "{shown} is in the built tree, got {texts:?}");
        }
        assert!(texts.iter().all(|text| text != "Close"), "the command list is not on show: {texts:?}");

        // Typing the sigil by hand reaches the same list.
        let (mut typed, _) = palette(false);
        type_into(&mut typed, "!");
        assert!(drawn(&mut typed).iter().any(|text| text == "Config reloaded"));
    }

    #[test]
    fn searching_filters_the_entries_and_choosing_an_offered_action_runs_it() {
        let (mut open, chosen) = palette(false);
        type_into(&mut open, "!config");
        let texts = drawn(&mut open);
        assert!(texts.iter().any(|text| text == "Config reloaded"), "{texts:?}");
        assert!(texts.iter().all(|text| !text.starts_with("Restore failed")), "the other entries are filtered out: {texts:?}");

        let (mut open, chosen_action) = palette(false);
        type_into(&mut open, "!log");
        heca_grid_ui::dispatch(&mut open, &Event::Widget(WidgetIntent::Activate));
        assert!(chosen.borrow().is_empty());
        assert_eq!(chosen_action.borrow().len(), 1);
        assert!(chosen_action.borrow()[0].ends_with(":restore.log"), "{:?}", chosen_action.borrow());
    }
}
