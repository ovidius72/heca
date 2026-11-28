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
        // **The second prefix is a literal `Ctrl+B` for whatever holds the keyboard.** While the
        // window holds it, it is given there like any key: a terminal placed anywhere types it, and
        // anything that does not take keys ignores it. Otherwise it is typed into the focused pane.
        if crate::app::tree_keys::window_holds_keyboard(state) {
            let combo = state.prefix.combo.clone();
            crate::app::tree_keys::deliver_press_to_tree(state, keymap, &combo, "");
        } else if let Some(pane_id) = state.focused_pane
            && let Some(backend) = state.client.backends.get_mut(pane_id)
        {
            let literal = prefix_combo_to_literal_input(&state.prefix.combo);
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
    // **A key that begins a longer sequence waits for the next one** (neovim's `timeoutlen`, here
    // `prefix_timeout_ms`): what it does on its own — a mode it opens, an action — is kept for when
    // the next key does not continue the sequence, or the wait runs out.
    if keymap
        .step(crate::keymap::LEADER_LAYER, std::slice::from_ref(&combo))
        .continues
    {
        let then_alone = match &entered_mode {
            Some((name, _)) => Some(crate::app_state::SequenceAlone::Mode(name.clone())),
            None => keymap
                .resolve(crate::keymap::LEADER_LAYER, &combo)
                .cloned()
                .map(crate::app_state::SequenceAlone::Action),
        };
        state.input_mode = InputMode::Sequence {
            keys: vec![combo],
            then_alone,
        };
        state.prefix_entered_at = Some(std::time::Instant::now());
        state.needs_redraw = true;
        return;
    }
    if let Some((mode_name, _sticky)) = entered_mode {
        state.input_mode = InputMode::Mode { name: mode_name };
        state.prefix_entered_at = None;
        state.needs_redraw = true;
        return;
    }

    // The prefix (`normal`) map first, then — when it misses — the focused surface's own layer,
    // before the key is dropped. That fall-through is what lets a component bind `r` without having
    // to know whether the user reaches it directly or through the prefix.
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
        let _ = dispatch_action_ref(state, registry, InteractionSource::Keyboard, act);
    } else if !ctx.key_text.is_empty() {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
    }
}

/// **The next key of a sequence** (see [`InputMode::Sequence`]). Returns true when the key did not
/// continue it: what the keys so far do alone has run, and the caller handles this key afresh.
pub(super) fn handle_sequence_mode(
    registry: &ActionRegistry,
    keymap: &KeymapRegistry,
    state: &mut AppState,
    keys: &[KeyCombo],
    then_alone: Option<crate::app_state::SequenceAlone>,
    ctx: KeyInputContext<'_>,
) -> bool {
    if matches!(ctx.logical_key, Key::Named(NamedKey::Escape)) {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        state.needs_redraw = true;
        return false;
    }
    if crate::app::keyboard::is_modifier_key(ctx.logical_key) {
        return false;
    }
    match next_in_sequence(keymap, keys, then_alone, ctx.event_combo) {
        SequenceNext::Wait { keys, then_alone } => {
            state.input_mode = InputMode::Sequence { keys, then_alone };
            state.prefix_entered_at = Some(std::time::Instant::now());
            state.needs_redraw = true;
            false
        }
        SequenceNext::Run(action) => {
            state.input_mode = InputMode::Normal;
            state.prefix_entered_at = None;
            state.needs_redraw = true;
            let _ = dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
            false
        }
        SequenceNext::Alone(then_alone) => {
            state.input_mode = InputMode::Normal;
            state.prefix_entered_at = None;
            state.needs_redraw = true;
            run_alone(state, registry, then_alone);
            true
        }
    }
}

/// What the next key does to a sequence on its way.
#[derive(Debug, PartialEq)]
pub(crate) enum SequenceNext {
    /// It continues a longer sequence: keep waiting, with what these keys now do alone.
    Wait {
        keys: Vec<KeyCombo>,
        then_alone: Option<crate::app_state::SequenceAlone>,
    },
    /// It completes one: run it.
    Run(crate::keymap::ActionRef),
    /// It continues nothing: do what the keys before it do alone, then handle it afresh.
    Alone(Option<crate::app_state::SequenceAlone>),
}

/// **The rule for the next key of a sequence** — neovim's: a key that continues a longer
/// sequence waits; one that completes a sequence and continues none runs it; anything else falls
/// back to what the keys so far do alone.
pub(crate) fn next_in_sequence(
    keymap: &KeymapRegistry,
    keys: &[KeyCombo],
    then_alone: Option<crate::app_state::SequenceAlone>,
    next: &KeyCombo,
) -> SequenceNext {
    let mut pressed = keys.to_vec();
    pressed.push(next.clone());
    let step = keymap.step(crate::keymap::LEADER_LAYER, &pressed);
    if step.continues {
        return SequenceNext::Wait {
            keys: pressed,
            then_alone: step
                .complete
                .map(crate::app_state::SequenceAlone::Action)
                .or(then_alone),
        };
    }
    match step.complete {
        Some(action) => SequenceNext::Run(action),
        None => SequenceNext::Alone(then_alone),
    }
}

/// [`run_alone`] for the wait running out — the loop's turn has no registry, so the action goes
/// out through the window's own dispatch door, judged as a key.
pub(crate) fn alone_after_waiting(
    state: &mut AppState,
    then_alone: Option<crate::app_state::SequenceAlone>,
) {
    use crate::app::interaction::InteractionIntent;
    let intent = match then_alone {
        Some(crate::app_state::SequenceAlone::Action(crate::keymap::ActionRef::Builtin(
            action,
        ))) => InteractionIntent::ActivateAction(action),
        Some(crate::app_state::SequenceAlone::Action(crate::keymap::ActionRef::Dynamic(
            intent,
        ))) => InteractionIntent::View(intent),
        Some(crate::app_state::SequenceAlone::Mode(name)) => {
            state.input_mode = InputMode::Mode { name };
            return;
        }
        None => return,
    };
    crate::chrome::ChromeIntentEmitter::new(&state.event_proxy, InteractionSource::Keyboard)
        .fire(intent);
}

/// **What the keys of an unfinished sequence do on their own** — the action they are bound to, or
/// the mode they open. Where the next key that does not continue the sequence ends up.
pub(crate) fn run_alone(
    state: &mut AppState,
    registry: &ActionRegistry,
    then_alone: Option<crate::app_state::SequenceAlone>,
) {
    match then_alone {
        Some(crate::app_state::SequenceAlone::Action(action)) => {
            let _ = dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
        }
        Some(crate::app_state::SequenceAlone::Mode(name)) => {
            state.input_mode = InputMode::Mode { name };
        }
        None => {}
    }
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
            let _ = dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
            if !stay {
                state.input_mode = InputMode::Normal;
                state.needs_redraw = true;
            }
        }
        ModeKey::Normal(action) => {
            let _ = dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
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
        let _ = dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::ExitScrollback,
        );
    }
}

#[cfg(test)]
mod sequence_tests {
    use super::*;
    use crate::app_state::SequenceAlone;
    use crate::keymap::{ActionRef, LEADER_LAYER};

    fn keymap() -> KeymapRegistry {
        let mut keymap = KeymapRegistry::new();
        let k = KeyCombo::parse;
        keymap.bind(
            LEADER_LAYER,
            k("s"),
            ActionRef::Builtin(WmAction::EnterSelectionMode),
        );
        keymap.bind_sequence(
            LEADER_LAYER,
            vec![k("s"), k("r")],
            ActionRef::Builtin(WmAction::RenameSession { session: None }),
        );
        keymap.bind_sequence(
            LEADER_LAYER,
            vec![k("s"), k("d"), k("x")],
            ActionRef::Builtin(WmAction::DeleteSession { session: None }),
        );
        keymap.bind_sequence(
            LEADER_LAYER,
            vec![k("s"), k("d")],
            ActionRef::Builtin(WmAction::DetachSession),
        );
        keymap
    }

    /// **neovim's rule.** `s` alone opens the selection and begins `s r`; after `s`: `r` completes
    /// the sequence and runs it; `d` completes `s d` but begins `s d x`, so it waits with `s d` as
    /// the fallback; a key that continues nothing falls back to what `s` does alone.
    #[test]
    fn the_next_key_waits_runs_or_falls_back() {
        let keymap = keymap();
        let k = KeyCombo::parse;
        let alone = Some(SequenceAlone::Action(ActionRef::Builtin(
            WmAction::EnterSelectionMode,
        )));
        assert_eq!(
            next_in_sequence(&keymap, &[k("s")], alone.clone(), &k("r")),
            SequenceNext::Run(ActionRef::Builtin(WmAction::RenameSession {
                session: None
            }))
        );
        assert_eq!(
            next_in_sequence(&keymap, &[k("s")], alone.clone(), &k("d")),
            SequenceNext::Wait {
                keys: vec![k("s"), k("d")],
                then_alone: Some(SequenceAlone::Action(ActionRef::Builtin(
                    WmAction::DetachSession
                ))),
            },
            "s d is complete but s d x is longer: wait, with s d as what runs alone"
        );
        assert_eq!(
            next_in_sequence(&keymap, &[k("s")], alone.clone(), &k("q")),
            SequenceNext::Alone(alone),
            "a key that continues nothing: what s does alone"
        );
    }
}
