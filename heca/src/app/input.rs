//! Keyboard input-mode handlers.
//!
//! This module keeps the large input-mode dispatch out of `main.rs` while
//! preserving the existing key handling behavior.

use crate::actions::ActionRegistry;
use heca_grid_ui::Component as _;
use heca_grid_ui::reactive::SignalUpdate as _;
use crate::app::interaction::InteractionSource;
use crate::app::interaction::{dispatch_action, dispatch_action_ref};
use crate::app::keyboard::{
    event_combo_matches, normalize_key_text, prefix_combo_to_literal_input, typed_candidate_char,
    winit_key_to_backend_event, winit_key_to_terminal_input,
};
use crate::app::selection::find_pane_location;
use crate::app_state::{AppState, InputMode, WorkspacePickTarget};
use crate::input::WmAction;
use crate::keymap::{KeyCombo, KeymapRegistry, Keymaps};
use heca_core::layout::PaneId;
use std::collections::HashMap;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

/// The keymap consulted while a chrome container holds keyboard focus (F003/P085/T352).
///
/// It is a mode **keymap**, not an [`InputMode`]: chrome focus already answers "where do the keys
/// go", and a mode beside it would be a second fact that can disagree with the first. Like the
/// `sidebar` map it has no trigger — you enter it by focusing a dock, not by pressing something.
///
/// What lives here is what the **widgets** answer — paging and edges for whatever scroll area the
/// focused container nests — because a scroll region behaves identically wherever it is mounted and
/// no component should have to declare that. What a *component* declares is a separate, per-kind
/// layer (`[keys.<kind>]`, F003/P085/T355) resolved through this same seam.
pub(crate) const FOCUS_LAYER: &str = "focus";

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
    let input_mode = state.input_mode.clone();
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

            let global_action = keymap.resolve("global", ctx.event_combo).cloned();
            if let Some(act) = global_action {
                dispatch_action_ref(state, registry, InteractionSource::Keyboard, &act);
                return;
            }

            // **Focus is the mode.** A dock holding chrome focus redirects the keyboard: the key
            // resolves in its layer, and an unbound one is **swallowed** rather than forwarded. A
            // `j` leaking into a shell while the user is driving a sidebar is the worse failure —
            // and the focus ring plus the status bar are what stop the swallowing being silent.
            //
            // `state.focused_pane` is deliberately untouched: only the keyboard is redirected, so
            // `prefix+Enter` still splits the pane you last worked in.
            if state.chrome_state.focused_container().is_some() {
                if let Some(act) =
                    focus_layer_action(state, mode_keymaps, component_keymaps, ctx.event_combo)
                {
                    dispatch_action_ref(state, registry, InteractionSource::Keyboard, &act);
                }
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
            handle_custom_mode(registry, mode_keymaps, mode_triggers, state, &name, ctx);
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
            handle_selection_mode(registry, mode_keymaps, state, ctx);
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
            search.input.borrow_mut().base_mut().focused.set(false);
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
    // Deliver the real character for plain typing so case and shifted symbols survive;
    // the combo key stays lowercased for chord matching only. Mirrors the overlay path.
    let key = {
        let mut cs = ctx.key_text.chars();
        match (cs.next(), cs.next()) {
            (Some(c), None) if !mods.ctrl && !mods.meta && !c.is_control() && c != ' ' => {
                heca_grid_ui::GridKey::Char(c)
            }
            _ => combo_key,
        }
    };
    let keymap = state.widget_keymap.clone();
    let mut edited = false;
    keymap.dispatch(key, mods, |ev| {
        match state.active_search_mut() {
            Some(search) => {
                let handled = heca_grid_ui::dispatch(&mut *search.input.borrow_mut(), ev);
                edited |= handled == heca_grid_ui::Handled::Yes;
                handled
            }
            None => heca_grid_ui::Handled::No,
        }
    });
    if edited {
        crate::app::terminal_host::run_scrollback_search(state);
    }
    state.needs_redraw = true;
}


/// The action a key resolves to in the **focused container's layer** — `None` when no chrome
/// container holds the keyboard, which is what makes this seam inert in the ordinary case.
///
/// Both key routes come through here, deliberately: the direct one (an unprefixed key while a dock
/// is focused) and the prefix fall-through (the global `normal` map missed). One declaration, both
/// doors — so a container's `r` works whether the user typed `r` or `prefix+r`.
fn focus_layer_action(
    state: &AppState,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    component_keymaps: &HashMap<String, KeymapRegistry>,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    let mount = state.chrome_state.focused_container()?;
    // **The component's own layer first** (`[[keys.component]]`, F003/P086/T362), so a component can
    // bind a key the host layer also uses and win — its rows are the more specific thing the key is
    // aimed at.
    //
    // **This placement, then the component.** An entry that named an `id` was built into a layer
    // under that mount id, already carrying the id-less base merged underneath it; so finding the
    // mount means the user narrowed this seating, and missing it means they spoke about the
    // component as a whole. Two lookups, no merging at press time.
    let kind = state.chrome_host.provider(&mount).map(|p| p.kind());
    for layer in [Some(mount.as_str()), kind] {
        if let Some(layer) = layer
            && let Some(action) = component_keymaps
                .get(layer)
                .and_then(|map| map.resolve(layer, combo))
        {
            return Some(action.clone());
        }
    }
    // Then what the **widgets** answer for every container alike — paging, edges, releasing focus.
    mode_keymaps
        .get(FOCUS_LAYER)
        .and_then(|map| map.resolve(FOCUS_LAYER, combo))
        .cloned()
}

fn handle_prefix_mode(
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
        if state.chrome_state.focused_container().is_some() {
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

    // The prefix (`normal`) map first, then — when it misses — the focused container's layer,
    // before the key is dropped. That fall-through is what lets a component bind `r` without having
    // to know whether the user reaches it directly or through the prefix (user decision,
    // 2026-07-29).
    let action = keymap
        .resolve("normal", &combo)
        .cloned()
        .or_else(|| focus_layer_action(state, mode_keymaps, component_keymaps, &combo));
    if let Some(ref act) = action {
        state.input_mode = InputMode::Normal;
        state.prefix_entered_at = None;
        dispatch_action_ref(state, registry, InteractionSource::Keyboard, act);
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
        dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
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

fn handle_follow_link_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[crate::app_state::LinkHint],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    // Any key exits the overlay; a matching letter opens its link. Esc just exits.
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some(hint) = candidates.iter().find(|h| h.label == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::OpenLink {
                url: hint.url.clone(),
            },
        );
    }
    state.needs_redraw = true;
}

/// Universal hint picker (`InputMode::HintPick`): a matching letter fires that
/// target's intent (resolved from the retained tree's hint-target registry and routed
/// through the interaction policy layer, exactly like a mouse click); any other key /
/// Esc just exits. Mirrors [`handle_follow_link_mode`].
fn handle_hint_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, heca_grid_ui::HintTargetId)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        // Resolve the picked target's intent from the shared registry (spans the chrome
        // + pane-header trees), then dispatch it (owned clone drops the borrow before the
        // mutable dispatch call).
        let intent = state.hint_targets.get(*id).cloned();
        if let Some(intent) = intent {
            crate::app::interaction::dispatch_intent(
                state,
                registry,
                InteractionSource::Keyboard,
                intent,
            );
        }
    }
    state.needs_redraw = true;
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

/// Resolve a [`InputMode::DockPick`] keypress: a matching candidate letter gives that **dock**
/// chrome keyboard focus; any other key (e.g. Esc) exits the mode.
///
/// It goes back out through the same `focus_dock` action, carrying the picked id — so the letter, an
/// RPC call and a script all take one path, and the pick is only how a keyboard supplies an argument
/// it cannot type (F003/P011/T020).
fn handle_dock_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, crate::chrome::ContainerId)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key);
    if let Some(ch) = typed
        && let Some((_, dock)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::FocusDock {
                dock: Some(dock.clone()),
            },
        );
    }
    state.needs_redraw = true;
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
        // ExitScrollback snaps the viewport to bottom, clears selection,
        // and exits Selection mode.
        state.input_mode = InputMode::Normal;
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::ExitScrollback,
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
            dispatch_action_ref(state, registry, InteractionSource::Keyboard, &action);
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

