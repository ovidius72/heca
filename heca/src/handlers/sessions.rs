//! Session UI binds the shared actions; it owns no process or lifecycle state.

use crate::{
    app_state::AppState,
    chrome::{ModalAction, ModalResult, ModalSpec, PropValue},
    input::WmAction,
};

pub(crate) fn handle_detach_session(state: &mut AppState, _: &WmAction) {
    let _ = state
        .event_proxy
        .send_event(crate::app::events::AppEvent::CloseWindow);
}

pub(crate) fn handle_switch_session(state: &mut AppState, action: &WmAction) {
    let WmAction::SwitchSession { name } = action else {
        return;
    };
    if let Err(error) = state.client.switch_session(name.clone()) {
        state.status_note = Some(error.to_string());
    }
}

pub(crate) fn handle_reconnect_session(state: &mut AppState, _: &WmAction) {
    if let Err(error) = state.client.reconnect_session() {
        state.status_note = Some(error.to_string());
        session_unavailable(state, true);
    }
}

pub(crate) fn handle_rename_session(state: &mut AppState, _: &WmAction) {
    let words = &state.settings.session_text;
    let spec = ModalSpec {
        title: state
            .action_catalog
            .label("rename_session")
            .unwrap_or_default()
            .into(),
        body: heca_view::build::Input::new()
            .name("name")
            .value(state.client.session.name.clone())
            .into(),
        actions: vec![
            ModalAction::new("ok", &words.accept).disabled_when_empty("name"),
            ModalAction::new("cancel", &words.cancel),
        ],
        danger: false,
        dismissible: true,
    };
    crate::chrome::open_modal(state, spec, |state, registry, result| {
        if let ModalResult::Action { id, data } = result
            && id == "ok"
            && let Some(name) = data.get("name").and_then(PropValue::as_text)
        {
            registry.execute(
                &WmAction::RenameSessionTo {
                    session: None,
                    name: name.trim().into(),
                },
                state,
            );
        }
    });
}

pub(crate) fn handle_new_session(state: &mut AppState, _: &WmAction) {
    crate::chrome::open_session_folder(state);
}

pub(crate) fn session_unavailable(state: &mut AppState, server_lost: bool) {
    if state.session_dialog.is_some() {
        return;
    }
    let words = &state.settings.session_text;
    let spec = ModalSpec::message(
        &words.unavailable,
        if server_lost {
            &words.server_unavailable
        } else {
            &words.terminated
        },
    )
    .dismissible(false)
    .action(ModalAction::new("restart", &words.restart))
    .action(ModalAction::new("exit", &words.exit));
    let id = crate::chrome::open_modal(state, spec, move |state, registry, result| {
        state.session_dialog = None;
        if let ModalResult::Action { id, .. } = result {
            match id.as_str() {
                "restart" if server_lost => registry.execute(&WmAction::ReconnectSession, state),
                "restart" => registry.execute(&WmAction::RestartSession { session: None }, state),
                "exit" => registry.execute(&WmAction::DetachSession, state),
                _ => {}
            }
        }
    });
    state.session_dialog = Some(id);
}
