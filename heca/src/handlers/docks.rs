//! Sidebars, regions and docks: showing them, focusing them, moving a container, and layers.

use super::pick::begin_pick;
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;
use crate::update_session_viewport;

/// **The one way a region is shown or hidden** — `set_region_visible`, `sidebar_left`,
/// `sidebar_right` and revealing a dock's region all end here, so a region has exactly one
/// visible/hidden state ([`SharedChromeState::is_visible`](crate::chrome::SharedChromeState)).
/// `[settings] show_*` is where that state starts and a config reload sets it back.
///
/// On a real change it reflows the session viewport and forces a full chrome rebuild, exactly like
/// the config-reload path. Drops the build, never the window root, so any surface that is up rides
/// through it.
fn set_region_visibility(
    state: &mut AppState,
    region: crate::chrome::RegionId,
    how: crate::input::RegionVisibility,
) {
    let visible = how.apply(state.chrome_state.is_visible(region));
    if !state.chrome_state.set_visible(region, visible) {
        return;
    }
    update_session_viewport(state);
    state.chrome_tree = None;
}

pub fn handle_sidebar_left(state: &mut AppState, _action: &WmAction) {
    set_region_visibility(
        state,
        crate::chrome::RegionId::LeftSidebar,
        crate::input::RegionVisibility::Toggle,
    );
}

pub fn handle_sidebar_right(state: &mut AppState, _action: &WmAction) {
    set_region_visibility(
        state,
        crate::chrome::RegionId::RightSidebar,
        crate::input::RegionVisibility::Toggle,
    );
}

/// Make `region` visible if it is currently hidden, so something seated in it can be seen.
///
/// A region is shown or hidden (no icon rail — see `docs/sidebar-provider-modes.md`). A region that
/// is already shown keeps its (possibly user-resized) width, and revealing it again is free.
fn reveal_region(state: &mut AppState, region: crate::chrome::RegionId) {
    set_region_visibility(state, region, crate::input::RegionVisibility::Show);
}

/// Enter sidebar-nav on the dock that has keyboard navigation — **whichever region it sits in**.
///
/// This used to read `left_visible()` and expand the *left* container, which is wrong the moment the
/// dock is seated elsewhere: the workspaces container declares `RegionSet::sidebars()`, so the focus
/// key expanded an empty left sidebar and navigated a tree drawn on the right (F003/P011/T020).
///
/// With no navigable dock mounted there is nothing to navigate, so this is a **no-op** — it does not
/// expand a region to show an empty frame.
/// Hand the keyboard to the dock that navigates — the overflow escape for the pane pickers.
///
/// When there are more panes than letters, a pick cannot offer them all, so the pickers fall back to
/// driving the dock instead. That is the app reaching for whichever container declares
/// `keyboard_navigable`, not for a named one; F003/P086 will make the pickers take their targets
/// from a component instead, at which point this goes.
pub(crate) fn focus_navigable_dock(state: &mut AppState) {
    let focused = state.chrome_state.focused_container();
    let Some((dock, _)) = crate::chrome::navigable_dock(&state.chrome_host, focused.as_deref())
    else {
        return;
    };
    handle_focus_dock(state, &WmAction::FocusDock { dock: Some(dock) });
}

/// What a request aimed at the dock `mount` should do, given who holds the keyboard.
///
/// A pure function of plain data so the one rule that separates `focus` from `toggle` is testable
/// without a window, and lives in one place instead of once per handler (the shape
/// `surface_action` uses for key resolution, F003/P082/T428).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DockFocus {
    /// Give it the keyboard.
    Take,
    /// It already has the keyboard, and this gesture means "give it back".
    Release,
    /// It already has the keyboard, and this gesture does not mean anything else.
    Nothing,
}

pub(crate) fn dock_focus_outcome(focused: Option<&str>, mount: &str, toggling: bool) -> DockFocus {
    match (focused == Some(mount), toggling) {
        (true, true) => DockFocus::Release,
        (true, false) => DockFocus::Nothing,
        (false, _) => DockFocus::Take,
    }
}

/// Focus a dock, or open the pick when none is named.
///
/// **It only focuses.** Aiming it at the dock that already has the keyboard does nothing — the
/// release is [`handle_toggle_dock`], which is what `global_focus` binds. Until F003/P082/T444 the
/// toggle lived here, so *every* caller inherited it: a click inside a focused dock released it, and
/// so did an RPC `focus-dock`. See [`WmAction::ToggleDock`].
pub fn handle_focus_dock(state: &mut AppState, action: &WmAction) {
    let dock = match action {
        WmAction::FocusDock { dock } => dock,
        _ => return,
    };
    if let Some(dock) = dock {
        // `dock` names a **placement or a component** (F003/P086/T363): a `global_focus` written
        // without an `id` speaks for the component, and lands on the seating you were last in.
        let focused = state.chrome_state.focused_container();
        let last = state.chrome_state.last_focused_container();
        let Some(mount) = crate::chrome::placement_for(
            &state.chrome_host,
            dock,
            focused.as_deref(),
            last.as_deref(),
        ) else {
            // Only a container the host actually has: focus is a promise that something is there to
            // receive the keys. Say so rather than letting the call vanish — a typo in an RPC call
            // or a binding's `dock` argument is otherwise indistinguishable from success.
            eprintln!("[heca] focus_dock: no container mounted under id or component '{dock}'");
            return;
        };
        // **Idempotent on purpose** — a click inside the dock, an RPC call and the command palette
        // all mean "focus it", and none of them mean "and release it if it already is". The release
        // is `ToggleDock` (F003/P082/T444).
        if dock_focus_outcome(focused.as_deref(), &mount, false) == DockFocus::Nothing {
            return;
        }
        // Show the region it sits in. Focusing a container the user cannot see is a promise
        // unkept — the keys go somewhere invisible. `sidebar_focus` did this and nothing else
        // did, so it moved here when that built-in was retired (F003/P085/T356).
        if let Some(region) = state.chrome_host.placement(&mount) {
            reveal_region(state, region);
        }
        state.chrome_state.set_focused_container(Some(mount));
        return;
    }
    let candidates = crate::chrome::dock_candidates(&state.chrome_host, |region| {
        crate::chrome::region_on_screen(state, region)
    });
    begin_pick(state, InputMode::DockPick { candidates });
}

/// **Focus a dock, or give the keyboard back if it already has it** — one key in and out.
///
/// This is what `global_focus` binds (`prefix+e` in, `prefix+e` out), so a binding to a named dock
/// is not a one-way door the user has to remember a second key to leave (F003/P085/T352). `Esc` is
/// the other way out, and both are deliberate.
///
/// **The toggle lives here and not in [`handle_focus_dock`]** because it belongs to the *gesture*,
/// not to the verb: pressing a key again plainly means "undo that", while a click, an RPC call and a
/// palette entry all mean "focus it" and nothing more. With the toggle inside `FocusDock`, every one
/// of those released a dock by asking to focus it (F003/P082/T444).
pub fn handle_toggle_dock(state: &mut AppState, action: &WmAction) {
    let WmAction::ToggleDock { dock } = action else {
        return;
    };
    if let Some(dock) = dock {
        let focused = state.chrome_state.focused_container();
        let last = state.chrome_state.last_focused_container();
        if let Some(mount) = crate::chrome::placement_for(
            &state.chrome_host,
            dock,
            focused.as_deref(),
            last.as_deref(),
        ) && dock_focus_outcome(focused.as_deref(), &mount, true) == DockFocus::Release
        {
            handle_unfocus_dock(state, &WmAction::UnfocusDock);
            return;
        }
    }
    // Anything else — not focused, or no dock named — is the plain focus, pick included.
    handle_focus_dock(state, &WmAction::FocusDock { dock: dock.clone() });
}

/// Give the keyboard back to the focused pane: chrome focus is released (F003/P085/T352).
///
/// The pane never lost focus — `state.focused_pane` is untouched while a dock holds the keyboard,
/// so there is nothing to restore here and nothing to guess about where the keys should land. All
/// this clears is the redirection.
///
/// A no-op when no dock is focused, so `Esc` in the focus layer and an RPC call are both safe to
/// repeat.
pub fn handle_unfocus_dock(state: &mut AppState, _action: &WmAction) {
    if state.chrome_state.focused_container().is_none() {
        return;
    }
    state.chrome_state.set_focused_container(None);
}

/// Move a mounted container to another chrome region (validated against the
/// container's `supported_regions`).
pub fn handle_move_container_to_region(state: &mut AppState, action: &WmAction) {
    let WmAction::MoveContainerToRegion {
        container_id,
        region,
    } = action
    else {
        return;
    };
    if let Err(e) = state.chrome_host.move_container(container_id, *region) {
        eprintln!("[heca] move container '{container_id}' failed: {e}");
    }
}

/// Reorder a mounted container within its region, before `before_id` (or to the
/// end when `None`).
pub fn handle_reorder_container_before(state: &mut AppState, action: &WmAction) {
    let WmAction::ReorderContainerBefore {
        container_id,
        before_id,
    } = action
    else {
        return;
    };
    if let Err(e) = state
        .chrome_host
        .reorder(container_id, before_id.as_deref())
    {
        eprintln!("[heca] reorder container '{container_id}' failed: {e}");
    }
}

/// Reorder a mounted container within its region, immediately after `after_id`.
pub fn handle_reorder_container_after(state: &mut AppState, action: &WmAction) {
    let WmAction::ReorderContainerAfter {
        container_id,
        after_id,
    } = action
    else {
        return;
    };
    if let Err(e) = state.chrome_host.reorder_after(container_id, after_id) {
        eprintln!("[heca] reorder container '{container_id}' after '{after_id}' failed: {e}");
    }
}

/// Show / hide / toggle a chrome region — the runtime side of `[settings] show_*`.
pub fn handle_set_region_visible(state: &mut AppState, action: &WmAction) {
    let WmAction::SetRegionVisible { region, visible } = *action else {
        return;
    };
    set_region_visibility(state, region, visible);
}

pub fn handle_layer_visibility(state: &mut AppState, action: &WmAction) {
    let (name, show) = match action {
        WmAction::ShowLayer { name, .. } => (name, Some(true)),
        WmAction::HideLayer { name, .. } => (name, Some(false)),
        WmAction::ToggleLayer { name, .. } => (name, None),
        _ => return,
    };
    let Some(name) = name.as_deref() else { return };
    // **Rebuild before showing.** A layer's content is structural — panes open, workspaces come and
    // go — and a signal replaces a prop, never a child. So shape follows a re-registration, and the
    // moment a layer is asked for is the moment its shape must be current. Without this the exposé
    // showed the session as it was at startup, whatever had happened since.
    if show != Some(false) {
        crate::chrome::rebuild_named_layer(state, name);
    }
    let Some(id) = state.layers.by_name(name) else {
        return;
    };
    let show = show.unwrap_or(!state.layers.is_visible_named(&state.window_root, name));
    match show {
        true => state.layers.show(&mut state.window_root, id),
        false => state.layers.hide(&mut state.window_root, id),
    }
}

#[cfg(test)]
mod dock_focus_tests;
