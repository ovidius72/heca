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
use heca_grid_ui::Component as _;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

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
    FOCUS_LAYER, FocusedSurface, LAYER_FLOOR, focused_surface, surface_action,
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

            // **A key acts on the surface in front of you** (F003/P082/T428). The focused surface
            // resolves it first — its own `[[keys.surface]]` declaration, then the floor its kind
            // is guaranteed — and the global map is the fallback for what nobody in front claimed.
            // That is the ordinary nearest-declaration-wins rule, and it is what makes `Escape`
            // mean *close the thing I am in* everywhere: a layer closes itself, a dock hands the
            // keyboard back, the panes let it reach the program running in them.
            //
            // It was the other way round until now, and a global `Escape` bound to `close_overlay`
            // therefore ate the key before a focused dock could see it — closing nothing, because
            // no overlay was up, and stranding the keyboard in the dock.
            let surface = focused_surface(state);
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
            // What the surface's keymaps left over is not thrown away, though: it goes into the
            // tree, to whatever inside the surface holds focus — a terminal docked in the sidebar
            // is typed into the way a field is. The panes never do this (their keys belong to the
            // program in the pane), and a key the tree does not take is still swallowed.
            if surface.delivers_to_tree() {
                crate::app::tree_keys::deliver_press_to_tree(state, ctx.event_combo, ctx.key_text);
                state.mark_full_redraw();
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
        InputMode::Search => {
            handle_search_mode(state, ctx);
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

/// Scrollback-search query entry (`InputMode::Search`). Mirrors rename-style buffer
/// editing: characters/backspace edit the query and re-run the search live; Enter
/// keeps the matches and returns to selection mode (so `n`/`N` navigate there); Esc
/// cancels the search. terminal-task-19.
fn handle_search_mode(state: &mut AppState, ctx: KeyInputContext<'_>) {
    let is_escape = matches!(ctx.logical_key, Key::Named(NamedKey::Escape));
    let is_enter = matches!(ctx.logical_key, Key::Named(NamedKey::Enter));

    if is_escape {
        // Cancels only this pane's search; other panes keep theirs.
        if let Some(pane) = state.search_target_pane() {
            state.clear_search(pane);
        }
        state.input_mode = InputMode::Selection;
        state.needs_redraw = true;
        return;
    }
    if is_enter {
        // Keep the matches for n/N; just leave query-entry. The field is no longer
        // taking keys, so it must not keep showing a caret as though it were.
        if let Some(search) = state.active_search_mut() {
            search.input.borrow().base().blur();
        }
        state.input_mode = InputMode::Selection;
        state.needs_redraw = true;
        return;
    }

    // Everything else is *text editing*, so it goes to the `Input` through the same
    // host-owned widget keymap every other field uses (`widget-keys-config`). That is
    // what makes `Ctrl+u`, `Ctrl+w`, select-all and caret motion behave here exactly
    // as they do in a dialog or the command palette — this handler used to parse
    // Backspace and single characters itself and silently ignored the rest.
    let Some((combo_key, mods)) = crate::app::registry::combo_to_grid(ctx.event_combo) else {
        return;
    };
    // Typed text goes in as text, before the chord is resolved: `Event::TextInput` carries what
    // the platform says the key produced, so case and shifted symbols survive without the host
    // patching the key it sends. Mirrors the overlay path.
    let mut edited = false;
    if let Some(text) = heca_grid_ui::typed_text(Some(ctx.key_text), mods)
        && let Some(search) = state.active_search_mut()
    {
        let ev = heca_grid_ui::Event::TextInput(text);
        let handled = heca_grid_ui::dispatch(&mut *search.input.borrow_mut(), &ev);
        if handled == heca_grid_ui::Handled::Yes {
            crate::app::terminal_host::run_scrollback_search(state);
            state.needs_redraw = true;
            return;
        }
    }
    let key = combo_key;
    let keymap = state.widget_keymap.clone();
    keymap.dispatch(key, mods, |ev| match state.active_search_mut() {
        Some(search) => {
            let handled = heca_grid_ui::dispatch(&mut *search.input.borrow_mut(), ev);
            edited |= handled == heca_grid_ui::Handled::Yes;
            handled
        }
        None => heca_grid_ui::Handled::No,
    });
    if edited {
        crate::app::terminal_host::run_scrollback_search(state);
    }
    state.needs_redraw = true;
}

#[cfg(test)]
mod tests;
