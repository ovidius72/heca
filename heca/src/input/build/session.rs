use super::*;

pub(super) fn build_session(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    Some(match name {
        "list_sessions" => WmAction::ListSessions,
        "kill_server" => WmAction::KillServer,
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
        "rename_session" => WmAction::RenameSession {
            session: get_string(args, "session"),
        },
        "detach_session" => WmAction::DetachSession,
        "close_window" => WmAction::CloseWindow,
        "last_session" => WmAction::LastSession,
        "new_window" => WmAction::NewWindow {
            session: get_string(args, "session"),
        },
        "reconnect_session" => WmAction::ReconnectSession,
        "follow" => WmAction::Follow {
            target: get_string(args, "target").and_then(|value| value.parse().ok()),
        },
        _ => return None,
    })
}
