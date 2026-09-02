//! Window event dispatch helpers.
//!
//! This module keeps per-event routing out of `main.rs` while preserving the
//! existing winit-driven behavior.

use crate::actions::ActionRegistry;
use crate::app::input::{KeyInputContext, handle_keyboard_input};
use crate::app::interaction::{InteractionIntent, InteractionSource, dispatch_action};
use crate::app::keyboard::{build_event_combo, is_prefix_match};
use crate::app::mutations::{MutationKind, after_mutation_change};
use crate::app::render::{render_frame, update_session_viewport};
use crate::app::terminal_host::{
    forward_mouse_button, forward_mouse_move, forward_mouse_wheel, notify_window_focus_changed,
};
use crate::app::terminal_metrics::refresh_terminal_cell_size;
use crate::app_state::AppState;
use crate::input::{FontZoomStep, WmAction};
use crate::keymap::Keymaps;
use crate::mouse;
use heca_core::layout::Point;
use heca_grid_ui::{Event, Handled, PointerButton, RawPointer, RawPointerKind};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
#[derive(Clone, Debug)]
pub enum AppEvent {
    BackendWake,
    RequestRedraw,
    ChromeIntent {
        source: InteractionSource,
        intent: InteractionIntent,
    },
    /// Posted by `Notification::send` through the host-installed sink (`notification::install_notification_sink`,
    /// wired at startup) — the one door a producer or plugin author raises a notification
    /// through. Handled by reading `Instant::now()` here (the host's clock, never the caller's)
    /// and calling the crate-private `NotificationRuntime::push`.
    RaiseNotification {
        draft: crate::notification::NotificationDraft,
    },
}

pub(crate) fn handle_window_event(
    event_loop: &ActiveEventLoop,
    registry: &ActionRegistry,
    keymaps: &Keymaps,
    state: &mut AppState,
    event: WindowEvent,
) {
    match event {
        WindowEvent::CloseRequested => event_loop.exit(),
        WindowEvent::Resized(phys) if phys.width > 0 && phys.height > 0 => {
            state.surface_config.width = phys.width;
            state.surface_config.height = phys.height;
            state
                .surface
                .configure(&state.device, &state.surface_config);
            let log_w = phys.width as f32 / state.scale_factor as f32;
            let log_h = phys.height as f32 / state.scale_factor as f32;
            state.text_renderer.set_target_size(phys.width, phys.height);
            state.grid_renderer.set_target_size(phys.width, phys.height);
            state
                .primitive_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .text_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .grid_renderer
                .set_screen_size(&state.queue, log_w, log_h);
            state
                .compositor
                .resize(&state.device, phys.width, phys.height);
            state.blur.resize(&state.device, phys.width, phys.height);
            state
                .background
                .resize(&state.device, phys.width, phys.height);
            update_session_viewport(state);
            after_mutation_change(state, MutationKind::Config);
        }
        WindowEvent::RedrawRequested => {
            state.needs_redraw = true;
            render_frame(state);
        }
        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            state.scale_factor = scale_factor;
            state.text_renderer.set_scale_factor(scale_factor);
            state.grid_renderer.set_scale_factor(scale_factor);
            state.terminal_layers.clear();
            refresh_terminal_cell_size(state);
            state.mark_full_redraw();
        }
        WindowEvent::Focused(focused) => {
            state.window_focused = focused;
            if focused {
                state.window.request_user_attention(None);
            }
            notify_window_focus_changed(state, focused);
            state.mark_full_redraw();
        }
        WindowEvent::KeyboardInput { event, .. } => {
            // **A release is half a keystroke, and it is delivered.** Returning here meant
            // `Event::Key { pressed: false }` never existed in this app, so
            // `ComponentExt::on_key_up` was a builder nothing could ever fire — the exposé's
            // delete keys did nothing while a headless test that dispatched both halves passed
            // (Antonio, driving, 2026-08-11). The same shape as the right-click that never opened
            // a menu because the funnel delivered only presses.
            //
            // Only the key itself goes down this path — no text, no resolved intents — which is
            // `Keymap::deliver_release`'s whole contract; and none of the keymap machinery below
            // runs, because a release resolves to no action and an in-flight prefix sequence is
            // driven by presses.
            if event.state != ElementState::Pressed {
                if layer_holds_keyboard(state) {
                    let key_text = event.logical_key.to_text().unwrap_or("").to_string();
                    let combo = build_event_combo(
                        &event.logical_key,
                        &event.physical_key,
                        &key_text,
                        state.modifiers,
                    );
                    if let Some((key, _)) = crate::app::registry::combo_to_grid(&combo) {
                        let keymap = state.widget_keymap.clone();
                        let handled = keymap.deliver_release(key, |ev| {
                            heca_grid_ui::dispatch(&mut state.window_root, ev)
                        });
                        if matches!(handled, Handled::Yes) {
                            state.mark_full_redraw();
                        }
                    }
                }
                return;
            }
            state.mark_full_redraw();

            let is_ctrl = state.modifiers.control_key();
            let is_shift = state.modifiers.shift_key();
            let log_key = &event.logical_key;
            let key_text = log_key.to_text().unwrap_or("").to_string();
            let event_combo = build_event_combo(
                &event.logical_key,
                &event.physical_key,
                &key_text,
                state.modifiers,
            );
            let is_prefix = is_prefix_match(&event_combo, &state.prefix_combo, log_key, &key_text);

            // A layer in front owns the keyboard — EXCEPT the universal hint picker, which must
            // still reach its buttons (they are collected as hint targets out of the laid-out
            // tree). So the prefix trigger and any in-flight prefix / hint-pick sequence fall
            // through to the keymap machinery below; every other key goes to the tree, which
            // delivers it to whatever holds focus inside that layer and bubbles it back up (a
            // `Dialog` self-handles focus/activation/dismiss, and tracks its own modifier state
            // from the broadcast `ModifiersChanged`).
            let picker_seq = is_prefix
                || matches!(
                    state.input_mode,
                    crate::app_state::InputMode::Prefix
                        | crate::app_state::InputMode::HintPick { .. }
                );
            if !picker_seq && layer_holds_keyboard(state) {
                // Overlay key resolution via the single host-owned widget keymap
                // (`widget-keys-config`). `Keymap::dispatch` delivers the raw key to the overlay
                // **field-first** (so an `Input`'s typing / caret and a menu's quick-pick letters
                // win), then the semantic `WidgetIntent`(s) the chord resolves to — a `Dialog`
                // takes `Item*`/`Activate`/`Dismiss`, a menu/`Select` takes `Menu*`, a focused
                // field takes `Edit*` (forwarded field-first by the overlay). One dispatch, one map.
                // Use the already-normalized `event_combo` (which carries the macOS
                // physical-key fallback for `Ctrl+letter`, unlike the raw logical key) so vim
                // `Ctrl+h/j/k/l` resolve to the right chord.
                if let Some((combo_key, mods)) = crate::app::registry::combo_to_grid(&event_combo) {
                    // **One call: the surface does not write the order.** Committed text, then the
                    // key, then the intents it resolves to — all inside `deliver_press`, so this
                    // surface and every other one feed a widget identically. Writing the sequence
                    // here is how the showcase came to have no `TextInput` step at all while the
                    // same `CommandPalette` typed fine in this app.
                    let press = heca_grid_ui::KeyPress {
                        key: combo_key,
                        text: Some(key_text.to_string()),
                        mods,
                    };
                    let keymap = state.widget_keymap.clone();
                    let handled = keymap.deliver_press(&press, |ev| {
                        heca_grid_ui::dispatch(&mut state.window_root, ev)
                    });
                    // **A key the overlay ignored is not consumed by the overlay.** Returning
                    // regardless swallowed every binding an open surface had no use for — which is
                    // why `q`, catalogued and bound to `close_overlay` alongside `Escape`, did
                    // nothing while the map was up: `Escape` resolves to the `dismiss` widget
                    // intent and was handled here, `q` resolves to nothing and died here.
                    if matches!(handled, Handled::Yes) {
                        state.mark_full_redraw();
                        return;
                    }
                } else {
                    state.mark_full_redraw();
                    return;
                }
            }

            handle_keyboard_input(
                registry,
                keymaps,
                state,
                KeyInputContext {
                    logical_key: &event.logical_key,
                    physical_key: &event.physical_key,
                    key_text: &key_text,
                    event_combo: &event_combo,
                    is_prefix,
                    is_ctrl,
                    is_shift,
                },
            );
        }
        WindowEvent::ModifiersChanged(new_mods) => {
            state.modifiers = new_mods.state();
            mouse::on_modifiers_changed(state);
            // Announce it to the whole tree so a self-contained widget (a `Dialog`) can do
            // Shift+Tab / Ctrl+h-l itself — its `Event::Key` carries no modifiers.
            //
            // No gate and no target: a modifier change is an announcement, and the framework
            // broadcasts it. Picking the front-most modal out of the registry and delivering only
            // there meant every other surface — a dock, a menu, a plugin's panel — tracked
            // modifiers only when it happened to be the thing in front.
            let mods = grid_modifiers(state.modifiers);
            let _ = heca_grid_ui::dispatch(&mut state.window_root, &Event::ModifiersChanged(mods));
            // Refresh the cursor affordance: pressing/releasing Cmd over a link
            // toggles the pointer cue even without pointer movement.
            mouse::update_cursor(state, state.mouse.pos);
            state.mark_full_redraw();
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = (
                position.x as f32 / state.scale_factor as f32,
                position.y as f32 / state.scale_factor as f32,
            );
            state.mouse.pos = pos;
            // A surface above the page gets the move first — hover on a dialog's buttons and menu
            // rows, and a thumb drag inside its body. It declines a move that misses it (a
            // notification card), and the page's own hover work below then runs as usual.
            if crate::chrome::dispatch_surface_pointer(
                state,
                &raw_pointer(state, RawPointerKind::Moved, PointerButton::Left),
            ) {
                return;
            }
            if let Some(action) = mouse::on_cursor_moved(state, pos) {
                dispatch_action(state, registry, InteractionSource::MouseContent, &action);
            }
            // Feed the move into the retained chrome tree so sidebar hover affordances
            // (MarkerGroup grip "grab" cue, Row hover) light up — the app otherwise
            // only sends presses.
            //
            // ⚠️ **A drag is NOT a reason to withhold this.** The move is what *drives* a drag in
            // flight: the framework reads the cursor position for the picture that follows it, runs
            // `DragEnter`/`DragOver` to mark the target and pick before/after/onto, and re-reads the
            // modifiers that decide move versus swap. Hold the move back and the gesture freezes
            // where it started — the one move that crosses the threshold gets through, and nothing
            // after it does. Hover is no reason either: the framework lights nothing under a drag
            // (guard `a_drag_in_flight_clears_hover`).
            if !mouse::is_resizing(state) {
                crate::chrome::chrome_dispatch_move(state, pos);
            }
            // The pane header and the pane viewport are their OWN retained trees, which a drag in
            // the window root does not reach — so they are still told to stay dark while something
            // is being carried, which is what keeps "nothing hovers under a drag" true for them.
            let mut pane_viewport_over = false;
            if !crate::chrome::drag_in_flight(state) && !mouse::is_resizing(state) {
                // Feed the move into the retained pane-info-bar headers so the action
                // buttons' hover affordance lights up (repaint via mark_full_redraw below).
                crate::chrome::dispatch_pane_header_move(state, pos);
                pane_viewport_over = crate::chrome::dispatch_pane_viewport_move(state, pos);
            }
            // Don't forward moves to the terminal while resizing a divider or while a
            // retained viewport widget (badge / scrollbar) owns the pointer.
            if !mouse::is_resizing(state) && !pane_viewport_over {
                forward_mouse_move(state, pos);
            }
            // Cursor affordance: Grab over a draggable, Grabbing while dragging.
            mouse::update_cursor(state, pos);
            state.mark_full_redraw();
        }
        // The pointer left the window: nothing may stay lit behind it. Without this a hover — or
        // a gesture the release never came back for — survives the cursor going somewhere else
        // entirely, which reads as a UI frozen mid-interaction.
        WindowEvent::CursorLeft { .. } => {
            // One walk is enough: `chrome_dispatch_cancelled` already feeds the whole window tree,
            // and every surface is a node in it. This used to deliver the cancel to the front-most
            // modal first and then again with the tree, which is one event arriving twice.
            let ev = raw_pointer(state, RawPointerKind::Cancelled, PointerButton::Left);
            crate::chrome::chrome_dispatch_cancelled(state, &ev);
            state.mark_full_redraw();
        }
        WindowEvent::MouseInput {
            state: button_state,
            button,
            ..
        } => {
            // A surface above the page gets the button first. A blocking one swallows it: a press
            // on a button (its `on_click` emits `SubmitOverlay`) or on the scrim
            // (`Dialog::on_dismiss` emits `CloseOverlay`) resolves it, and anything else is
            // consumed so clicks don't leak to the page. A surface that only wants what lands on
            // it — a notification card's × — declines the rest, and the page's own press pipeline
            // below runs untouched.
            // The **release** goes in too — this branch used to forward the press alone, which is
            // what left a body's scrollbar thumb stuck to the cursor: the widget was still waiting
            // for the end of a gesture the host had decided not to deliver.
            {
                let kind = match button_state {
                    ElementState::Pressed => RawPointerKind::Pressed,
                    ElementState::Released => RawPointerKind::Released,
                };
                let ev = raw_pointer(state, kind, grid_button(button));
                if crate::chrome::dispatch_surface_pointer(state, &ev) {
                    return;
                }
            }
            // Pane info-bar action **buttons** intercept a plain left-press so a click
            // hits the button (not the terminal). Only an actual button hit is
            // consumed — a press on the empty header band falls through to the normal
            // content/drag/resize paths (the lower pane's band sits on the divider, so
            // consuming it would break divider/resize gestures). A modifier-held press
            // also falls through (meta-drag).
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && let Some((_pane_id, true)) =
                    crate::chrome::dispatch_pane_header_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            // Terminal viewport widgets (scrollbar / badge) intercept a plain
            // left-press before divider resize/content forwarding.
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && crate::chrome::dispatch_pane_viewport_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            // Divider resize: a plain left-press on a column/pane divider starts a
            // resize-drag. Only an actual divider hit consumes — a miss falls
            // through to the normal content/drag paths. A modifier-held press
            // (meta-drag) falls through. Checked after the header-button block
            // because the lower pane's header band sits on top of the divider.
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Pressed
                && !mouse::interactive_move_modifier_held(state)
                && mouse::resize::on_press(state, state.mouse.pos)
            {
                mouse::update_cursor(state, state.mouse.pos);
                state.mark_full_redraw();
                return;
            }
            // Read before the release block below ends any resize drag, so a release that ended
            // one still counts as consumed and is not also forwarded to the terminal.
            let resize_before = mouse::is_resizing(state);
            if button == winit::event::MouseButton::Left
                && button_state == ElementState::Released
            {
                // **The divider resize ends here, at the same level its press started it.** It used
                // to end inside `mouse::on_mouse_input`, which sits behind the viewport
                // early-return below — so a release that a pane's scrollbar happened to claim (it
                // answers one whenever it holds a thumb grab) never reached the resize, and
                // `state.mouse.resize` stayed `Some`. Every later cursor move then took the
                // resize branch in `mouse::on_cursor_moved` with no button held, and the pane went
                // on resizing itself until it was gone. A gesture must never outlive the release
                // that ends it — the same rule that keeps a scrollbar thumb from welding to the
                // cursor, one layer up (F004/P084/T409).
                mouse::resize::on_release(state);
                // Every retained tree that could have started a gesture gets the release, whether
                // or not the cursor is still over it — that is what ends a scrollbar drag. The
                // chrome tree is unconditional: it consumes nothing it did not start, and gating a
                // release on position is precisely how a thumb ends up welded to the cursor.
                crate::chrome::chrome_dispatch_release(state, state.mouse.pos);
                crate::chrome::dispatch_pane_header_release(state, state.mouse.pos);
                if crate::chrome::dispatch_pane_viewport_release(state, state.mouse.pos) {
                    mouse::update_cursor(state, state.mouse.pos);
                    state.mark_full_redraw();
                    return;
                }
            }
            let interactive_before = state.mouse.interactive_move.is_some();
            if let Some((action, source)) = mouse::on_mouse_input(state, button, button_state) {
                dispatch_action(state, registry, source, &action);
            }
            let started_interactive_move =
                !interactive_before && state.mouse.interactive_move.is_some();
            // A button event that started, drove, or ended a divider resize (e.g.
            // the right-button fallback press, or a release) must not also reach the
            // terminal — the gesture consumed it.
            let resize_consumed = resize_before || mouse::is_resizing(state);
            // A right-press that just opened the context menu (now a host-owned overlay layer)
            // must not also forward to the terminal (it would deliver a stray right-click to the TUI).
            let opened_context_menu = crate::chrome::top_modal(state).is_some();
            if !started_interactive_move && !resize_consumed && !opened_context_menu {
                forward_mouse_button(state, state.mouse.pos, button, button_state, registry);
            }
            // Snap the cursor on press/release (drag start → Grabbing, drop → Grab/Default)
            // without waiting for the next move.
            mouse::update_cursor(state, state.mouse.pos);
            state.mark_full_redraw();
        }
        WindowEvent::MouseWheel { delta, .. } => {
            // Ctrl/Meta+wheel is a font-zoom gesture, resolved by what's under the
            // pointer. It is intercepted at the WM level BEFORE terminal wheel
            // forwarding so the modified wheel never reaches the TUI as a scroll.
            if handle_wheel_font_zoom(state, registry, state.mouse.pos, delta) {
                state.mark_full_redraw();
                return;
            }
            // An open modal owns the wheel: its body may be a scroll region, and the page
            // behind must stay still either way. This branch did not exist, so a described
            // scroll area inside a modal could not be scrolled at all.
            let wheel = wheel_event(state, delta);
            if crate::chrome::dispatch_surface_pointer(state, &wheel) {
                return;
            }
            // The retained chrome tree next: a hovered scroll region in the sidebar takes it, and
            // the terminal must not also scroll. A region gates on its own hover, so this is a
            // no-op whenever the pointer is over a pane.
            if crate::chrome::chrome_dispatch_wheel(state, &wheel)
                || crate::chrome::dispatch_pane_header_wheel(state, &wheel)
            {
                state.mark_full_redraw();
                return;
            }
            forward_mouse_wheel(state, state.mouse.pos, delta);
            state.mark_full_redraw();
        }
        _ => {}
    }
}

/// Is a **layer** the surface in front of the keyboard?
///
/// The one question the key path needs before it hands a raw key to the tree, and it is asked of
/// the same rule everything else asks — [`focused_surface`](crate::app::input::focused_surface),
/// which AGENTS.md § Keybinding Style states as *a key acts on the surface in front of you*.
///
/// **A dock is deliberately not included.** A focused container answers keys through the actions it
/// declared (F003/P085/T352-T355): the key resolves in its own `[[keys.surface]]` map and arrives
/// as the intent it meant. Offering it the raw key here as well would be a second door onto one
/// input, and the two cannot stay identical — the thing ⭐⭐ RULE ZERO forbids.
///
/// This replaces `top_modal(state).is_some()` (F003/P097/T495). The difference is not the answer
/// but *what is asked*: the old form went on to name a node for the key to be delivered into, and
/// a host that names the target is a second opinion about what is in front. Now the key goes to
/// the tree and the tree delivers it to whatever holds focus — so a dialog's field, its buttons,
/// and a nested surface above it are reached by the rule that already reaches every other widget.
fn layer_holds_keyboard(state: &AppState) -> bool {
    matches!(
        crate::app::input::focused_surface(state),
        crate::app::input::FocusedSurface::Layer { .. }
    )
}

/// A winit wheel delta as grid-ui's [`Event::Scroll`], in **lines** (positive = down / right).
///
/// The host owns the modifier→axis mapping, because `Event::Scroll` carries both axes and a plain
/// mouse wheel only reports the vertical one: `Shift`+wheel is remapped to X. A trackpad already
/// fills X itself, so it is left alone. Same convention as the showcase, deliberately — a region
/// must scroll identically wherever it is mounted.
fn wheel_event(state: &AppState, delta: MouseScrollDelta) -> Event {
    let (dx_raw, dy_raw) = match delta {
        MouseScrollDelta::LineDelta(x, y) => (-x, -y),
        MouseScrollDelta::PixelDelta(p) => (-(p.x as f32) / 20.0, -(p.y as f32) / 20.0),
    };
    let (delta_x, delta_y) = if state.modifiers.shift_key() && dx_raw == 0.0 {
        (dy_raw, 0.0)
    } else {
        (dx_raw, dy_raw)
    };
    // No modifiers attached: the framework fills in what is held down, from the
    // `ModifiersChanged` this same loop broadcasts. Attaching them here is the second source of
    // truth that let a drop disagree with its own outline (F003/P097/T496).
    let mut raw = RawPointer::new(
        RawPointerKind::Wheel,
        Point::new(state.mouse.pos.0 as f64, state.mouse.pos.1 as f64),
    );
    raw.delta_x = delta_x;
    raw.delta_y = delta_y;
    Event::Raw(raw)
}

/// A raw pointer event at the current cursor, carrying the button and the live modifiers.
///
/// **One kind of pointer event leaves the host.** What it means — which widget it is for, whether
/// a press and a release were a click, whether the pointer just left something, whether a drag
/// began — is the framework's to work out, once, for every tree heca mounts. The host used to
/// answer some of those itself, per surface, and the answers drifted.
fn raw_pointer(state: &AppState, kind: RawPointerKind, button: PointerButton) -> Event {
    Event::Raw(
        RawPointer::new(
            kind,
            Point::new(state.mouse.pos.0 as f64, state.mouse.pos.1 as f64),
        )
        .with_button(button),
    )
}

/// winit's mouse button as the library's.
///
/// The press that reaches a widget now says **which** button it was, so a widget can own its own
/// right-click instead of the app rebuilding "what did you click" from a position.
fn grid_button(button: winit::event::MouseButton) -> PointerButton {
    match button {
        winit::event::MouseButton::Left => PointerButton::Left,
        winit::event::MouseButton::Right => PointerButton::Right,
        winit::event::MouseButton::Middle => PointerButton::Middle,
        winit::event::MouseButton::Back => PointerButton::Other(3),
        winit::event::MouseButton::Forward => PointerButton::Other(4),
        winit::event::MouseButton::Other(n) => PointerButton::Other(n),
    }
}

/// Handle a `Ctrl`/`Meta`+wheel font-zoom gesture. Returns `true` when the wheel
/// event was consumed as a zoom (so the caller must NOT forward it to terminal
/// scroll). Resolution is pointer-based: over a pane → zoom that pane; over
/// chrome/empty space → app-wide zoom. Scroll up zooms in, down zooms out.
fn handle_wheel_font_zoom(
    state: &mut AppState,
    registry: &ActionRegistry,
    pos: (f32, f32),
    delta: MouseScrollDelta,
) -> bool {
    if !state.mouse_wheel_change_font_size {
        return false;
    }
    let mods = state.modifiers;
    if !(mods.control_key() || mods.super_key()) {
        return false;
    }

    // Consume the gesture regardless of direction so a modified wheel never leaks
    // to the TUI; only dispatch when there is a usable vertical direction.
    let vertical = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(p) => p.y as f32,
    };
    if vertical == 0.0 {
        return true;
    }
    let step = if vertical > 0.0 {
        FontZoomStep::In
    } else {
        FontZoomStep::Out
    };

    let action = match mouse::hit_test_pane(state, pos) {
        Some(pane_id) => WmAction::PaneTerminalFontZoom {
            pane_id: Some(pane_id),
            step,
        },
        None => WmAction::AppFontZoom { step },
    };
    dispatch_action(state, registry, InteractionSource::MouseContent, &action);
    true
}

/// The grid-ui [`Modifiers`](heca_grid_ui::Modifiers) mirror of the current winit modifier state
/// — broadcast to an open overlay so a self-contained widget (e.g. a modal `Dialog`) can do
/// Shift+Tab / Ctrl+h-l without the host special-casing it.
fn grid_modifiers(m: winit::keyboard::ModifiersState) -> heca_grid_ui::Modifiers {
    heca_grid_ui::Modifiers {
        ctrl: m.control_key(),
        alt: m.alt_key(),
        shift: m.shift_key(),
        meta: m.super_key(),
    }
}
