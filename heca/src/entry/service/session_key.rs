//! The identity of a session that outlives a run of the server.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// **Which session this is, for as long as the session lives.** A service keeps what it stores for
/// a session against this, and finds it again by the same key.
///
/// It is made once, by heca, when a session is created, and never changes: not when the session is
/// renamed, terminated, restarted or has windows come and go, and not with its name, its folder or
/// a window's connection. It goes when the session is deleted. [`ActionCx::session`] and
/// [`ProjectCx::session`] are a different thing: a number for this run only.
///
/// It is opaque: compare it, hash it, store it, send it. Nothing can be read out of it and nothing
/// can make one but heca. A server that starts again gives its sessions new keys until sessions are
/// restored across restarts.
///
/// [`ActionCx::session`]: super::ActionCx::session
/// [`ProjectCx::session`]: super::ProjectCx::session
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionKey([u8; 16]);

impl SessionKey {
    /// A new key: 128 bits from the operating system's random source.
    pub(crate) fn generate() -> Self {
        let mut bits = [0; 16];
        getrandom::fill(&mut bits).expect("the operating system has no random source");
        Self(bits)
    }
}

/// Shown for logs; it reads as 32 hex digits and nothing parses it back.
impl fmt::Display for SessionKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "SessionKey({self})")
    }
}

impl Serialize for SessionKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SessionKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let digit = |pair: &[u8]| std::str::from_utf8(pair).ok().and_then(|pair| u8::from_str_radix(pair, 16).ok());
        let bytes: Option<Vec<u8>> = text.as_bytes().chunks(2).map(digit).collect();
        bytes.and_then(|bytes| <[u8; 16]>::try_from(bytes).ok()).map(Self)
            .ok_or_else(|| de::Error::custom("a session key is 32 hex digits"))
    }
}
