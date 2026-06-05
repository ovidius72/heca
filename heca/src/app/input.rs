//! Keyboard input-mode handlers.
//!
//! This module keeps the large input-mode dispatch out of `main.rs` while
//! preserving the existing key handling behavior.

use crate::actions::ActionRegistry;
use crate::app::focus::sync_focus;
use crate::app::keyboard::{
    event_combo_matches, normalize_key_text, prefix_combo_to_literal_input, typed_candidate_char,
    winit_key_to_terminal_input,
};
use crate::app::selection::find_pane_location;
use crate::app_state::{AppState, InputMode, RenameTarget};
use crate::input::WmAction;
use crate::keymap::{KeyCombo, KeymapRegistry};
use std::collections::HashMap;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

/// Check if a Key::Character matches a given lowercase letter.
fn char_key(key: &Key, ch: char) -> bool {
    matches!(key, Key::Character(c) if c.eq_ignore_ascii_case(&ch.to_string()))
}

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
                registry.execute(&act, state);
                return;
            }

            if let Some(pane_id) = state.focused_pane
                && let Some(backend) = state.backends.get_mut(&pane_id)
            {
                let input_bytes =
                    winit_key_to_terminal_input(ctx.logical_key, ctx.key_text, ctx.is_ctrl);
                if !input_bytes.is_empty() {
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
        InputMode::SidebarNav => {
            handle_sidebar_nav_mode(registry, keymap, state, ctx);
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
                    && let Some(pane) = ws.find_pane_mut(heca_core::layout::PaneId(*pane_id))
                {
                    pane.title = if new_name.is_empty() {
                        format!("pane{}", pane_id)
                    } else {
                        new_name
                    };
                }
            }
        }
        sync_focus(state);
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
    let InputMode::ConfirmDelete { action, .. } = &state.input_mode else {
        return false;
    };

    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_y = ctx.key_text == "y" || ctx.key_text == "Y";
    let is_n = ctx.key_text == "n" || ctx.key_text == "N";

    if is_escape || is_n {
        state.input_mode = InputMode::Normal;
    } else if is_y {
        let action = action.as_ref().clone();
        state.input_mode = InputMode::Normal;
        registry.execute(&action, state);
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
            && let Some(backend) = state.backends.get_mut(&pane_id)
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
        registry.execute(act, state);
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
            registry.execute(&WmAction::FocusWorkspace { ws_idx }, state);
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
        registry.execute(&action, state);
        if !sticky {
            state.input_mode = InputMode::Normal;
            state.needs_redraw = true;
        }
    }
}

fn handle_pane_select_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, u64)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        registry.execute(
            &WmAction::FocusPane {
                pane_id: *target_id,
            },
            state,
        );
    }
    state.input_mode = InputMode::Normal;
}

fn handle_pane_swap_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, u64)],
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
        registry.execute(
            &WmAction::Swap {
                a_id: current_id,
                b_id: *target_id,
            },
            state,
        );

        if focus_after {
            registry.execute(
                &WmAction::FocusPane {
                    pane_id: current_id,
                },
                state,
            );
        } else {
            registry.execute(
                &WmAction::FocusPane {
                    pane_id: *target_id,
                },
                state,
            );
        }
    }
    state.needs_redraw = true;
}

fn handle_pane_take_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, u64)],
    focus_after: bool,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        registry.execute(
            &WmAction::TakePane {
                pane_id: *target_id,
                focus_after,
            },
            state,
        );
    }
    state.needs_redraw = true;
}

fn handle_sidebar_nav_mode(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));
    let is_a = !ctx.is_ctrl && char_key(ctx.logical_key, 'a');
    let is_n = !ctx.is_ctrl && char_key(ctx.logical_key, 'n');
    let is_d = !ctx.is_ctrl && char_key(ctx.logical_key, 'd');
    let is_ctrl_j = ctx.is_ctrl && char_key(ctx.logical_key, 'j');
    let is_ctrl_k = ctx.is_ctrl && char_key(ctx.logical_key, 'k');

    if is_escape {
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
    } else if is_enter {
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(crate::sidebar::SidebarItem::Pane { pane_id }) => {
                let target_pane_id = heca_core::layout::PaneId(*pane_id);
                let target_ws = state
                    .session
                    .workspaces
                    .iter()
                    .position(|ws| ws.find_pane(target_pane_id).is_some());
                if let Some(ws_idx) = target_ws {
                    if ws_idx != state.session.active_workspace_idx {
                        registry.execute(&WmAction::FocusWorkspace { ws_idx }, state);
                    }
                    registry.execute(&WmAction::FocusPane { pane_id: *pane_id }, state);
                }
            }
            Some(crate::sidebar::SidebarItem::Workspace { .. }) => {
                let ws_idx = state
                    .sidebar_tree
                    .cursor_workspace_index()
                    .unwrap_or(state.session.active_workspace_idx);
                if ws_idx != state.session.active_workspace_idx {
                    registry.execute(&WmAction::FocusWorkspace { ws_idx }, state);
                }
            }
            _ => {}
        }
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
    } else if is_a {
        // Add new workspace with a column and pane.
        registry.execute(&WmAction::CreateWorkspace, state);
        // After create, the new workspace is active. Rebuild sidebar tree.
        sync_focus(state);
        state.needs_redraw = true;
    } else if is_n {
        // Context-sensitive "new" — add child item based on cursor.
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(crate::sidebar::SidebarItem::Workspace { ws_idx }) => {
                // Switch to workspace, then split horizontally (add column).
                if *ws_idx != state.session.active_workspace_idx {
                    registry.execute(&WmAction::FocusWorkspace { ws_idx: *ws_idx }, state);
                }
                registry.execute(&WmAction::SplitHorizontal, state);
            }
            Some(crate::sidebar::SidebarItem::Column { ws_idx, col_idx }) => {
                // Add a pane to this column.
                registry.execute(
                    &WmAction::AddPaneToColumn {
                        ws_idx: *ws_idx,
                        col_idx: *col_idx,
                    },
                    state,
                );
            }
            _ => {}
        }
        sync_focus(state);
        state.needs_redraw = true;
    } else if is_d {
        // Context-sensitive delete.
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(crate::sidebar::SidebarItem::Pane { pane_id }) => {
                registry.execute(&WmAction::ClosePaneById { pane_id: *pane_id }, state);
            }
            Some(crate::sidebar::SidebarItem::Column { ws_idx, col_idx }) => {
                let ws_idx = *ws_idx;
                let col_idx = *col_idx;
                let ws_label = state
                    .session
                    .workspaces
                    .get(ws_idx)
                    .and_then(|ws| ws.name.clone())
                    .unwrap_or_else(|| format!("ws {}", ws_idx + 1));
                state.input_mode = InputMode::ConfirmDelete {
                    message: format!("Delete column {} from {}? (y/n)", col_idx + 1, ws_label),
                    action: Box::new(WmAction::DeleteColumn { ws_idx, col_idx }),
                };
                state.needs_redraw = true;
                return;
            }
            Some(crate::sidebar::SidebarItem::Workspace { ws_idx }) => {
                let ws_idx = *ws_idx;
                let ws_label = state
                    .session
                    .workspaces
                    .get(ws_idx)
                    .and_then(|ws| ws.name.clone())
                    .unwrap_or_else(|| format!("workspace {}", ws_idx + 1));
                state.input_mode = InputMode::ConfirmDelete {
                    message: format!("Delete {}? (y/n)", ws_label),
                    action: Box::new(WmAction::DeleteWorkspace { ws_idx }),
                };
                state.needs_redraw = true;
                return;
            }
            _ => {}
        }
        sync_focus(state);
        state.needs_redraw = true;
    } else if is_ctrl_j {
        // Context-sensitive move/navigate down.
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(crate::sidebar::SidebarItem::Pane { .. }) => {
                registry.execute(&WmAction::SwapDown, state);
            }
            Some(crate::sidebar::SidebarItem::Column { .. }) => {
                registry.execute(&WmAction::MoveColumnDown, state);
            }
            Some(crate::sidebar::SidebarItem::Workspace { .. }) => {
                registry.execute(&WmAction::WorkspaceNext, state);
            }
            _ => {}
        }
        sync_focus(state);
        state.needs_redraw = true;
    } else if is_ctrl_k {
        // Context-sensitive move/navigate up.
        let item = state.sidebar_tree.current_item().cloned();
        match &item {
            Some(crate::sidebar::SidebarItem::Pane { .. }) => {
                registry.execute(&WmAction::SwapUp, state);
            }
            Some(crate::sidebar::SidebarItem::Column { .. }) => {
                registry.execute(&WmAction::MoveColumnUp, state);
            }
            Some(crate::sidebar::SidebarItem::Workspace { .. }) => {
                registry.execute(&WmAction::WorkspacePrev, state);
            }
            _ => {}
        }
        sync_focus(state);
        state.needs_redraw = true;
    } else {
        let combo = mode_combo(ctx);
        let action = keymap.resolve("sidebar", &combo).cloned();
        if let Some(act) = action {
            registry.execute(&act, state);
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
