//! App-wide actions: the palette, the context menu, reload, running a command, modes, font zoom, notifications and search history.

use crate::app_state::AppState;
use crate::input::{FontZoomStep, WmAction};

pub fn handle_command_palette(state: &mut AppState, action: &WmAction) {
    let (mode, query) = match action {
        WmAction::CommandPalette { mode, query } => (mode.as_deref(), query.as_deref()),
        _ => (None, None),
    };
    crate::chrome::open_command_palette(state, mode, query);
}

/// Open the context menu for the active context (keyboard / RPC entry, `prefix+>`).
///
/// A focused container's cursor row wins; otherwise the focused content pane. **Resolved here, at
/// dispatch time** — it used to be resolved before the `Prefix` transition and
/// stashed in `pending_context`, because the mode the handler ran in could no longer say a
/// container had been driving. Chrome focus is not a mode and the transition does not touch it, so
/// there is nothing left to carry across and the stash is gone.
pub fn handle_open_context_menu(state: &mut AppState, _action: &WmAction) {
    // **The focused widget's own menu first**. Generalised, not duplicated: this
    // action stopped meaning "the focused pane's menu" and started meaning "the focused widget's,
    // bubbling outwards" — so `prefix+>`, `Shift+F10` and the Menu key all reach a pane row, a
    // column, a workspace or a plugin's row through one catalogued action, and the host knows what
    // none of them are. Deliberately NOT a `[keys.widgets]` intent: `Escape` meaning both the WM's
    // `close_overlay` and the widget's `dismiss` is the split that swallowed `q`.
    if crate::chrome::open_declared_menu_for_focus(state) {
        return;
    }
    // Nothing declared one: the content pane is the app's own domain and still resolves the old
    // way, since a terminal surface is not a widget that can carry a declaration.
    let Some((path, target)) = crate::chrome::resolve_active_context(state) else {
        return;
    };
    let (cx, cy) = crate::mouse::window_center_logical(state);
    crate::chrome::open_context_menu_for(
        state,
        &path.0,
        target,
        heca_core::layout::Point::new(cx as f64, cy as f64),
        crate::app::interaction::InteractionSource::Keyboard,
    );
}

pub fn handle_reload_config(state: &mut AppState, _action: &WmAction) {
    state.pending_reload = true;
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
pub fn handle_clear_search_history(state: &mut AppState, action: &WmAction) {
    let scope = match action {
        WmAction::ClearSearchHistory { scope } => scope.clone(),
        _ => None,
    };
    crate::search_state::forget(
        state,
        scope.as_deref(),
        crate::search_state::Forget::Queries,
    );
}

/// Forget the usage counts that rank a search surface's list.
pub fn handle_clear_search_ranking(state: &mut AppState, action: &WmAction) {
    let scope = match action {
        WmAction::ClearSearchRanking { scope } => scope.clone(),
        _ => None,
    };
    crate::search_state::forget(
        state,
        scope.as_deref(),
        crate::search_state::Forget::Ranking,
    );
}

/// Dismiss one notification by id — the toast's own × (a real, automatically-pickable
/// `Button`), or RPC/a plugin naming a specific id. No default keybinding: a keypress cannot
/// supply an id.
pub fn handle_notification_dismiss_one(state: &mut AppState, action: &WmAction) {
    let WmAction::NotificationDismissOne { notification_id } = action else {
        return;
    };
    let id = crate::notification::NotificationId::from_raw(*notification_id);
    state.client.notify(crate::server::NotificationCommand::DismissOne(id));
}

/// Dismiss every currently visible notification — no on-screen control; reachable from the
/// command palette, a keybinding, and RPC.
pub fn handle_notification_dismiss_all(state: &mut AppState, _action: &WmAction) {
    state.client.notify(crate::server::NotificationCommand::DismissAll);
}

/// Dismiss the first eligible visible notification in stable toast order.
pub fn handle_notification_dismiss_last(state: &mut AppState, _action: &WmAction) {
    state.client.notify(crate::server::NotificationCommand::DismissLast);
}

/// Toggle the scoped picker over the visible toast actions/×, in addition to (never instead
/// of) their global `prefix+/` letters. `KeyHintGroup::open_when` reads this signal directly.
pub fn handle_notification_pick(state: &mut AppState, _action: &WmAction) {
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
    let open = state.notification_pick_open.get_untracked();
    state.notification_pick_open.set(!open);
}

/// Resolve `ToastStack::on_action(id, key)` (relayed as this action by the mount, since a
/// notification's real Intent cannot be known until the notification exists — see
/// chrome::surfaces::notifications) into the notification's real `Intent` and fire it through the
/// same layer_emitter the mount built, landing on the event loop's next turn.
pub fn handle_notification_action_relay(state: &mut AppState, action: &WmAction) {
    let WmAction::NotificationActionRelay {
        notification_id,
        key,
    } = action
    else {
        return;
    };
    let id = crate::notification::NotificationId::from_raw(*notification_id);
    // The real `Intent` rides in the projection the server sent (`NotificationView::action`); the
    // `ToastSpec` keeps only the key, so this resolves it from the received views, not a server
    // round-trip.
    let Some((intent, dismiss_after)) = state
        .notifications
        .iter()
        .find(|view| view.id == id)
        .and_then(|view| view.action(key))
        .map(|action| (action.intent.clone(), action.dismiss_after))
    else {
        return;
    };
    // Fired **as the stack**, so the relayed intent is judged exactly as the click that asked for
    // it was — the surface's identity comes from its own constant, not from an id someone kept.
    let emit = crate::chrome::layer_emitter(
        &state.event_proxy,
        crate::chrome::notification_surface_key(),
    );
    emit.fire_intent(intent);
    if dismiss_after {
        state.client.notify(crate::server::NotificationCommand::DismissOne(id));
    }
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
pub fn handle_app_font_zoom(state: &mut AppState, action: &WmAction) {
    let WmAction::AppFontZoom { step } = action else {
        return;
    };
    let delta = font_zoom_delta(state, *step);
    crate::app::terminal_metrics::apply_app_font_zoom(state, delta);
}
