//! Keyboard input-mode handlers.
//!
//! This module keeps the large input-mode dispatch out of `main.rs` while
//! preserving the existing key handling behavior.

use crate::actions::ActionRegistry;
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
use heca_grid_ui::Component as _;
use heca_grid_ui::reactive::SignalUpdate as _;
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

/// The keymap consulted while a **layer** holds the keyboard — the exposé, a modal, a plugin's
/// surface (F003/P082/T428).
///
/// The layer twin of [`FOCUS_LAYER`], and it exists for the same reason: what every layer answers
/// alike does not belong in each layer's own declaration. Today that is `Escape` — the front-most
/// layer closes itself — asserted as a floor in `registry::build_mode_keymaps` so a layer that
/// declares nothing is still closable from the keyboard.
pub(crate) const LAYER_FLOOR: &str = "layer";

/// The surface name of the **scrolling area** — the panes, and what is in front when no layer and
/// no dock hold the keyboard (F003/P082/T428).
///
/// It is a surface like any other, so it can be spoken about in `[[keys.surface]]` the way a dock
/// and an overlay already are. It ships with **no bindings of its own**, and that absence is
/// deliberate: a key nothing here claims reaches the program running in the pane, which is why
/// `Escape` gets to mean what it means inside vim.
pub(crate) const PANES_SURFACE: &str = "heca.panes";

/// Which surface holds the keyboard — the one question the key-resolution rule is asked
/// (F003/P082/T428).
///
/// Reduced from [`AppState`] by [`focused_surface`] so the rule itself is a pure function of plain
/// data and can be tested without a window. Ordered from the front backwards: a layer covers a
/// dock, a dock covers the panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FocusedSurface {
    /// A layer is in front. `name` is its addressable, owner-prefixed name — the one `show_layer`
    /// takes — and is `None` for a layer nothing named.
    Layer { name: Option<String> },
    /// A chrome container holds the keyboard: its mount id, and the `kind()` of the provider
    /// mounted there.
    Dock { mount: String, kind: Option<String> },
    /// Nothing is in front of the panes.
    Panes,
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

            let global_action = keymap.resolve("global", ctx.event_combo).cloned();
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

/// The action a key resolves to in **one named surface's** binding layer — a dock's `kind()`, a
/// placement id, or a layer's own name (F003/P082/T416).
///
/// Split out of [`focus_layer_action`] so a layer and a dock consult the same map by the same rule.
/// Whichever surface holds the keyboard, its `[[keys.surface]]` entry is the more specific thing
/// the key is aimed at, and there is exactly one lookup for both.
fn surface_layer_action(
    component_keymaps: &HashMap<String, KeymapRegistry>,
    surface: &str,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    component_keymaps
        .get(surface)
        .and_then(|map| map.resolve(surface, combo))
        .cloned()
}

/// The action a key resolves to in one **floor** — the keys a whole kind of surface answers alike
/// (`focus` for docks, `layer` for layers), which no individual surface should have to declare.
fn floor_action(
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    floor: &str,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    mode_keymaps
        .get(floor)
        .and_then(|map| map.resolve(floor, combo))
        .cloned()
}

/// Which surface holds the keyboard, read off the session — the whole of [`AppState`] this rule
/// needs, so [`surface_action`] can stay a pure function of plain data.
pub(crate) fn focused_surface(state: &AppState) -> FocusedSurface {
    if let Some(id) = state.layers.top_modal_id(&state.window_root) {
        return FocusedSurface::Layer {
            name: state.layers.name_of(id),
        };
    }
    match state.chrome_state.focused_container() {
        Some(mount) => {
            let kind = state
                .chrome_host
                .provider(&mount)
                .map(|p| p.kind().to_string());
            FocusedSurface::Dock { mount, kind }
        }
        None => FocusedSurface::Panes,
    }
}

/// The action a key resolves to **in the surface that holds the keyboard** (F003/P082/T428).
///
/// One order, whatever kind of surface it is: the surface's **own** `[[keys.surface]]` declaration
/// first — a dock's rows are the more specific thing a key is aimed at than anything host-wide —
/// then the **floor** its kind is guaranteed. `None` means nobody in front claimed the key, and the
/// caller falls back to the global map.
///
/// A dock is consulted at two names, placement before component: an entry that named an `id` was
/// built into a layer under that mount id, already carrying the id-less base merged underneath it,
/// so finding the mount means the user narrowed this seating and missing it means they spoke about
/// the component as a whole. Two lookups, no merging at press time (F003/P086/T362).
///
/// Both key routes come through here, deliberately: the direct one (an unprefixed key while a dock
/// is focused) and the prefix fall-through (the global `normal` map missed). One declaration, both
/// doors — so a container's `r` works whether the user typed `r` or `prefix+r`.
pub(crate) fn surface_action(
    surface: &FocusedSurface,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    component_keymaps: &HashMap<String, KeymapRegistry>,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    let (names, floor): (Vec<&str>, Option<&str>) = match surface {
        FocusedSurface::Layer { name } => {
            (name.as_deref().into_iter().collect(), Some(LAYER_FLOOR))
        }
        FocusedSurface::Dock { mount, kind } => (
            [Some(mount.as_str()), kind.as_deref()]
                .into_iter()
                .flatten()
                .collect(),
            Some(FOCUS_LAYER),
        ),
        // The panes declare nothing by default, and have no floor: a key nothing claims belongs to
        // the program running in the pane.
        FocusedSurface::Panes => (vec![PANES_SURFACE], None),
    };
    names
        .into_iter()
        .find_map(|name| surface_layer_action(component_keymaps, name, combo))
        .or_else(|| floor.and_then(|floor| floor_action(mode_keymaps, floor, combo)))
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

    // The prefix (`normal`) map first, then — when it misses — the focused surface's own layer,
    // before the key is dropped. That fall-through is what lets a component bind `r` without having
    // to know whether the user reaches it directly or through the prefix (user decision,
    // 2026-07-29).
    let action = keymap.resolve("normal", &combo).cloned().or_else(|| {
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
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
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
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
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

/// Universal picker (`InputMode::HintPick`): a matching letter runs what the picked region said a
/// pick does to it; any other key / Esc just exits. Mirrors [`handle_follow_link_mode`].
///
/// **The host resolves nothing.** There is no registry to look an id up in and no intent to route
/// here: the region declared the behaviour itself (`KeyHint::on_hint`, or a described node's `hint`
/// event), and running it emits whatever that region emits — which is how a plugin's row gets the
/// same picker the app's own rows have. A target whose tree was rebuilt under the letters simply
/// answers `false`.
fn handle_hint_pick_mode(
    _registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, crate::chrome::HintTarget)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    // The letters come down however this ends — picked, wrong key, or Esc. Withdrawn before the
    // pick runs, because running it may tear the tree down and a keycap must not outlive the mode
    // that put it up.
    crate::chrome::clear_hint_letters(state);
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        crate::chrome::fire_hint(state, target);
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

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
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

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
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
    candidates: &[(char, usize, heca_core::layout::WorkspaceId)],
    target: WorkspacePickTarget,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target_ws, _)) = candidates.iter().find(|(c, ..)| *c == ch)
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
    candidates: &[(char, crate::app_state::ColumnPickTarget)],
    pane_id: PaneId,
    ctx: KeyInputContext<'_>,
) {
    use crate::app_state::ColumnPickTarget;
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        // Each destination names the act that reaches it, so the letter runs the same action a
        // keybinding or an RPC call would — the pick is only how a keyboard supplies an argument it
        // cannot type.
        let action = match *target {
            ColumnPickTarget::Existing {
                ws_idx, col_idx, ..
            } => WmAction::MovePaneToColumn {
                pane_id,
                ws_idx,
                col_idx,
            },
            ColumnPickTarget::New => WmAction::MovePaneToNewColumn,
        };
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
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

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::conflicts::Conflicts;
    use crate::app::registry::{build_component_keymaps, build_modes};
    use crate::keymap::BindingIndex;

    /// The shipped defaults, built the way the app builds them — no config of a test's own, so what
    /// these assert is what a user gets.
    fn defaults() -> (
        HashMap<String, KeymapRegistry>,
        HashMap<String, KeymapRegistry>,
    ) {
        let config = heca_config::theme::Config::default();
        let (modes, _) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        let (components, _) =
            build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
        (modes, components)
    }

    fn escape(surface: &FocusedSurface) -> Option<crate::keymap::ActionRef> {
        let (modes, components) = defaults();
        surface_action(surface, &modes, &components, &KeyCombo::parse("Escape"))
    }

    fn dock(mount: &str, kind: &str) -> FocusedSurface {
        FocusedSurface::Dock {
            mount: mount.to_string(),
            kind: Some(kind.to_string()),
        }
    }

    /// **THE REGRESSION** (F003/P082/T428). Antonio, driving 2026-08-13: entering a sidebar pane
    /// list with `prefix+/` or `prefix+e`, *"I'm not able to exit without selecting anything. I
    /// should be able to do Esc and go back to normal in the scrolling area."*
    ///
    /// The keymap and the floor were both correct the whole time; a **global** `Escape` bound to
    /// `close_overlay` resolved first and consumed the key, closing nothing because no overlay was
    /// up. This asserts the rule that replaced it, at the level the bug lived: with a dock holding
    /// the keyboard and no layer in front, `Escape` reaches `unfocus_dock`.
    #[test]
    fn escape_releases_a_focused_dock_back_to_the_panes() {
        assert_eq!(
            escape(&dock("workspaces", "workspaces")),
            Some(crate::keymap::ActionRef::Builtin(WmAction::UnfocusDock)),
        );
    }

    /// A layer in front closes itself — the same press, one surface further forward.
    #[test]
    fn escape_closes_the_layer_in_front() {
        let surface = FocusedSurface::Layer {
            name: Some("heca.expose".to_string()),
        };
        assert_eq!(
            escape(&surface),
            Some(crate::keymap::ActionRef::Builtin(WmAction::CloseOverlay {
                overlay: None
            })),
        );
    }

    /// **And in the scrolling area it is nobody's.** `None` here is what sends the key on to the
    /// global map and then to the pane, so `Escape` still means what it means inside vim. This is
    /// the assertion that fails if anyone gives the panes a floor "for symmetry".
    #[test]
    fn escape_in_the_scrolling_area_belongs_to_the_pane() {
        assert_eq!(escape(&FocusedSurface::Panes), None);
    }

    /// Nearest declaration wins: what a surface declares for itself beats the floor its kind gets,
    /// and beats the global map by resolving here at all. The exposé's `x` is the live case.
    #[test]
    fn a_surfaces_own_keys_come_before_its_floor() {
        let (modes, components) = defaults();
        let surface = FocusedSurface::Layer {
            name: Some("heca.expose".to_string()),
        };
        assert!(
            surface_action(&surface, &modes, &components, &KeyCombo::parse("x")).is_some(),
            "the map's own delete key resolves in its own layer",
        );
        // A key neither the map nor the `layer` floor claims. (Not `q` — that is the floor's, and
        // key matching is case-insensitive on the name, so `Q` is the same key.)
        assert_eq!(
            surface_action(&surface, &modes, &components, &KeyCombo::parse("w")),
            None,
            "and a key it does not claim falls through to the global map",
        );
    }

    /// **A way out of an overlay is declared once, for every layer** — in the `layer` floor, not in
    /// each surface's own entry and not in the global map.
    ///
    /// The global map is the fallback for what nothing in front claimed, so a key there is taken
    /// from the program in the pane whether or not an overlay is up. `close_overlay` shipped as a
    /// global `q`, and `:q` in vim stopped at the colon — in every terminal, always (Antonio,
    /// driving, 2026-08-20). Declared here it exists only while a layer holds the keyboard, which
    /// is the same thing tmux's key tables do.
    #[test]
    fn closing_an_overlay_is_a_layer_key_never_a_global_one() {
        let (modes, components) = defaults();
        let layer = FocusedSurface::Layer {
            name: Some("heca.expose".to_string()),
        };
        for key in ["q", "Ctrl+q"] {
            assert_eq!(
                surface_action(&layer, &modes, &components, &KeyCombo::parse(key)),
                Some(crate::keymap::ActionRef::Builtin(WmAction::CloseOverlay {
                    overlay: None
                })),
                "{key} closes the overlay in front of you",
            );
            // …and in the scrolling area nobody claims it, which is what sends it to the program.
            assert_eq!(
                surface_action(
                    &FocusedSurface::Panes,
                    &modes,
                    &components,
                    &KeyCombo::parse(key)
                ),
                None,
                "{key} belongs to the pane when no overlay is up",
            );
        }
    }

    /// A dock is consulted at two names, **placement before component**, so narrowing one seating
    /// of a provider does not have to restate the rest (F003/P086/T362).
    #[test]
    fn a_dock_is_consulted_at_its_placement_then_its_kind() {
        let (modes, components) = defaults();
        let combo = KeyCombo::parse("x");
        let by_kind = surface_action(
            &dock("nowhere-in-particular", "workspaces"),
            &modes,
            &components,
            &combo,
        );
        assert!(
            by_kind.is_some(),
            "an unknown mount still resolves through the provider's kind",
        );
    }
}
