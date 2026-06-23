//! Keyboard input-mode handlers.
//!
//! This module keeps the large input-mode dispatch out of `main.rs` while
//! preserving the existing key handling behavior.

use crate::actions::ActionRegistry;
use crate::app::interaction::InteractionSource;
use crate::app::interaction::dispatch_action;
use crate::app::keyboard::{
    event_combo_matches, normalize_key_text, prefix_combo_to_literal_input, typed_candidate_char,
    winit_key_to_backend_event, winit_key_to_terminal_input,
};
use crate::app::mutations::after_metadata_change;
use crate::app::selection::find_pane_location;
use crate::app_state::{AppState, InputMode, RenameTarget, WorkspacePickTarget};
use crate::input::WmAction;
use crate::keymap::{KeyCombo, KeymapRegistry};
use heca_core::layout::PaneId;
use std::collections::HashMap;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

#[derive(Clone, Copy)]
pub(crate) struct KeyInputContext<'a> {
    pub logical_key: &'a Key,
    pub physical_key: &'a PhysicalKey,
    pub key_text: &'a str,
    pub event_combo: &'a KeyCombo,
    pub is_prefix: bool,
    pub is_ctrl: bool,
    pub is_shift: bool,
}

pub(crate) fn handle_keyboard_input(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    if handle_rename_input(state, ctx) {
        return;
    }

    if handle_confirm_delete_input(registry, state, ctx) {
        return;
    }

    let input_mode = state.input_mode.clone();
    match input_mode {
        InputMode::Normal => {
            if ctx.is_prefix {
                state.input_mode = InputMode::Prefix;
                state.prefix_entered_at = Some(std::time::Instant::now());
                state.needs_redraw = true;
                return;
            }

            let global_action = keymap.resolve("global", ctx.event_combo).cloned();
            if let Some(act) = global_action {
                dispatch_action(state, registry, InteractionSource::Keyboard, &act);
                return;
            }

            if let Some(pane_id) = state.focused_pane
                && let Some(backend) = state.backends.get_mut(pane_id)
            {
                let handled = winit_key_to_backend_event(ctx.logical_key, state.modifiers)
                    .is_some_and(|event| backend.process_key_event(&event));
                let input_bytes =
                    winit_key_to_terminal_input(ctx.logical_key, ctx.key_text, ctx.is_ctrl);
                if !handled && !input_bytes.is_empty() {
                    backend.process_input(&input_bytes);
                }
            }
        }
        InputMode::Prefix => {
            handle_prefix_mode(registry, keymap, mode_triggers, state, ctx);
        }
        InputMode::Chord { sequence } => {
            handle_chord_mode(registry, state, &sequence, ctx);
        }
        InputMode::Mode { name } => {
            handle_custom_mode(registry, mode_keymaps, mode_triggers, state, &name, ctx);
        }
        InputMode::PaneSelect { candidates } => {
            handle_pane_select_mode(registry, state, &candidates, ctx);
        }
        InputMode::PaneSwap {
            candidates,
            focus_after,
        } => {
            handle_pane_swap_mode(registry, state, &candidates, focus_after, ctx);
        }
        InputMode::PaneTake {
            candidates,
            focus_after,
        } => {
            handle_pane_take_mode(registry, state, &candidates, focus_after, ctx);
        }
        InputMode::WorkspacePick { candidates, target } => {
            handle_workspace_pick_mode(registry, state, &candidates, target, ctx);
        }
        InputMode::ColumnPick {
            candidates,
            pane_id,
        } => {
            handle_column_pick_mode(registry, state, &candidates, pane_id, ctx);
        }
        InputMode::SidebarNav => {
            handle_sidebar_nav_mode(registry, mode_keymaps, state, ctx);
        }
        InputMode::Selection => {
            handle_selection_mode(registry, mode_keymaps, state, ctx);
        }
        _ => {}
    }
}

fn handle_rename_input(state: &mut AppState, ctx: KeyInputContext<'_>) -> bool {
    let InputMode::Rename { target, buffer } = &mut state.input_mode else {
        return false;
    };

    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));
    let is_backspace = matches!(ctx.logical_key, Key::Named(NamedKey::Backspace));

    if is_escape {
        state.input_mode = InputMode::Normal;
    } else if is_enter {
        let new_name = buffer.trim().to_string();
        match target {
            RenameTarget::Workspace(ws_idx) => {
                if let Some(ws) = state.session.workspaces.get_mut(*ws_idx) {
                    ws.name = if new_name.is_empty() {
                        None
                    } else {
                        Some(new_name)
                    };
                }
            }
            RenameTarget::Column { ws_idx, col_idx } => {
                if let Some(ws) = state.session.workspaces.get_mut(*ws_idx)
                    && let Some(col) = ws.scrolling.columns.get_mut(*col_idx)
                {
                    col.name = if new_name.is_empty() {
                        None
                    } else {
                        Some(new_name)
                    };
                }
            }
            RenameTarget::Pane(pane_id) => {
                if let Some(ws) = state.session.active_workspace_mut()
                    && let Some(pane) = ws.find_pane_mut(*pane_id)
                {
                    // Set a user override that wins over the process-derived name; an
                    // empty entry clears it so the name tracks the process again.
                    pane.custom_name = if new_name.is_empty() {
                        None
                    } else {
                        Some(new_name)
                    };
                }
            }
        }
        after_metadata_change(state);
        state.input_mode = InputMode::Normal;
    } else if is_backspace {
        buffer.pop();
    } else if ctx.key_text.len() == 1 && !ctx.is_ctrl {
        buffer.push_str(ctx.key_text);
    }

    state.needs_redraw = true;
    true
}

fn handle_confirm_delete_input(
    registry: &ActionRegistry,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) -> bool {
    let (action, resume_sidebar) = match &state.input_mode {
        InputMode::ConfirmDelete {
            action,
            resume_sidebar,
            ..
        } => (action.as_ref().clone(), *resume_sidebar),
        _ => return false,
    };

    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_y = ctx.key_text == "y" || ctx.key_text == "Y";
    let is_n = ctx.key_text == "n" || ctx.key_text == "N";
    let resume_mode = if resume_sidebar {
        InputMode::SidebarNav
    } else {
        InputMode::Normal
    };

    if is_escape || is_n {
        state.input_mode = resume_mode;
    } else if is_y {
        state.input_mode = resume_mode;
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
    }
    state.needs_redraw = true;
    true
}

fn handle_prefix_mode(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    if ctx.is_prefix {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        if let Some(pane_id) = state.focused_pane
            && let Some(backend) = state.backends.get_mut(pane_id)
        {
            let literal = prefix_combo_to_literal_input(&state.prefix_combo);
            if !literal.is_empty() {
                backend.process_input(&literal);
            }
        }
        return;
    }

    let is_modifier_only = ctx.key_text.is_empty()
        && matches!(
            ctx.logical_key,
            Key::Named(
                NamedKey::Shift
                    | NamedKey::Control
                    | NamedKey::Alt
                    | NamedKey::Super
                    | NamedKey::Hyper
                    | NamedKey::Meta
            )
        );
    if is_modifier_only {
        return;
    }

    let combo = mode_combo(ctx);
    let mut entered_mode = None;
    for (mode_name, (trigger_combo, sticky)) in mode_triggers {
        if event_combo_matches(&combo, trigger_combo) {
            entered_mode = Some((mode_name.clone(), *sticky));
            break;
        }
    }
    if let Some((mode_name, _sticky)) = entered_mode {
        state.input_mode = InputMode::Mode { name: mode_name };
        state.prefix_entered_at = None;
        state.needs_redraw = true;
        return;
    }

    let action = keymap.resolve("normal", &combo).cloned();
    if let Some(ref act) = action {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        dispatch_action(state, registry, InteractionSource::Keyboard, act);
    } else if !ctx.key_text.is_empty() {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
    }
}

fn handle_chord_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    sequence: &[String],
    ctx: KeyInputContext<'_>,
) {
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    if is_escape {
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
        return;
    }

    if sequence.len() == 1
        && sequence[0].eq_ignore_ascii_case("w")
        && let Some(digit) = ctx
            .key_text
            .chars()
            .next()
            .filter(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
    {
        let ws_idx = (digit as usize).saturating_sub(1);
        if ws_idx < state.session.workspaces.len() {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusWorkspace { ws_idx },
            );
            state.needs_redraw = true;
        }
        state.input_mode = InputMode::Normal;
        return;
    }

    state.input_mode = InputMode::Normal;
    state.needs_redraw = true;
}

fn handle_custom_mode(
    registry: &ActionRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    name: &str,
    ctx: KeyInputContext<'_>,
) {
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));
    if is_escape || is_enter {
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
        return;
    }

    let combo = mode_combo(ctx);
    if let Some(mode_map) = mode_keymaps.get(name)
        && let Some(action) = mode_map.resolve(name, &combo).cloned()
    {
        let sticky = mode_triggers.get(name).map(|(_, s)| *s).unwrap_or(true);
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
        if !sticky {
            state.input_mode = InputMode::Normal;
            state.needs_redraw = true;
        }
    }
}

fn handle_pane_select_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::FocusPane {
                pane_id: *target_id,
            },
        );
    }
    state.input_mode = InputMode::Normal;
}

fn handle_pane_swap_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    focus_after: bool,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    let current_id = state.focused_pane;
    if let Some(ch) = typed
        && let Some(current_id) = current_id
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
        && let Some((_, _, _)) = find_pane_location(&state.session, current_id)
        && let Some((_, _, _)) = find_pane_location(&state.session, *target_id)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::Swap {
                a_id: current_id,
                b_id: *target_id,
            },
        );

        if focus_after {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusPane {
                    pane_id: current_id,
                },
            );
        } else {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusPane {
                    pane_id: *target_id,
                },
            );
        }
    }
    state.needs_redraw = true;
}

fn handle_pane_take_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    focus_after: bool,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::TakePane {
                pane_id: *target_id,
                focus_after,
            },
        );
    }
    state.needs_redraw = true;
}

/// Resolve a [`InputMode::WorkspacePick`] keypress: a matching candidate letter moves
/// the captured `target` (active column or pane) into that workspace; any other key
/// (e.g. Esc) just exits the mode. Mirrors [`handle_pane_swap_mode`].
fn handle_workspace_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, usize)],
    target: WorkspacePickTarget,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, target_ws)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        let target_ws = *target_ws;
        let action = match target {
            WorkspacePickTarget::Column {
                ws_idx: origin_ws,
                col_idx,
            } => {
                // `move_column_to_workspace` resolves the source column against the
                // active workspace, so re-activate the captured origin first in case
                // the active workspace drifted while the pick was open.
                if state.session.active_workspace_idx != origin_ws {
                    crate::app::focus::switch_workspace_tracked(state, origin_ws);
                }
                WmAction::MoveColumnToWorkspace {
                    col_idx,
                    ws_idx: target_ws,
                    focus: true,
                }
            }
            WorkspacePickTarget::Pane(pane_id) => WmAction::MovePaneToWorkspace {
                pane_id,
                ws_idx: target_ws,
            },
        };
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
    }
    state.needs_redraw = true;
}

/// Resolve a [`InputMode::ColumnPick`] keypress: a matching candidate letter moves the
/// captured pane into that column of the active workspace (stacking with its panes);
/// any other key (e.g. Esc) exits the mode.
fn handle_column_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, usize, usize)],
    pane_id: PaneId,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, ws_idx, col_idx)) = candidates.iter().find(|(c, _, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::MovePaneToColumn {
                pane_id,
                ws_idx: *ws_idx,
                col_idx: *col_idx,
            },
        );
    }
    state.needs_redraw = true;
}

fn handle_sidebar_nav_mode(
    registry: &ActionRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));

    if is_escape {
        if let Some(item) = state.sidebar_tree.current_item().cloned()
            && let Some(pane_id) = sidebar_item_focus_target(&state.session, &item)
        {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusPane { pane_id },
            );
        }
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
    } else if is_enter {
        let item = state.sidebar_tree.current_item().cloned();
        match item {
            Some(crate::sidebar::SidebarItem::Pane { pane_id })
            | Some(crate::sidebar::SidebarItem::FloatingPane { pane_id, .. }) => {
                dispatch_action(
                    state,
                    registry,
                    InteractionSource::Keyboard,
                    &WmAction::FocusPane { pane_id },
                );
                state.input_mode = InputMode::Normal;
            }
            Some(crate::sidebar::SidebarItem::Workspace { ws_idx })
            | Some(crate::sidebar::SidebarItem::Column { ws_idx, .. })
                if ws_idx != state.session.active_workspace_idx =>
            {
                dispatch_action(
                    state,
                    registry,
                    InteractionSource::Keyboard,
                    &WmAction::FocusWorkspace { ws_idx },
                );
            }
            _ => {}
        }
        state.needs_redraw = true;
    } else {
        let combo = mode_combo(ctx);
        let action = mode_keymaps
            .get("sidebar")
            .and_then(|mode_map| mode_map.resolve("sidebar", &combo).cloned());
        if let Some(act) = action {
            dispatch_action(state, registry, InteractionSource::Keyboard, &act);
        }
    }
}

fn sidebar_item_focus_target(
    session: &heca_core::layout::Session,
    item: &crate::sidebar::SidebarItem,
) -> Option<PaneId> {
    match item {
        crate::sidebar::SidebarItem::Pane { pane_id }
        | crate::sidebar::SidebarItem::FloatingPane { pane_id, .. } => Some(*pane_id),
        crate::sidebar::SidebarItem::Column { ws_idx, col_idx } => session
            .workspaces
            .get(*ws_idx)
            .and_then(|ws| ws.scrolling.columns.get(*col_idx))
            .and_then(|col| col.active_pane().or_else(|| col.panes.first()))
            .map(|pane| pane.id),
        crate::sidebar::SidebarItem::Workspace { ws_idx } => session
            .workspaces
            .get(*ws_idx)
            .and_then(workspace_focus_target),
    }
}

fn workspace_focus_target(ws: &heca_core::layout::Workspace) -> Option<PaneId> {
    ws.active_pane()
        .map(|pane| pane.id)
        .or_else(|| {
            ws.scrolling
                .columns
                .iter()
                .find_map(|col| col.active_pane().or_else(|| col.panes.first()))
                .map(|pane| pane.id)
        })
        .or_else(|| ws.floating_panes.first().map(|float| float.pane.id))
}

fn handle_selection_mode(
    registry: &ActionRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    // Selection mode keyboard contract (Task 05 — keyboard-first selection):
    //   Esc     → clear selection/caret and return to Normal.
    //   Enter   → if selection exists: confirm and return to Normal.
    //             if caret-only: just return to Normal (no selection to confirm).
    //   prefix  → return to Prefix mode AND arm the prefix timeout.
    //   mode bindings → resolve through the `selection` mode keymap and
    //                   dispatch real actions via the registry.
    //   other keys → ignored; do not forward to the focused backend.
    //
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));

    if is_escape {
        // Route through the action architecture — no direct selection-state
        // mutation here, consistent with the "no registry bypasses" rule.
        // We set the mode to Normal first so the dispatched handler runs
        // against a consistent state.
        state.input_mode = InputMode::Normal;
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::ClearSelection,
        );
    } else if is_enter {
        // Enter confirms the selection and returns to Normal.
        // `selection.end()` is a mode-internal state transition (Selecting → Selected),
        // not a user-visible action. Unlike `ClearSelection` (which is exposed as a
        // WmAction because it can be triggered from keyboard/mouse/RPC), confirming
        // a selection only happens via Enter in selection mode — there is no
        // `ConfirmSelection` action because the confirmation is a mode-internal
        // gesture (like Enter in rename or confirm-delete modes).
        if state.selection.is_active() {
            state.selection.end();
        }
        // Whether we had a selection or just a caret, return to Normal.
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
    } else if ctx.is_prefix {
        // Match the Normal→Prefix promotion exactly: set the mode AND
        // arm the timeout. `handle_prefix_mode` will not arm it later.
        state.input_mode = InputMode::Prefix;
        state.prefix_entered_at = Some(std::time::Instant::now());
        state.needs_redraw = true;
    } else {
        let combo = mode_combo(ctx);
        if let Some(mode_map) = mode_keymaps.get("selection")
            && let Some(action) = mode_map.resolve("selection", &combo).cloned()
        {
            dispatch_action(state, registry, InteractionSource::Keyboard, &action);
        }
    }
}

fn mode_combo(ctx: KeyInputContext<'_>) -> KeyCombo {
    KeyCombo {
        key: normalize_key_text(
            ctx.logical_key,
            ctx.key_text,
            ctx.is_shift,
            ctx.is_ctrl,
            ctx.physical_key,
        ),
        ctrl: ctx.is_ctrl,
        shift: ctx.is_shift,
        alt: false,
        super_: false,
    }
}

#[cfg(test)]
mod tests {
    use super::sidebar_item_focus_target;
    use crate::sidebar::SidebarItem;
    use heca_core::layout::column::Pane;
    use heca_core::layout::types::{LayoutOptions, Point, Rectangle, Size};
    use heca_core::layout::{FocusDomain, PaneId, Session, SessionId};

    fn make_session() -> Session {
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        session.add_pane(Pane::new(PaneId(1), "pane-1"), None, true);
        session.add_pane(Pane::new(PaneId(2), "pane-2"), Some(0), true);
        session.add_pane(Pane::new(PaneId(3), "pane-3"), None, true);
        session
    }

    #[test]
    fn workspace_focus_target_prefers_active_pane() {
        let session = make_session();
        let target = sidebar_item_focus_target(&session, &SidebarItem::Workspace { ws_idx: 0 });
        assert_eq!(target, Some(PaneId(3)));
    }

    #[test]
    fn column_focus_target_uses_column_active_pane() {
        let session = make_session();
        let target = sidebar_item_focus_target(
            &session,
            &SidebarItem::Column {
                ws_idx: 0,
                col_idx: 0,
            },
        );
        assert_eq!(target, Some(PaneId(2)));
    }

    #[test]
    fn floating_and_pane_items_target_their_exact_pane() {
        let mut session = make_session();
        let ws = session
            .active_workspace_mut()
            .expect("active workspace exists");
        ws.floating_panes
            .push(heca_core::layout::workspace::FloatingPane {
                pane: Pane::new(PaneId(99), "float"),
                position: Point::new(0.0, 0.0),
                size: Size::new(200.0, 100.0),
                is_active: false,
                original_column_idx: None,
                original_pane_idx: None,
            });
        ws.focus_domain = FocusDomain::Tiled;

        assert_eq!(
            sidebar_item_focus_target(&session, &SidebarItem::Pane { pane_id: PaneId(2) }),
            Some(PaneId(2))
        );
        assert_eq!(
            sidebar_item_focus_target(
                &session,
                &SidebarItem::FloatingPane {
                    pane_id: PaneId(99),
                    ws_idx: 0,
                },
            ),
            Some(PaneId(99))
        );
    }

    #[test]
    fn workspace_focus_target_falls_back_to_first_floating_pane() {
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        let ws = session
            .active_workspace_mut()
            .expect("active workspace exists");
        ws.floating_panes
            .push(heca_core::layout::workspace::FloatingPane {
                pane: Pane::new(PaneId(77), "float-only"),
                position: Point::new(0.0, 0.0),
                size: Size::new(200.0, 100.0),
                is_active: false,
                original_column_idx: None,
                original_pane_idx: None,
            });
        ws.update_working_area(Rectangle::new(
            Point::new(0.0, 0.0),
            Size::new(1280.0, 800.0),
        ));

        let target = sidebar_item_focus_target(&session, &SidebarItem::Workspace { ws_idx: 0 });
        assert_eq!(target, Some(PaneId(77)));
    }

    // Note: `handle_selection_mode` is not unit-tested directly because it
    // requires a fully-constructed `AppState` (winit window + wgpu device).
    // Its contracts are verified by:
    //   - the `selection_model` unit tests (clear/end/SelectionState lifecycle)
    //   - the registry integration (ClearSelection is registered and routed
    //     through `build_registry()`)
    //   - the keymap test `default_selection_bindings_resolve` (the
    //     `prefix+s` binding reaches the action surface)
    //   - the `cargo check`/`cargo clippy` builds (compile-time
    //     exhaustiveness of the `InputMode::Selection` arm and the
    //     `action_from_name` mapping)
    //
    // The two coordinator-flagged regressions are structurally prevented by
    // the implementation:
    //   - `handle_selection_mode`'s `ctx.is_prefix` arm sets
    //     `state.prefix_entered_at = Some(Instant::now())` alongside the
    //     `InputMode::Prefix` transition, so the timeout in
    //     `lifecycle::handle_about_to_wait` is armed and the prefix mode
    //     cannot get stuck.
    //   - The Esc path dispatches `WmAction::ClearSelection` through
    //     `dispatch_action(...)` instead of calling
    //     `state.selection.clear()` directly, so the new action surface
    //     is the only entry point for clearing the selection.
}
