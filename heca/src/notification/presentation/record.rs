use super::{NotificationView, entry_view};
use crate::notification::{AppNotification, NotificationArchiveCause, NotificationSource};
use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Where a remembered notification stands: still on a card, or gone and why.
///
/// Queued notifications have no standing yet — they were never shown — so they are not in the
/// history at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationStanding {
    /// Still on screen.
    Visible,
    /// Left the screen: dismissed by the user, expired, or resolved by heca.
    Archived(NotificationArchiveCause),
}

impl NotificationStanding {
    /// The standing as a word: `visible`, `dismissed`, `expired` or `resolved`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Archived(NotificationArchiveCause::Dismissed) => "dismissed",
            Self::Archived(NotificationArchiveCause::Expired) => "expired",
            Self::Archived(NotificationArchiveCause::Resolved) => "resolved",
        }
    }
}

/// **One line of the notification history, as plain data** — what the server sends a window that
/// asks for it ([`ServerMsg::NotificationHistory`](crate::server::wire::ServerMsg::NotificationHistory)).
///
/// The notification itself is the same [`NotificationView`] a toast is drawn from (level, title,
/// full body, actions); the record adds what only the history needs: where it came from, when it
/// was raised and whether it is still up. A window draws it with its own widgets; the server owns
/// the list, so every window — including one that has just reattached — reads the same one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationRecord {
    /// The notification's own presentation data. Its actions are present **only while it is
    /// [`Visible`](NotificationStanding::Visible)**: a dismissed notification has nothing left to
    /// act on, and the server is the one that knows.
    pub view: NotificationView,
    /// Where it came from.
    pub source: NotificationSource,
    /// When it was raised, in whole seconds since the Unix epoch — wall-clock time, because the
    /// server's `Instant` means nothing to a window.
    pub raised_at: u64,
    /// Still on screen, or gone and why.
    pub standing: NotificationStanding,
    /// How many times it was raised: more than one when notifications with the same dedup key
    /// replaced it in place, so a window shows `×3` instead of three identical lines.
    #[serde(default = "one")]
    pub repeats: u32,
}

fn one() -> u32 {
    1
}

/// The history line for a retained notification, or `None` for one that is only queued.
///
/// `now` and `wall` are the same moment on the monotonic and the wall clock; the raised time is
/// the wall clock minus how long ago the notification's own `Instant` was. Taking both as
/// arguments keeps this deterministic — the store reads no clock.
pub fn notification_record(
    notification: &AppNotification,
    now: Instant,
    wall: SystemTime,
) -> Option<NotificationRecord> {
    let standing = match notification.archived_at() {
        Some((cause, _)) => NotificationStanding::Archived(cause),
        None if notification.is_visible() => NotificationStanding::Visible,
        None => return None,
    };
    let mut view = entry_view(notification);
    if standing != NotificationStanding::Visible {
        view.actions.clear();
    }
    let age = now.saturating_duration_since(notification.created_at);
    let wall = wall.duration_since(UNIX_EPOCH).unwrap_or_default();
    Some(NotificationRecord {
        view,
        source: notification.source.clone(),
        raised_at: wall.saturating_sub(age).as_secs(),
        standing,
        repeats: notification.repeats,
    })
}
