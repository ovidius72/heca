//! Session queries use the same framed socket as windows, without attaching a window.

use super::{Answer, Invocation};
use crate::server::sessions::{SessionCommand, SessionReply};

pub(super) fn list(invocation: &Invocation) -> Answer {
    let reply = request(SessionCommand::List)?;
    let SessionReply::List(sessions) = reply else {
        return Err("unexpected session reply\n".into());
    };
    if invocation.json {
        return serde_json::to_string_pretty(&sessions)
            .map(|text| text + "\n")
            .map_err(|error| error.to_string());
    }
    Ok(sessions
        .iter()
        .map(|session| {
            format!(
                "{}\t{}\t{}\t{}\t{:?}\n",
                session.name,
                session
                    .folder
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                session.windows,
                session.agents,
                session.status
            )
        })
        .collect())
}

pub(super) fn terminate(invocation: &Invocation) -> Answer {
    let name = invocation
        .get("name")
        .ok_or("missing session name")?
        .to_owned();
    request(SessionCommand::Terminate { name })?;
    Ok("ok\n".into())
}

pub(super) fn check_attach(invocation: &Invocation) -> Answer {
    let name = invocation.get("name").ok_or("missing session name")?;
    let SessionReply::List(sessions) = request(SessionCommand::List)? else {
        return Err("unexpected session reply".into());
    };
    if sessions.iter().any(|session| session.name == name) {
        Ok(String::new())
    } else {
        Err(format!(
            "session '{name}' not found; create one with `heca --new [path] --name <name>`\n"
        ))
    }
}

#[cfg(unix)]
fn request(command: SessionCommand) -> Result<SessionReply, String> {
    use crate::server::{
        ServerWaker,
        wire::{ClientLink, ClientMsg, ServerMsg, socket::SocketClient},
    };
    use std::sync::mpsc;
    let (ready, receive) = mpsc::sync_channel(1);
    let wake = ServerWaker::new(move || {
        let _ = ready.try_send(());
    });
    let config = heca_config::loader::AppConfig::load()
        .config
        .settings
        .connection;
    let path = crate::server::socket::socket_path().ok_or("no runtime folder for the server")?;
    let link = SocketClient::connect(&path, config, wake)
        .map_err(|error| format!("could not reach the server: {error}\n"))?;
    link.send(ClientMsg::Session {
        request_id: 1,
        command,
    })
    .map_err(|error| error.to_string())?;
    let reply = crate::server::wire::await_message(&link, &receive, config, || {})
        .map_err(|error| error.to_string())?;
    match reply {
        ServerMsg::Sessions {
            request_id: 1,
            result,
        } => result,
        _ => Err("unexpected session reply".into()),
    }
}

#[cfg(not(unix))]
fn request(_: SessionCommand) -> Result<SessionReply, String> {
    Err("session sockets are not available on this platform".into())
}
