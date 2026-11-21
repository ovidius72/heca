//! **State heca and every extension save** — what an extension wants back after a restart (the
//! order of its rooms, a choice the user made), kept by heca, written by the server only.
//!
//! Not the user's configuration: that stays a file a person writes and heca only reads. This is
//! machine-written state, under heca's state-file rules: it lives in the server's own folder, a
//! write is atomic, and a missing, corrupt or unreadable value is simply not there — never a
//! failed start.
//!
//! An entry is found by a **key** and an **id**: `("room-order", "")` for one value, or
//! `("room-color", room_id)` for one per thing. heca adds the extension's name to the key, so two
//! extensions can never read or overwrite each other's entries. The value is the extension's own
//! type, stored as JSON; a value that no longer fits that type (the extension changed it) reads as
//! nothing, and the extension starts from its default.
//!
//! ```ignore
//! // In an action: the order of this session's rooms.
//! let order: Vec<RoomId> = cx.saved(Scope::Session, "room-order", "").unwrap_or_default();
//! cx.save(Scope::Session, "room-order", "", &order)?;
//! ```
//!
//! Each write raises `state.changed` (THE EVENTS) with the scope, the key and the id.

use super::SessionKey;

/// **Where an entry lives.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One for the whole server: every session reads the same value.
    Global,
    /// The session the call is in — an action's or a projection's session. It is saved with the
    /// session, comes back with it after a restart and goes when the session is deleted. With
    /// `save_sessions = false` the sessions are not kept across a restart, and neither is this.
    Session,
    /// A named session, for a call that is in none (a service handling a message).
    SessionOf(SessionKey),
}

/// Why an entry could not be saved.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SavedError {
    /// [`Scope::Session`] from a call that is in no session — name it with [`Scope::SessionOf`].
    NoSession,
    /// The value is larger than `[settings.services] saved_entry_max_bytes` allows.
    TooLarge { bytes: usize, limit: usize },
    /// The value could not be written as JSON.
    Encode(String),
}

impl std::fmt::Display for SavedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSession => f.write_str("this call is in no session: name one with Scope::SessionOf"),
            Self::TooLarge { bytes, limit } => write!(f, "the value is {bytes} bytes; the limit is {limit} ([settings.services] saved_entry_max_bytes)"),
            Self::Encode(why) => write!(f, "the value could not be written as JSON: {why}"),
        }
    }
}

impl std::error::Error for SavedError {}

/// **`state.changed`**: an entry was saved or forgotten. `key` is the full key, with the extension's
/// name in front (`heca-pro.room-order`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateEvent {
    /// `None` for [`Scope::Global`], else the session the entry belongs to.
    pub session: Option<SessionKey>,
    pub key: String,
    pub id: String,
}
