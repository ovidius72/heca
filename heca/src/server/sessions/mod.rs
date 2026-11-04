//! Session identities and lifecycle requests shared by the one server protocol.

use heca_core::layout::SessionId;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionStatus {
    Active,
    Terminated,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionInfo {
    pub id: SessionId,
    pub name: String,
    pub folder: Option<PathBuf>,
    pub label: Option<String>,
    pub status: SessionStatus,
    pub windows: usize,
    pub agents: usize,
}

impl SessionInfo {
    pub(super) fn unnamed(id: SessionId, template: &str, folder: Option<PathBuf>) -> Self {
        Self {
            id,
            name: template.replace("{number}", &id.0.to_string()),
            folder,
            label: None,
            status: SessionStatus::Active,
            windows: 0,
            agents: 0,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum SessionCommand {
    List,
    Create {
        folder: PathBuf,
        name: Option<String>,
        label: Option<String>,
    },
    Rename {
        name: String,
        new_name: String,
    },
    Terminate {
        name: String,
    },
    Restart {
        name: String,
    },
    Delete {
        name: String,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum SessionReply {
    List(Vec<SessionInfo>),
    Updated(SessionInfo),
    Deleted(SessionId),
}

/// A terminal's original launch and reported native agent identity, not a live process.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct RecordedTerminal {
    pub running: bool,
    pub launch: Option<heca_core::backend::ProcessLaunch>,
    pub owner: crate::app::backend_store::TerminalOwner,
    pub spec: crate::app::backend_store::TerminalSpec,
    pub agent: Option<heca_core::agent::AgentState>,
}

pub(super) fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err("a session name must contain visible text and no control characters".into());
    }
    Ok(())
}

pub(crate) fn owns(kind: crate::input::WmActionKind) -> bool {
    use crate::input::WmActionKind as K;
    matches!(
        kind,
        K::ListSessions | K::CreateSession
            | K::RenameSessionTo
            | K::TerminateSession
            | K::RestartSession
            | K::DeleteSession
    )
}

pub(super) fn from_action(
    action: &crate::input::WmAction,
    current: Option<&str>,
) -> Result<SessionCommand, String> {
    use crate::input::WmAction as A;
    let target = |name: &Option<String>| {
        name.clone()
            .or_else(|| current.map(str::to_owned))
            .ok_or("no session is selected".to_owned())
    };
    Ok(match action {
        A::ListSessions => SessionCommand::List,
        A::CreateSession {
            folder,
            name,
            label,
        } => SessionCommand::Create {
            folder: folder.into(),
            name: name.clone(),
            label: label.clone(),
        },
        A::RenameSessionTo { session, name } => SessionCommand::Rename {
            name: target(session)?,
            new_name: name.clone(),
        },
        A::TerminateSession { session } => SessionCommand::Terminate {
            name: target(session)?,
        },
        A::RestartSession { session } => SessionCommand::Restart {
            name: target(session)?,
        },
        A::DeleteSession { session } => SessionCommand::Delete {
            name: target(session)?,
        },
        _ => return Err("not a session lifecycle action".into()),
    })
}

/// Resolve a user path without changing the server process's working directory.
pub(crate) fn resolve_folder(
    path: Option<&str>,
    base: &std::path::Path,
) -> std::io::Result<PathBuf> {
    let path = match path {
        None | Some("") => return Ok(base.to_owned()),
        Some("~") => {
            return dirs::home_dir().ok_or_else(|| std::io::Error::other("no home folder"));
        }
        Some(path) if path.starts_with("~/") => dirs::home_dir()
            .ok_or_else(|| std::io::Error::other("no home folder"))?
            .join(&path[2..]),
        Some(path) => PathBuf::from(path),
    };
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}
