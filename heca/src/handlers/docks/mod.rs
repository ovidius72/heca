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

pub fn handle_sidebar_left(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    set_region_visibility(
        state,
        crate::chrome::RegionId::LeftSidebar,
        crate::input::RegionVisibility::Toggle,
    );
    Ok(())
}

pub fn handle_sidebar_right(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    set_region_visibility(
        state,
        crate::chrome::RegionId::RightSidebar,
        crate::input::RegionVisibility::Toggle,
    );
    Ok(())
}

/// Make `region` visible if it is currently hidden, so something seated in it can be seen.
///
/// A region is shown or hidden (no icon rail — see `docs/chrome-and-ui.md` § 2.5). A region that
/// is already shown keeps its (possibly user-resized) width, and revealing it again is free.
fn reveal_region(state: &mut AppState, region: crate::chrome::RegionId) {
    set_region_visibility(state, region, crate::input::RegionVisibility::Show);
}

/// Enter sidebar-nav on the dock that has keyboard navigation — **whichever region it sits in**.
///
/// This used to read `left_visible()` and expand the *left* container, which is wrong the moment the
/// dock is seated elsewhere: the workspaces container declares `RegionSet::sidebars()`, so the focus
/// key expanded an empty left sidebar and navigated a tree drawn on the right.
///
/// With no navigable dock mounted there is nothing to navigate, so this is a **no-op** — it does not
/// expand a region to show an empty frame.
/// Hand the keyboard to the dock that navigates — the overflow escape for the pane pickers.
///
/// When there are more panes than letters, a pick cannot offer them all, so the pickers fall back to
/// driving the dock instead. That is the app reaching for whichever container declares
/// `keyboard_navigable`, not for a named one. Once the pickers take their targets from a
/// component instead, this goes.
pub(crate) fn focus_navigable_dock(state: &mut AppState) {
    let focused = crate::app::tree_focus::focused_dock(state);
    let Some((dock, _)) = crate::chrome::navigable_dock(&state.chrome_host, focused.as_deref())
    else {
        return;
    };
    let _ = handle_focus_dock(state, &WmAction::FocusDock { dock: Some(dock) });
}

/// What a request aimed at the dock `mount` should do, given who holds the keyboard.
///
/// A pure function of plain data so the one rule that separates `focus` from `toggle` is testable
/// without a window, and lives in one place instead of once per handler (the shape
/// `surface_action` uses for key resolution).
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
/// release is [`handle_toggle_dock`], which is what `global_focus` binds. When the
/// toggle lived here, *every* caller inherited it: a click inside a focused dock released it, and
/// so did an RPC `focus-dock`. See [`WmAction::ToggleDock`].
pub fn handle_focus_dock(state: &mut AppState, action: &WmAction) -> crate::actions::ActionResult {
    let dock = match action {
        WmAction::FocusDock { dock } => dock,
        _ => return Ok(()),
    };
    if let Some(dock) = dock {
        // `dock` names a **placement or a component**: a `global_focus` written
        // without an `id` speaks for the component, and lands on the seating you were last in.
        let focused = crate::app::tree_focus::focused_dock(state);
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
            return Err(format!(
                "no container mounted under id or component '{dock}'"
            ));
        };
        // **Idempotent on purpose** — a click inside the dock, an RPC call and the command palette
        // all mean "focus it", and none of them mean "and release it if it already is". The release
        // is `ToggleDock`.
        if dock_focus_outcome(focused.as_deref(), &mount, false) == DockFocus::Nothing {
            return Ok(());
        }
        // Show the region it sits in. Focusing a container the user cannot see is a promise
        // unkept — the keys go somewhere invisible. `sidebar_focus` did this and nothing else
        // did, so it moved here when that built-in was retired.
        if let Some(region) = state.chrome_host.placement(&mount) {
            reveal_region(state, region);
        }
        crate::app::tree_focus::focus_dock(state, &mount);
        return Ok(());
    }
    let candidates = crate::chrome::dock_candidates(&state.chrome_host, |region| {
        crate::chrome::region_on_screen(state, region)
    });
    begin_pick(state, InputMode::DockPick { candidates });
    Ok(())
}

/// **Focus a dock, or give the keyboard back if it already has it** — one key in and out.
///
/// This is what `global_focus` binds (`prefix+e` in, `prefix+e` out), so a binding to a named dock
/// is not a one-way door the user has to remember a second key to leave. `Esc` is
/// the other way out, and both are deliberate.
///
/// **The toggle lives here and not in [`handle_focus_dock`]** because it belongs to the *gesture*,
/// not to the verb: pressing a key again plainly means "undo that", while a click, an RPC call and a
/// palette entry all mean "focus it" and nothing more. With the toggle inside `FocusDock`, every one
/// of those released a dock by asking to focus it.
pub fn handle_toggle_dock(state: &mut AppState, action: &WmAction) -> crate::actions::ActionResult {
    let WmAction::ToggleDock { dock } = action else {
        return Ok(());
    };
    if let Some(dock) = dock {
        let focused = crate::app::tree_focus::focused_dock(state);
        let last = state.chrome_state.last_focused_container();
        if let Some(mount) = crate::chrome::placement_for(
            &state.chrome_host,
            dock,
            focused.as_deref(),
            last.as_deref(),
        ) && dock_focus_outcome(focused.as_deref(), &mount, true) == DockFocus::Release
        {
            return handle_unfocus_dock(state, &WmAction::UnfocusDock);
        }
    }
    // Anything else — not focused, or no dock named — is the plain focus, pick included.
    handle_focus_dock(state, &WmAction::FocusDock { dock: dock.clone() })
}

/// Give the keyboard back to the focused pane: chrome focus is released.
///
/// The pane never lost focus — `state.focused_pane` is untouched while a dock holds the keyboard,
/// so there is nothing to restore here and nothing to guess about where the keys should land. All
/// this clears is the redirection.
///
/// A no-op when no dock is focused, so `Esc` in the focus layer and an RPC call are both safe to
/// repeat.
pub fn handle_unfocus_dock(
    state: &mut AppState,
    _action: &WmAction,
) -> crate::actions::ActionResult {
    let Some(dock) = crate::app::tree_focus::focused_dock(state) else {
        return Ok(());
    };
    crate::app::tree_focus::release_dock(state, &dock);
    Ok(())
}

/// Move a mounted container to another chrome region (validated against the
/// container's `supported_regions`).
pub fn handle_move_container_to_region(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::MoveContainerToRegion {
        container_id,
        region,
    } = action
    else {
        return Ok(());
    };
    state
        .chrome_host
        .move_container(container_id, *region)
        .map_err(|e| format!("cannot move container '{container_id}': {e}"))
}

/// Reorder a mounted container within its region, before `before_id` (or to the
/// end when `None`).
pub fn handle_reorder_container_before(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::ReorderContainerBefore {
        container_id,
        before_id,
    } = action
    else {
        return Ok(());
    };
    state
        .chrome_host
        .reorder(container_id, before_id.as_deref())
        .map_err(|e| format!("cannot reorder container '{container_id}': {e}"))
}

/// Reorder a mounted container within its region, immediately after `after_id`.
pub fn handle_reorder_container_after(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::ReorderContainerAfter {
        container_id,
        after_id,
    } = action
    else {
        return Ok(());
    };
    state
        .chrome_host
        .reorder_after(container_id, after_id)
        .map_err(|e| format!("cannot reorder container '{container_id}' after '{after_id}': {e}"))
}

/// Fold a placed dock to its header, or open it again — the header's chevron, a key, a script.
/// Unnamed, it is the dock that holds the keyboard. The region is rebuilt with the dock's new share
/// (`chrome_signature` reads the fold).
pub fn handle_toggle_container_collapsed(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::ToggleContainerCollapsed { container_id } = action else {
        return Ok(());
    };
    let Some(dock) = container_id
        .clone()
        .or_else(|| crate::app::tree_focus::focused_dock(state))
    else {
        return Ok(());
    };
    if state.chrome_host.placement(&dock).is_none() {
        return Err(format!("no dock '{dock}' is placed"));
    }
    let folded = state.chrome_state.is_container_collapsed(&dock);
    state.chrome_state.set_container_collapsed(&dock, !folded);
    Ok(())
}

/// **The least a resize leaves a dock**, in lines of the interface font — its header and a row,
/// so a dock never shrinks to nothing under the pointer. Folding is how a dock goes to its header.
const MIN_DOCK_LINES: f64 = 4.0;

/// Make a placed dock taller or shorter by `by` pixels, taking the room from its neighbour — the
/// edge between two docks while it is dragged, a key, a script.
///
/// **Shares follow what is seen.** Every open dock of the region gets the share its height on
/// screen is now — so a dock whose own body asked for a share does not jump — and then the two
/// neighbours trade `by` pixels, neither going below a minimum that follows the font. The shares
/// are then the user's, as a hand move is.
pub fn handle_resize_container(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::ResizeContainer { container_id, by } = action else {
        return Ok(());
    };
    // Unnamed, it is the dock that holds the keyboard — what a key in the resize mode means.
    let Some(container_id) = container_id
        .clone()
        .or_else(|| crate::app::tree_focus::focused_dock(state))
    else {
        return Ok(());
    };
    let container_id = &container_id;
    let Some(region) = state.chrome_host.placement(container_id) else {
        return Err(format!("no dock '{container_id}' is placed"));
    };
    let height = |id: &str| {
        heca_grid_ui::node_with_key(&state.window_root, id).map(|seat| seat.base().bounds.size.h)
    };
    // The open docks of the region, as shown, with their heights.
    let open: Vec<(String, f64)> = state
        .chrome_host
        .shown(region)
        .into_iter()
        .filter(|(id, _)| !state.chrome_state.is_container_collapsed(id))
        .filter_map(|(id, _)| Some((id.clone(), height(&id)?)))
        .collect();
    let Some(at) = open.iter().position(|(id, _)| id == container_id) else {
        return Ok(());
    };
    // **`by` moves an edge, as for panes**: the one below the dock, or the one above when it is the
    // last — so a positive `by` (down) grows the dock above an edge and shrinks the one below it.
    let (other, by) = if at + 1 < open.len() {
        (at + 1, *by)
    } else if at > 0 {
        (at - 1, -*by)
    } else {
        return Ok(());
    };
    let total: f64 = open.iter().map(|(_, h)| h).sum();
    if total <= 0.0 {
        return Ok(());
    }
    let floor = f64::from(state.app_ui_font_size()) * MIN_DOCK_LINES;
    let mut heights: Vec<f64> = open.iter().map(|(_, h)| *h).collect();
    let pair = heights[at] + heights[other];
    let grown = (heights[at] + by).clamp(floor.min(pair / 2.0), (pair - floor).max(pair / 2.0));
    heights[at] = grown;
    heights[other] = pair - grown;
    // Shares that average one, so a dock added later at its default share sits among them. Set on
    // the host (for every rebuild) and on the drawn dock itself, so the region is not rebuilt and a
    // drag of the edge goes on under the pointer.
    let count = heights.len() as f64;
    for ((id, _), h) in open.iter().zip(&heights) {
        let share = (h / total * count) as f32;
        state.chrome_host.set_share(id, share);
        if let Some(seat) = heca_grid_ui::node_with_key_mut(&mut state.window_root, id) {
            seat.base_mut().style.layout.flex = Some(share);
        }
    }
    Ok(())
}

/// Show / hide / toggle a chrome region — the runtime side of `[settings] show_*`.
pub fn handle_set_region_visible(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let WmAction::SetRegionVisible { region, visible } = *action else {
        return Ok(());
    };
    set_region_visibility(state, region, visible);
    Ok(())
}

pub fn handle_layer_visibility(
    state: &mut AppState,
    action: &WmAction,
) -> crate::actions::ActionResult {
    let (name, show) = match action {
        WmAction::ShowLayer { name, .. } => (name, Some(true)),
        WmAction::HideLayer { name, .. } => (name, Some(false)),
        WmAction::ToggleLayer { name, .. } => (name, None),
        _ => return Ok(()),
    };
    let Some(name) = name.as_deref() else {
        return Ok(());
    };
    // **Rebuild before showing.** A layer's content is structural — panes open, workspaces come and
    // go — and a signal replaces a prop, never a child. So shape follows a re-registration, and the
    // moment a layer is asked for is the moment its shape must be current. Without this the exposé
    // showed the session as it was at startup, whatever had happened since.
    if show != Some(false) {
        crate::chrome::rebuild_named_layer(state, name);
    }
    let Some(id) = state.layers.by_name(name) else {
        return Ok(());
    };
    let show = show.unwrap_or(!state.layers.is_visible_named(&state.window_root, name));
    match show {
        true => state.layers.show(&mut state.window_root, id),
        false => state.layers.hide(&mut state.window_root, id),
    }
    Ok(())
}

#[cfg(test)]
mod dock_focus_tests;
