//! Keyboard input-mode handlers.
//!
//! This module keeps the large input-mode dispatch out of `main.rs` while
//! preserving the existing key handling behavior.

use crate::actions::ActionRegistry;
use crate::app::interaction::InteractionSource;
use crate::app::interaction::dispatch_action_ref;
use crate::app::keyboard::{winit_key_to_backend_event, winit_key_to_terminal_input};
use crate::app_state::{AppState, InputMode};
use crate::keymap::{KeyCombo, Keymaps};
use winit::keyboard::{Key, PhysicalKey};

mod modes;
mod pick_modes;
mod surface;

use modes::{SELECTION_MODE, handle_chord_mode, handle_mode, handle_prefix_mode};
use pick_modes::{
    handle_column_pick_mode, handle_dock_pick_mode, handle_follow_link_mode, handle_hint_pick_mode,
    handle_pane_select_mode, handle_pane_swap_mode, handle_pane_take_mode,
    handle_workspace_pick_mode,
};
pub(crate) use surface::{
    FOCUS_LAYER, FocusedSurface, LAYER_FLOOR, focused_surface, surface_action, way_out_action,
};

#[derive(Clone, Copy)]
pub(crate) struct KeyInputContext<'a> {
    pub logical_key: &'a Key,
    pub physical_key: &'a PhysicalKey,
    pub key_text: &'a str,
    pub event_combo: &'a KeyCombo,
    pub is_prefix: bool,
    pub is_ctrl: bool,
    pub is_shift: bool,
    /// Whether the window tree has already been offered this key — a layer in front is offered it
    /// before the key rules run, so they must not offer it again.
    pub offered_to_tree: bool,
}

pub(crate) fn handle_keyboard_input(
    registry: &ActionRegistry,
    keymaps: &Keymaps,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    let (keymap, mode_keymaps, component_keymaps, mode_triggers) = (
        &keymaps.flat,
        &keymaps.modes,
        &keymaps.components,
        &keymaps.triggers,
    );
    // **The last key's reply is answered by the next key.** Cleared here, before this press is
    // dispatched, so a note set by *this* handler survives to be read — and is gone the moment you
    // do anything else. No timer, and nothing to schedule: a stale line in a status bar costs
    // nothing, which is the whole reason it is not a toast.
    if state.status_note.take().is_some() {
        state.needs_redraw = true;
    }
    let input_mode = state.input_mode.clone();
    // **A picker is waiting for one letter, and a modifier is not it.** Every pick mode ends on the
    // next key — picked, wrong key, or Esc — so reaching for Shift to type a capital would cancel
    // the picker before the letter arrived. Asked once here rather than inside each mode's handler:
    // there are seven of them, and guarding them one at a time reached three.
    if input_mode.awaits_pick_letter() && crate::app::keyboard::is_modifier_key(ctx.logical_key) {
        return;
    }
    match input_mode {
        InputMode::Normal => {
            if ctx.is_prefix {
                // Nothing is stashed across the transition any more (F003/P086/T365): the context
                // a menu opens for is the focused container's cursor row, and chrome focus is not
                // a mode — the transition to `Prefix` does not touch it — so `handle_open_context_menu`
                // resolves it for itself when the action actually runs.
                state.input_mode = InputMode::Prefix;
                state.prefix_entered_at = Some(std::time::Instant::now());
                state.needs_redraw = true;
                return;
            }

            // **A key acts on the surface in front of you** (F003/P082/T428), in the DOM's order:
            // the key goes to the focused widget first and bubbles up, and the surface's own keys
            // are its handlers on the way — they act on what the tree did not take.
            //
            // **One exception, the way out.** The key that leaves a focused dock is reserved and
            // answered before the tree, like a browser's reserved shortcuts: a terminal that holds
            // the keyboard can eat every key it is sent, and it must not be able to trap you in
            // the dock. Everything else the dock's keymap says — paging, the sidebar's `j`/`k` — is
            // consulted after the tree, so a program in a docked terminal gets its PageUp.
            let surface = focused_surface(state);
            if let Some(act) = way_out_action(&surface, mode_keymaps, ctx.event_combo) {
                dispatch_action_ref(state, registry, InteractionSource::Keyboard, &act);
                return;
            }
            // What the surface holds goes to the tree. (A layer was offered the key before the key
            // rules ran, by whoever holds it; the panes never are — their keys belong to the
            // program in the pane.)
            if surface.delivers_to_tree()
                && !ctx.offered_to_tree
                && crate::app::tree_keys::deliver_press_to_tree(
                    state,
                    ctx.event_combo,
                    ctx.key_text,
                ) == Some(heca_grid_ui::Handled::Yes)
            {
                state.mark_full_redraw();
                return;
            }
            if let Some(act) =
                surface_action(&surface, mode_keymaps, component_keymaps, ctx.event_combo)
            {
                // A layer's intents are stamped with the surface that owns them, which is what lets
                // the policy tell the map acting on itself from the app being driven behind it.
                let source = match state.layers.top_modal_id(&state.window_root) {
                    Some(id) if matches!(surface, FocusedSurface::Layer { .. }) => {
                        InteractionSource::Surface(state.layers.surface_key(id))
                    }
                    _ => InteractionSource::Keyboard,
                };
                dispatch_action_ref(state, registry, source, &act);
                return;
            }

            let global_action = keymap
                .resolve(crate::keymap::DIRECT_LAYER, ctx.event_combo)
                .cloned();
            if let Some(act) = global_action {
                dispatch_action_ref(state, registry, InteractionSource::Keyboard, &act);
                return;
            }

            // **Focus is the mode.** A layer or a dock holding the keyboard **swallows** what it did
            // not claim rather than forwarding it. A `j` leaking into a shell while the user is
            // driving a sidebar is the worse failure — and the focus ring plus the status bar are
            // what stop the swallowing being silent. Forwarding to the dock instead is how `j`/`k`
            // went on moving the sidebar cursor underneath an open context menu (Antonio,
            // 2026-08-10).
            //
            // `state.focused_pane` is deliberately untouched: only the keyboard is redirected, so
            // `prefix+Enter` still splits the pane you last worked in.
            //
            if !matches!(surface, FocusedSurface::Panes) {
                return;
            }

            if let Some(pane_id) = state.focused_pane
                && let Some(backend) = state.backends.get_mut(pane_id)
            {
                // Snap to live bottom when user sends keyboard input (Q5).
                // Skip modifier-only keys (Shift, Ctrl, Alt alone) so that
                // e.g. Shift+click mouse selection works after scrolling
                // with direct bindings.
                let is_modifier_only = ctx.key_text.is_empty()
                    && crate::app::keyboard::is_modifier_key(ctx.logical_key);
                if !is_modifier_only {
                    backend.scroll_to_bottom();
                }

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
            handle_prefix_mode(
                registry,
                keymap,
                mode_keymaps,
                component_keymaps,
                mode_triggers,
                state,
                ctx,
            );
        }
        InputMode::Chord { sequence } => {
            handle_chord_mode(registry, state, &sequence, ctx);
        }
        InputMode::Mode { name } => {
            handle_mode(
                registry,
                keymap,
                mode_keymaps,
                mode_triggers,
                state,
                &name,
                ctx,
            );
        }
        InputMode::PaneSelect { candidates } => {
            handle_pane_select_mode(registry, state, &candidates, ctx);
        }
        InputMode::FollowLink { candidates } => {
            handle_follow_link_mode(registry, state, &candidates, ctx);
        }
        InputMode::HintPick { candidates } => {
            handle_hint_pick_mode(registry, state, &candidates, ctx);
        }
        InputMode::Search { terminal } => {
            handle_search_mode(state, terminal, ctx);
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
        InputMode::DockPick { candidates } => {
            handle_dock_pick_mode(registry, state, &candidates, ctx);
        }
        InputMode::Selection => {
            handle_mode(
                registry,
                keymap,
                mode_keymaps,
                mode_triggers,
                state,
                SELECTION_MODE,
                ctx,
            );
        }
        _ => {}
    }
}

/// **A terminal's search field has the keyboard**, so what is typed goes into the terminal's own
/// tree, where the field takes it through the same rule as every other field: text as text, the
/// editing shortcuts as the intents they resolve to. Enter and Escape come back as the terminal's
/// own messages (keep the matches, dismiss the search), which is also what leaves this mode.
fn handle_search_mode(
    state: &mut AppState,
    terminal: crate::chrome::terminal::TerminalId,
    ctx: KeyInputContext<'_>,
) {
    crate::app::tree_keys::deliver_press_to_terminal_tree(
        state,
        terminal,
        ctx.event_combo,
        ctx.key_text,
    );
    state.needs_redraw = true;
}

#[cfg(test)]
mod tests;
