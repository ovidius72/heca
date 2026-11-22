//! Session queries use the same framed socket as windows, without attaching a window.

use super::{Answer, Invocation};
use crate::server::{
    sessions::{SessionCommand, SessionReply},
    socket::Start,
};

pub(super) fn terminate_confirmation(
    invocation: &Invocation,
) -> Result<Option<super::confirmation::Confirmation>, String> {
    let name = invocation.get("name").ok_or("missing session name")?;
    let SessionReply::List(sessions) = request(SessionCommand::List, Start::Never)? else {
        return Err("unexpected session reply\n".into());
    };
    let target = sessions
        .iter()
        .find(|session| session.name == name)
        .ok_or_else(|| format!("session '{name}' not found\n"))?;
    let config = target
        .folder
        .as_deref()
        .map(heca_config::loader::AppConfig::load_for)
        .unwrap_or_else(heca_config::loader::AppConfig::load)
        .config;
    let mut spec = crate::actions::ConfirmSpec::for_session(
        "terminate_session",
        &config.settings.session_text,
    )
    .ok_or("session confirmation is not declared\n")?;
    if !spec.forced
        && !config
            .confirm
            .enabled(&spec.config_name, spec.default_enabled)
    {
        return Ok(None);
    }
    spec.message = spec.message.replace("{name}", &target.name);
    Ok(Some(super::confirmation::Confirmation {
        spec,
        prompt: config.settings.session_text.confirm_prompt,
    }))
}

pub(super) fn list(invocation: &Invocation) -> Answer {
    let reply = request(SessionCommand::List, Start::IfMissing)?;
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
    request(SessionCommand::Terminate { name }, Start::Never)?;
    Ok("ok\n".into())
}

pub(super) fn check_attach(invocation: &Invocation) -> Answer {
    let name = invocation.get("name").ok_or("missing session name")?;
    let SessionReply::List(sessions) = request(SessionCommand::List, Start::IfMissing)? else {
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

/// Ask the server one session question. `start` is whether a missing server is started: a command
/// that needs the sessions (listing, attaching) starts it — they come back with it — and one that
/// acts on a running server (terminating) does not.
fn request(command: SessionCommand, start: Start) -> Result<SessionReply, String> {
    use crate::server::{
        ServerWaker,
        wire::{ClientMsg, ServerMsg},
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
    let link = crate::server::socket::connect(config, wake, start)
        .map_err(|error| format!("could not reach the server: {error}\n"))?;
    link.send(ClientMsg::Session {
        request_id: 1,
        command,
    })
    .map_err(|error| error.to_string())?;
    let reply = crate::server::wire::await_message(link.as_ref(), &receive, config, || {})
        .map_err(|error| error.to_string())?;
    match reply {
        ServerMsg::Sessions {
            request_id: 1,
            result,
        } => result,
        _ => Err("unexpected session reply".into()),
    }
}
