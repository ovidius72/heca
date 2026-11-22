//! The identity of a workspace that outlives a reorder and a restart of the server.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::SessionKey;

/// **Which workspace this is, for as long as the workspace lives** — the same on the server (in an
/// action, [`ActionCx::workspace`](super::ActionCx::workspace)) and in a window (a dock reading
/// `cx.state().active_workspace()`), so what one stores the other finds.
///
/// It is the workspace's session and the number heca gave the workspace when it was made. It does
/// not change when workspaces are reordered, renamed or folded, nor when the server stops and
/// starts again (a saved session keeps its workspaces' numbers); it goes when the workspace is
/// deleted. Its position in the list — `ws_idx` — is a different thing and changes on a reorder.
///
/// It is opaque, like [`SessionKey`]: compare it, hash it, store it, send it. Nothing can be read
/// out of it and nothing but heca can make one.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceKey {
    session: SessionKey,
    workspace: u64,
}

impl WorkspaceKey {
    /// The key of workspace `workspace` in session `session`.
    pub(crate) fn new(session: SessionKey, workspace: heca_core::layout::WorkspaceId) -> Self {
        Self {
            session,
            workspace: workspace.0,
        }
    }

    /// The session this workspace belongs to.
    pub fn session(&self) -> SessionKey {
        self.session
    }
}

/// Shown for logs as `<session>.<number>`; [`Deserialize`] reads the same form back.
impl fmt::Display for WorkspaceKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.{}", self.session, self.workspace)
    }
}

impl fmt::Debug for WorkspaceKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "WorkspaceKey({self})")
    }
}

impl Serialize for WorkspaceKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for WorkspaceKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let bad = || de::Error::custom("a workspace key is <session key>.<number>");
        let (session, workspace) = text.split_once('.').ok_or_else(bad)?;
        let session: SessionKey =
            serde_json::from_value(serde_json::Value::String(session.to_owned()))
                .map_err(|_| bad())?;
        let workspace = workspace.parse().map_err(|_| bad())?;
        Ok(Self { session, workspace })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_key_is_stored_and_read_back_unchanged() {
        let key = WorkspaceKey::new(SessionKey::generate(), heca_core::layout::WorkspaceId(7));
        let stored = serde_json::to_string(&key).unwrap();
        assert_eq!(serde_json::from_str::<WorkspaceKey>(&stored).unwrap(), key);
        assert!(serde_json::from_str::<WorkspaceKey>("\"nonsense\"").is_err());
    }
}
