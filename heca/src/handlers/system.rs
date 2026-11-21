//! App-wide actions: the palette, the context menu, reload, running a command, modes, font zoom, notifications and search history.

use crate::app_state::AppState;
use crate::input::{FontZoomStep, WmAction};

pub fn handle_command_palette(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let (mode, query) = match action {
        WmAction::CommandPalette {
            ask: Some(action),
            arguments,
            ..
        } => {
            let given = crate::chrome::palette_arguments_given(arguments.as_deref());
            return crate::chrome::ask_action_arguments(state, action, given);
        }
        WmAction::CommandPalette { mode, query, .. } => (mode.as_deref(), query.as_deref()),
        _ => (None, None),
    };
    crate::chrome::open_command_palette(state, mode, query, None);
    Ok(())
}

/// Open the context menu for the active context (keyboard / RPC entry, `prefix+>`).
///
/// A focused container's cursor row wins; otherwise the focused content pane. **Resolved here, at
/// dispatch time** — it used to be resolved before the `Prefix` transition and
/// stashed in `pending_context`, because the mode the handler ran in could no longer say a
/// container had been driving. Chrome focus is not a mode and the transition does not touch it, so
/// there is nothing left to carry across and the stash is gone.
pub fn handle_open_context_menu(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    // **The focused widget's own menu first**. Generalised, not duplicated: this
    // action stopped meaning "the focused pane's menu" and started meaning "the focused widget's,
    // bubbling outwards" — so `prefix+>`, `Shift+F10` and the Menu key all reach a pane row, a
    // column, a workspace or a plugin's row through one catalogued action, and the host knows what
    // none of them are. Deliberately NOT a `[keys.widgets]` intent: `Escape` meaning both the WM's
    // `close_overlay` and the widget's `dismiss` is the split that swallowed `q`.
    if crate::chrome::open_declared_menu_for_focus(state) {
        return Ok(());
    }
    // No mounted widget declared one: retain the registered active-pane context lookup.
    // Both routes use the library's keyboard anchor, never host-computed coordinates.
    let Some((path, target)) = crate::chrome::resolve_active_context(state) else {
        return Ok(());
    };
    let Some(anchor) = heca_grid_ui::menu::keyboard_anchor(&state.window_root, None) else {
        return Ok(());
    };
    crate::chrome::open_context_menu_for(
        state,
        &path.0,
        target,
        anchor,
        crate::app::interaction::InteractionSource::Keyboard,
    );
    Ok(())
}

/// **`install_command`** — put `heca` on the command line, and say where (and, when its folder is not
/// on the `PATH`, the line to add).
pub fn handle_install_command(
    _state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    crate::app::install_command::install()?
        .notification()
        .send();
    Ok(())
}

pub fn handle_reload_config(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    state.pending_reload = Some(crate::app_state::ReloadReason::Explicit);
    Ok(())
}

/// Give chrome keyboard focus to a dock — by id, or by letter.
///
/// `dock = Some(id)` focuses that container directly (RPC, a menu entry, a script). A bare
/// `focus_dock` opens the pick: every dock on screen lights a letter and the next keypress focuses
/// the one chosen ([`InputMode::DockPick`](crate::app_state::InputMode::DockPick), resolved in `app/input.rs`).
///
/// The pick is uniform — it is offered even when there is only one dock — because "sometimes a letter
/// appears and sometimes the key acts immediately" is a rule a user has to learn from surprise.
/// Nothing mounted, or nothing on screen, means nothing to focus: a no-op.
/// Forget the past queries every search surface remembers, or just one scope's.
pub fn handle_clear_search_history(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let scope = match action {
        WmAction::ClearSearchHistory { scope } => scope.clone(),
        _ => None,
    };
    crate::search_state::forget(
        state,
        scope.as_deref(),
        crate::search_state::Forget::Queries,
    );
    Ok(())
}

/// Forget the usage counts that rank a search surface's list.
pub fn handle_clear_search_ranking(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let scope = match action {
        WmAction::ClearSearchRanking { scope } => scope.clone(),
        _ => None,
    };
    crate::search_state::forget(
        state,
        scope.as_deref(),
        crate::search_state::Forget::Ranking,
    );
    Ok(())
}

/// Dismiss one notification by id — the toast's own × (a real, automatically-pickable
/// `Button`), or RPC/a plugin naming a specific id. No default keybinding: a keypress cannot
/// supply an id.
pub fn handle_notification_dismiss_one(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::NotificationDismissOne { notification_id } = action else {
        return Ok(());
    };
    let id = crate::notification::NotificationId::from_raw(*notification_id);
    state
        .client
        .notify(crate::server::NotificationCommand::DismissOne(id));
    Ok(())
}

/// Dismiss every currently visible notification — no on-screen control; reachable from the
/// command palette, a keybinding, and RPC.
pub fn handle_notification_dismiss_all(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    state
        .client
        .notify(crate::server::NotificationCommand::DismissAll);
    Ok(())
}

/// Dismiss the first eligible visible notification in stable toast order.
pub fn handle_notification_dismiss_last(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    state
        .client
        .notify(crate::server::NotificationCommand::DismissLast);
    Ok(())
}

/// Toggle the scoped picker over the visible toast actions/×, in addition to (never instead
/// of) their global `prefix+/` letters. `KeyHintGroup::open_when` reads this signal directly.
pub fn handle_notification_pick(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
    let open = state.notification_pick_open.get_untracked();
    state.notification_pick_open.set(!open);
    Ok(())
}

/// List the notification history. The server owns it, so this only asks; the palette's
/// notifications mode opens when the answer arrives (`AppState::receive_client`).
pub fn handle_notification_history(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    state.client.request_notification_history();
    Ok(())
}

/// Resolve `ToastStack::on_action(id, key)` (relayed as this action by the mount, since a
/// notification's real Intent cannot be known until the notification exists — see
/// chrome::surfaces::notifications) into the notification's real `Intent` and fire it through the
/// same layer_emitter the mount built, landing on the event loop's next turn.
pub fn handle_notification_action_relay(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::NotificationActionRelay {
        notification_id,
        key,
    } = action
    else {
        return Ok(());
    };
    let id = crate::notification::NotificationId::from_raw(*notification_id);
    // The real `Intent` rides in the projection the server sent (`NotificationView::action`); the
    // `ToastSpec` keeps only the key, so this resolves it from the received views, not a server
    // round-trip.
    let Some((intent, dismiss_after, runs)) = state
        .notifications
        .iter()
        .find(|view| view.id == id)
        .and_then(|view| view.action(key))
        .map(|action| (action.intent.clone(), action.dismiss_after, action.runs))
    else {
        return Ok(());
    };
    // **A button made with `NotificationAction::run`** runs its closure where the notification was
    // raised — the server — whichever window it was pressed in.
    if runs {
        state
            .client
            .notify(crate::server::NotificationCommand::Run(id, key.clone()));
        if dismiss_after {
            state
                .client
                .notify(crate::server::NotificationCommand::DismissOne(id));
        }
        return Ok(());
    }
    // Fired **as the stack**, so the relayed intent is judged exactly as the click that asked for
    // it was — the surface's identity comes from its own constant, not from an id someone kept.
    let emit = crate::chrome::layer_emitter(
        &state.event_proxy,
        crate::chrome::notification_surface_key(),
    );
    emit.fire_intent(intent);
    if dismiss_after {
        state
            .client
            .notify(crate::server::NotificationCommand::DismissOne(id));
    }
    Ok(())
}

/// Resolve a [`FontZoomStep`] into a signed point delta using the configured step
/// size (`[settings] terminal_font_zoom_step`). `Reset` maps to `0.0`, which both
/// zoom helpers treat as "clear the offset". A non-positive configured step falls
/// back to the built-in default so zoom never becomes a no-op.
pub(crate) fn font_zoom_delta(state: &AppState, step: FontZoomStep) -> f32 {
    use crate::app::terminal_metrics::TERMINAL_FONT_ZOOM_STEP;
    let size = if state.settings.terminal_font_zoom_step > 0.0 {
        state.settings.terminal_font_zoom_step
    } else {
        TERMINAL_FONT_ZOOM_STEP
    };
    match step {
        FontZoomStep::In => size,
        FontZoomStep::Out => -size,
        FontZoomStep::Reset => 0.0,
    }
}

/// App-wide terminal font zoom — steps every pane's size.
pub fn handle_app_font_zoom(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::AppFontZoom { step } = action else {
        return Ok(());
    };
    let delta = font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_app_font_zoom(state, delta);
    Ok(())
}
