//! How many notifications the server remembers: from the config at start, and again on reload.

use super::*;
use crate::notification::NotificationDraft;
use std::time::Duration;

fn config(text: &str) -> heca_config::loader::AppConfig {
    heca_config::loader::AppConfig {
        config: heca_config::loader::config_from_layers(text).unwrap(),
        theme: Default::default(),
    }
}

fn titles(server: &ServerState) -> Vec<String> {
    server.notification_history_for(1).into_iter().map(|record| record.view.title).collect()
}

/// Raise `titles` and dismiss them all, so each one is history.
fn raise_and_dismiss(server: &mut ServerState, titles: &[&str], now: Instant) {
    for title in titles {
        server.execute(ServerAction::Raise(NotificationDraft::new(*title)), now);
    }
    server.execute(ServerAction::DismissAll, now);
}

#[test]
fn the_server_starts_with_the_configured_history_size() {
    let mut server = ServerState::from_config(
        &config("[settings.notification_system]\nhistory_limit = 2\n"),
        ServerWaker::new(|| {}),
    );
    raise_and_dismiss(&mut server, &["a", "b", "c"], Instant::now());
    assert_eq!(titles(&server).len(), 2, "the config's 2, not a number written in the code");

    let mut server = ServerState::from_config(&config(""), ServerWaker::new(|| {}));
    let many: Vec<String> = (0..120).map(|n| n.to_string()).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    // The default is 100, and the stack shows only a few at once, so raise in batches.
    for batch in many.chunks(5) {
        raise_and_dismiss(&mut server, batch, Instant::now());
    }
    assert_eq!(titles(&server).len(), 100, "the default is 100");
}

#[test]
fn a_reload_changes_the_history_size_and_zero_turns_it_off() {
    let mut server = ServerState::for_tests(Default::default());
    let now = Instant::now();
    raise_and_dismiss(&mut server, &["a", "b", "c"], now);
    assert_eq!(titles(&server).len(), 3);

    // What a reload hands over, read from the same config the start-up read.
    let reload = |text: &str| {
        ServerAction::Configure(NotificationSettings::from(
            &config(text).config.settings.notification_system,
        ))
    };
    server.execute(reload("[settings.notification_system]\nhistory_limit = 1\n"), now);
    assert_eq!(titles(&server), ["c"], "the oldest were forgotten as soon as the config said so");

    server.execute(reload("[settings.notification_system]\nhistory_limit = 0\n"), now);
    assert!(titles(&server).is_empty(), "0 keeps nothing");

    // A new notification dismissed under 0 is not kept either.
    raise_and_dismiss(&mut server, &["d"], now + Duration::from_secs(1));
    assert!(titles(&server).is_empty());

    server.execute(reload("[settings.notification_system]\nhistory_limit = 5\n"), now);
    raise_and_dismiss(&mut server, &["e"], now + Duration::from_secs(2));
    assert_eq!(titles(&server), ["e"], "and it keeps again once the limit is raised");
}

/// **A button made with `NotificationAction::run` runs its closure on the server**, whichever
/// window asked; the closure lives as long as the notification does — a dropped notification never
/// keeps it, and one that leaves the screen lets it go.
#[test]
fn a_run_button_runs_its_closure_and_lives_only_as_long_as_its_notification() {
    use crate::notification::NotificationAction;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let now = Instant::now();
    let pressed = Arc::new(AtomicUsize::new(0));
    let button = || {
        let pressed = pressed.clone();
        NotificationAction::run("Retry", move || {
            pressed.fetch_add(1, Ordering::SeqCst);
        })
    };
    let holders = |pressed: &Arc<AtomicUsize>| Arc::strong_count(pressed) - 1;

    let mut server = ServerState::from_config(&config(""), ServerWaker::new(|| {}));
    let action = button();
    let key = action.intent.action.clone();
    server.execute(ServerAction::Raise(NotificationDraft::new("Agent finished").action(action)), now);
    let id = server.notifications.visible_views()[0].id;
    assert!(server.notifications.visible_views()[0].action(&key).is_some_and(|a| a.runs), "every window knows it runs");
    server.execute(ServerAction::Run(id, key.clone()), now);
    assert_eq!(pressed.load(Ordering::SeqCst), 1, "pressed once, ran once");
    assert_eq!(holders(&pressed), 1, "the store holds the closure while the notification is up");
    server.execute(ServerAction::DismissAll, now);
    assert_eq!(holders(&pressed), 0, "gone from the screen, the closure is let go (even kept in the history)");
    server.execute(ServerAction::Run(id, key), now);
    assert_eq!(pressed.load(Ordering::SeqCst), 1, "an archived button cannot be pressed");

    // A notification the server drops (`mode = "none"`) never keeps its closure.
    let mut silent = ServerState::from_config(&config("[settings.notification_system]\nmode = \"none\"\n"), ServerWaker::new(|| {}));
    silent.execute(ServerAction::Raise(NotificationDraft::new("dropped").action(button())), now);
    assert_eq!(holders(&pressed), 0, "a dropped notification drops its closure");

    // Two buttons on one notification have their own names.
    assert_ne!(button().intent.action, button().intent.action);
}
