//! Session UI binds the shared actions; it owns no process or lifecycle state.

use crate::{
    app_state::AppState,
    chrome::{ModalAction, ModalResult, ModalSpec, PropValue},
    input::WmAction,
};

/// **`new_window`** — another window on the same server, on the named session or this window's. The
/// program opens it (a window cannot make one itself); this window stays as it is.
pub(crate) fn handle_new_window(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::NewWindow { session } = action else {
        return Ok(());
    };
    let session = session
        .clone()
        .or_else(|| (!state.client.detached).then(|| state.client.session.name.clone()));
    state.event_proxy.new_window(session);
    Ok(())
}

/// **`close_window`** — this window goes; its session and its programs keep running. heca ends
/// with its last window (macOS: Cmd+W closes a window, Cmd+Q the app).
pub(crate) fn handle_close_window(
    state: &mut AppState,
    _: &WmAction,
) -> crate::actions::ActionResult {
    let _ = state
        .event_proxy
        .send_event(crate::app::events::AppEvent::CloseWindow);
    Ok(())
}

/// **`last_session`** — back to the session this window showed before; again, and it comes back
/// (tmux's last session). Each window remembers its own.
pub(crate) fn handle_last_session(
    state: &mut AppState,
    _: &WmAction,
) -> crate::actions::ActionResult {
    let Some(name) = state.client.last_session_name() else {
        state.status_note = Some("No other session visited in this window yet".into());
        return Ok(());
    };
    let action = WmAction::SwitchSession { name };
    handle_switch_session(state, &action)
}

pub(crate) fn handle_detach_session(
    state: &mut AppState,
    _: &WmAction,
) -> crate::actions::ActionResult {
    if state.client.detached {
        session_choice(state, SessionChoice::Detached);
    } else if let Err(error) = state.client.detach_session() {
        state.status_note = Some(error.to_string());
        session_unavailable(state, true);
    }
    Ok(())
}

pub(crate) fn handle_switch_session(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::SwitchSession { name } = action else {
        return Ok(());
    };
    if let Err(error) = state.client.switch_session(name.clone()) {
        state.status_note = Some(error.to_string());
    } else {
        settle_before_leaving(state);
    }
    Ok(())
}

/// **The window is about to show another session**: what is still dissolving (the palette the
/// choice came from) goes now, and the switch waits for the frame without it — the last picture
/// before the change is held on screen while the new session loads, and must not hold a surface
/// already closed. A covered window draws nothing to wait for.
fn settle_before_leaving(state: &mut AppState) {
    state.layers.cut_exits(&mut state.window_root);
    if !state.window_occluded {
        // The frame the selection waits for is always drawn: asked for, and marked as needed.
        state.client.hold_selection_for_a_frame();
        state.mark_full_redraw();
        state.window.request_redraw();
    }
}

pub(crate) fn handle_follow(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::Follow { target } = action else {
        return Ok(());
    };
    // Window-local: mirror the target (or stop). The server relays the leader's view; the follow
    // breaks on this window's own input.
    state.client.follow(*target);
    Ok(())
}

pub(crate) fn handle_reconnect_session(
    state: &mut AppState,
    _: &WmAction,
) -> crate::actions::ActionResult {
    if let Err(error) = state.client.reconnect_session() {
        state.status_note = Some(error.to_string());
        session_unavailable(state, true);
    }
    Ok(())
}

/// **`rename_session`** — the rename dialog, for `session` or this window's: its name to edit, OK
/// renaming it through `rename_session_to`. The one rename prompt, from a key and from a session
/// row's menu alike.
pub(crate) fn handle_rename_session(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::RenameSession { session } = action else {
        return Ok(());
    };
    let target = session.clone();
    let current = target
        .clone()
        .unwrap_or_else(|| state.client.session.name.clone());
    let words = &state.settings.session_text;
    let spec = ModalSpec {
        title: state
            .action_catalog
            .label("rename_session")
            .unwrap_or_default()
            .into(),
        body: heca_view::build::Input::new()
            .name("name")
            .value(current)
            .into(),
        actions: vec![
            ModalAction::new("ok", &words.accept).disabled_when_empty("name"),
            ModalAction::new("cancel", &words.cancel),
        ],
        danger: false,
        dismissible: true,
    };
    let _ = crate::chrome::open_modal(state, spec, move |state, registry, result| {
        if let ModalResult::Action { id, data } = result
            && id == "ok"
            && let Some(name) = data.get("name").and_then(PropValue::as_text)
        {
            let _ = registry.execute(
                &WmAction::RenameSessionTo {
                    session: target.clone(),
                    name: name.trim().into(),
                },
                state,
            );
        }
    });
    Ok(())
}

pub(crate) fn handle_new_session(
    state: &mut AppState,
    _: &WmAction,
) -> crate::actions::ActionResult {
    crate::chrome::open_session_folder(state);
    Ok(())
}

pub(crate) fn session_unavailable(state: &mut AppState, server_lost: bool) {
    session_choice(
        state,
        if server_lost {
            SessionChoice::ServerLost
        } else {
            SessionChoice::Terminated
        },
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionChoice {
    Detached,
    Terminated,
    Deleted,
    ServerLost,
}

pub(crate) fn session_choice(state: &mut AppState, reason: SessionChoice) {
    if state.session_dialog.is_some() {
        return;
    }
    let spec = session_choice_spec(
        &state.settings.session_text,
        reason,
        state
            .action_catalog
            .label("list_sessions")
            .unwrap_or_default(),
        state
            .action_catalog
            .label("new_session")
            .unwrap_or_default(),
    );
    let id = crate::chrome::open_modal(state, spec, move |state, registry, result| {
        state.session_dialog = None;
        let ModalResult::Action { id, .. } = result else {
            return;
        };
        if id == "exit" {
            let _ = state
                .event_proxy
                .send_event(crate::app::events::AppEvent::CloseWindow);
            return;
        }
        let action = match id.as_str() {
            "restart" if reason == SessionChoice::ServerLost => WmAction::ReconnectSession,
            "restart" if reason == SessionChoice::Terminated => {
                WmAction::RestartSession { session: None }
            }
            "select" => WmAction::CommandPalette {
                mode: Some("session".into()),
                query: None,
                ask: None,
                arguments: None,
            },
            "new" => WmAction::NewSession,
            _ => return,
        };
        // Keep a blocking choice behind a picker, including when that picker is canceled.
        if matches!(id.as_str(), "select" | "new") {
            session_choice(state, reason);
        }
        let _ = registry.execute(&action, state);
    });
    state.session_dialog = id.ok();
}

pub(crate) fn session_choice_spec(
    words: &heca_config::settings::SessionText,
    reason: SessionChoice,
    select: &str,
    new: &str,
) -> ModalSpec {
    let mut spec = ModalSpec::message(
        &words.unavailable,
        match reason {
            SessionChoice::ServerLost => &words.server_unavailable,
            SessionChoice::Terminated => &words.terminated,
            SessionChoice::Detached => &words.detached,
            SessionChoice::Deleted => &words.deleted,
        },
    )
    .dismissible(false);
    if matches!(
        reason,
        SessionChoice::ServerLost | SessionChoice::Terminated
    ) {
        spec = spec.action(ModalAction::new("restart", &words.restart));
    }
    if reason != SessionChoice::ServerLost {
        spec = spec
            .action(ModalAction::new("select", select))
            .action(ModalAction::new("new", new));
    }
    spec.action(ModalAction::new("exit", &words.exit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_dialog_requires_an_explicit_choice_and_uses_configured_text() {
        let words = heca_config::settings::SessionText {
            unavailable: "Unavailable title".into(),
            terminated: "Stopped session".into(),
            server_unavailable: "Lost server".into(),
            restart: "Try again".into(),
            exit: "Leave".into(),
            ..Default::default()
        };
        for (reason, message) in [
            (SessionChoice::Terminated, "Stopped session"),
            (SessionChoice::ServerLost, "Lost server"),
            (SessionChoice::Detached, words.detached.as_str()),
            (SessionChoice::Deleted, words.deleted.as_str()),
        ] {
            let spec = session_choice_spec(&words, reason, "Choose another", "Create another");
            assert!(!spec.dismissible);
            assert_eq!(spec.title, words.unavailable);
            assert_eq!(
                spec.body.props.get("text").and_then(PropValue::as_text),
                Some(message)
            );
            assert_eq!(
                spec.actions
                    .last()
                    .map(|a| (a.id.as_str(), a.label.as_str())),
                Some(("exit", "Leave"))
            );
            let has = |id: &str| spec.actions.iter().any(|a| a.id == id);
            assert_eq!(
                has("restart"),
                matches!(
                    reason,
                    SessionChoice::Terminated | SessionChoice::ServerLost
                )
            );
            assert_eq!(has("select"), reason != SessionChoice::ServerLost);
            assert_eq!(has("new"), reason != SessionChoice::ServerLost);
        }
    }

    #[test]
    fn session_controls_share_catalog_rpc_and_configurable_key_paths() {
        use crate::{
            app::registry,
            keymap::{KeyCombo, LEADER_LAYER},
        };
        use heca_config::theme::{BindingValue, Config, ModeBindingConfig};
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let actions = registry::build_registry();
        for line in [
            "list_sessions",
            "new_session",
            "create_session folder=/project name=A",
            "rename_session",
            "rename_session_to name=B",
            "terminate_session",
            "restart_session",
            "delete_session",
            "detach_session",
            "switch_session name=B",
            "reconnect_session",
        ] {
            let (intent, _) = crate::rpc::parse_rpc(line)
                .unwrap()
                .into_intent(&catalog)
                .unwrap();
            let args = crate::app::interaction::intent_args_as_strings(&intent);
            let action = crate::input::resolve_action(&intent.action, &args).unwrap();
            assert!(
                actions.has_handler(&action) || crate::server::sessions::owns(action.kind()),
                "{line}"
            );
            assert_eq!(
                crate::app::interaction::action_policy(&action),
                crate::app::interaction::ActionPolicy::Global
            );
            let mut config = Config::default();
            config.keys.bind = vec![ModeBindingConfig {
                action: intent.action,
                args,
                keys: BindingValue::Many(vec!["prefix+F12".into(), "prefix+Ctrl+F12".into()]),
            }];
            let keys =
                registry::build_keymaps(&config, &mut crate::app::conflicts::Conflicts::default());
            for key in ["F12", "Ctrl+F12"] {
                assert_eq!(
                    keys.flat
                        .resolve_builtin(LEADER_LAYER, &KeyCombo::parse(key)),
                    Some(&action),
                    "{line}: {key}"
                );
            }
        }
    }
}
