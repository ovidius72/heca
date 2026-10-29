use super::*;

pub(super) fn build_session(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    Some(match name {
        "list_sessions" => WmAction::ListSessions,
        "create_session" => WmAction::CreateSession {
            folder: get_string(args, "folder")?,
            name: get_string(args, "name"),
            label: get_string(args, "label"),
        },
        "rename_session_to" => WmAction::RenameSessionTo {
            session: get_string(args, "session"),
            name: get_string(args, "name")?,
        },
        "terminate_session" => WmAction::TerminateSession {
            session: get_string(args, "session"),
        },
        "restart_session" => WmAction::RestartSession {
            session: get_string(args, "session"),
        },
        "delete_session" => WmAction::DeleteSession {
            session: get_string(args, "session"),
        },
        "switch_session" => WmAction::SwitchSession {
            name: get_string(args, "name")?,
        },
        "new_session" => WmAction::NewSession,
        "rename_session" => WmAction::RenameSession,
        "detach_session" => WmAction::DetachSession,
        "reconnect_session" => WmAction::ReconnectSession,
        _ => return None,
    })
}
