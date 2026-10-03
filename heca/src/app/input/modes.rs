//! The modes a key can be in beyond the plain one: prefix, chord, and the named (sticky) modes.

use super::{KeyInputContext, focused_surface, surface_action};
use crate::actions::ActionRegistry;
use crate::app::interaction::InteractionSource;
use crate::app::interaction::{dispatch_action, dispatch_action_ref};
use crate::app::keyboard::{event_combo_matches, prefix_combo_to_literal_input};
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;
use crate::keymap::{KeyCombo, KeymapRegistry};
use std::collections::HashMap;
use winit::keyboard::{Key, NamedKey};

pub(super) fn handle_prefix_mode(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    component_keymaps: &HashMap<String, KeymapRegistry>,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    ctx: KeyInputContext<'_>,
) {
    if ctx.is_prefix {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        // The double-prefix literal passthrough is for a pane that is *taking text*. While a dock
        // holds the keyboard nothing is, so sending a literal `Ctrl+B` to a pane the user is not
        // typing in is a surprise rather than a passthrough (F003/P085/T352).
        if crate::app::tree_focus::focused_dock(state).is_some() {
            return;
        }
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

    // The key exactly as pressed — the same combo the normal bindings see, so a prefix binding with
    // Cmd or Alt (`prefix+Alt+n`) is reachable, and a held Cmd never turns into the bare key.
    let combo = ctx.event_combo.clone();
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

    // The prefix (`normal`) map first, then — when it misses — the focused surface's own layer,
    // before the key is dropped. That fall-through is what lets a component bind `r` without having
    // to know whether the user reaches it directly or through the prefix (user decision,
    // 2026-07-29).
    let action = keymap
        .resolve(crate::keymap::LEADER_LAYER, &combo)
        .cloned()
        .or_else(|| {
            surface_action(
                &focused_surface(state),
                mode_keymaps,
                component_keymaps,
                &combo,
            )
        });
    if let Some(ref act) = action {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        dispatch_action_ref(state, registry, InteractionSource::Keyboard, act);
    } else if !ctx.key_text.is_empty() {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
    }
}

pub(super) fn handle_chord_mode(
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

/// The keymap name selection mode's own keys are declared under (`[[keys.mode]] name = "selection"`).
pub(super) const SELECTION_MODE: &str = "selection";

/// One key press, as [`mode_key`] needs to see it.
pub(crate) struct ModeKeyPress<'a> {
    pub(crate) escape: bool,
    pub(crate) enter: bool,
    pub(crate) prefix: bool,
    /// The key exactly as pressed — one combo for the mode's own keys and the normal bindings, so
    /// Cmd+V is Cmd+V in both.
    pub(crate) combo: &'a KeyCombo,
}

/// What a key does while a mode holds the keyboard.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModeKey {
    /// Esc: leave the mode, dropping what it was doing.
    Cancel,
    /// Enter: leave the mode, keeping what it made.
    Confirm,
    /// The prefix key: leave the mode for the prefix.
    Prefix,
    /// One of the mode's own keys; `stay` is false for a one-shot (non-sticky) mode.
    Own {
        action: crate::keymap::ActionRef,
        stay: bool,
    },
    /// A key the mode does not bind but the normal bindings do. It **bubbles** to them and the mode
    /// stays — which is what lets Cmd+V paste while selecting.
    Normal(crate::keymap::ActionRef),
    /// Nobody binds it. Swallowed: a mode never lets a key through to the program behind it, so a
    /// stray `j` does not type into the shell.
    Swallow,
}

/// **What a key does in a mode** — one rule for every mode, selection included: Esc cancels, Enter
/// confirms, the prefix key goes to the prefix, then the mode's own keys, then the normal bindings
/// (AGENTS § 0c — keys bubble). Pure, like [`surface_action`], so it is testable without a window.
pub(crate) fn mode_key(
    mode: &str,
    sticky: bool,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    keymap: &KeymapRegistry,
    key: &ModeKeyPress<'_>,
) -> ModeKey {
    if key.escape {
        return ModeKey::Cancel;
    }
    if key.enter {
        return ModeKey::Confirm;
    }
    if key.prefix {
        return ModeKey::Prefix;
    }
    if let Some(action) = mode_keymaps
        .get(mode)
        .and_then(|map| map.resolve(mode, key.combo))
    {
        return ModeKey::Own {
            action: action.clone(),
            stay: sticky,
        };
    }
    match keymap.resolve(crate::keymap::DIRECT_LAYER, key.combo) {
        Some(action) => ModeKey::Normal(action.clone()),
        None => ModeKey::Swallow,
    }
}

/// Carry out [`mode_key`]'s answer — the one handler for selection mode and every custom mode.
pub(super) fn handle_mode(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    mode_triggers: &HashMap<String, (KeyCombo, bool)>,
    state: &mut AppState,
    mode: &str,
    ctx: KeyInputContext<'_>,
) {
    let sticky = mode_triggers.get(mode).map(|(_, s)| *s).unwrap_or(true);
    let key = ModeKeyPress {
        escape: matches!(ctx.logical_key, Key::Named(NamedKey::Escape)),
        enter: matches!(ctx.logical_key, Key::Named(NamedKey::Enter)),
        prefix: ctx.is_prefix,
        combo: ctx.event_combo,
    };
    match mode_key(mode, sticky, mode_keymaps, keymap, &key) {
        ModeKey::Cancel => leave_mode(registry, state, false),
        ModeKey::Confirm => leave_mode(registry, state, true),
        ModeKey::Prefix => {
            // Match the Normal→Prefix promotion exactly: set the mode AND arm the timeout.
            state.input_mode = InputMode::Prefix;
            state.prefix_entered_at = Some(std::time::Instant::now());
            state.needs_redraw = true;
        }
        ModeKey::Own { action, stay } => {
            dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
            if !stay {
                state.input_mode = InputMode::Normal;
                state.needs_redraw = true;
            }
        }
        ModeKey::Normal(action) => {
            dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
        }
        ModeKey::Swallow => {}
    }
}

/// Leave the mode in front — `keep` for Enter, dropping what it did for Esc.
///
/// Selection mode is the one that made something: Enter keeps the selection (a mode-internal step,
/// `Selecting → Selected`, not an action of its own), and Esc runs `exit_scrollback`, which snaps
/// the viewport back to the bottom and clears the selection.
pub(super) fn leave_mode(registry: &ActionRegistry, state: &mut AppState, keep: bool) {
    let selecting = matches!(state.input_mode, InputMode::Selection);
    state.input_mode = InputMode::Normal;
    state.needs_redraw = true;
    if !selecting {
        return;
    }
    if keep {
        if state.selection.is_active() {
            state.selection.end();
        }
    } else {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::ExitScrollback,
        );
    }
}
