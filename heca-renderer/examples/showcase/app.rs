use super::*;

impl ApplicationHandler for App {
    /// When the capped-frame timer (set via `WaitUntil`) fires, request the next
    /// animation frame.
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause
            && let Some(state) = &self.state
            && state.next_frame_in.is_some()
        {
            state.window.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let state = pollster::block_on(GpuState::new(event_loop));
            state.window.request_redraw();
            self.state = Some(state);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        // Any non-redraw event (input, resize, …) can change unknown regions, so the
        // frame it triggers repaints in full. Animation/timed frames arrive as a bare
        // RedrawRequested and instead repaint only the widgets that flagged themselves.
        if !matches!(event, WindowEvent::RedrawRequested) {
            state.force_full = true;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                // Map through the zoom-aware scale so hit-testing matches the zoomed
                // layout (else clicks land on the wrong widgets when zoomed).
                let eff = state.effective_scale();
                state.cursor = Point::new(position.x / eff, position.y / eff);
                // The OS manages the cursor (arrow in content, resize at the
                // decorated window's edges) — don't override it.
                // Route hover through grid-ui: the overlay LAYER gets first dibs but
                // only swallows it if it consumes it (an input-grabbing Dialog/palette
                // returns Yes so items behind don't hover; the ToastStack returns Yes only for
                // the strip its cards actually cover), otherwise
                // it falls to the page tree (whose own overlay scan covers an open
                // Select dropdown) and then the widget under the cursor.
                let ev = Event::Raw(state.raw(RawPointerKind::Moved, PointerButton::Left));
                if state.focus_ov.dispatch(&mut state.overlays, &ev) == Handled::No {
                    state.focus.dispatch(&mut state.ui, &ev);
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Route the click through grid-ui: the overlay layer first (an open
                // Dialog/palette/menu captures it), then the page tree — whose own
                // overlay scan lets an open Select dropdown capture clicks on rows
                // outside its layout bounds; otherwise dispatch focuses the clicked
                // widget (clearing focus on a miss) and delivers the press.
                let ev = Event::Raw(state.raw(RawPointerKind::Pressed, PointerButton::Left));
                if state.focus_ov.dispatch(&mut state.overlays, &ev) == Handled::No {
                    state.focus.dispatch(&mut state.ui, &ev);
                }
                state.layout_dirty = true; // a click can change content/size
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                // Right-click opens the context menu at the cursor (the host decides
                // *where* and *when*; the widget just renders + captures input) —
                // but NOT when the point is occluded by an overlay drawn above the
                // page (an open Dialog/palette, a toast card, an open dropdown's
                // panel): the overlay owns that pixel, and a menu opening underneath
                // reads as broken layering. An already-open ContextMenu deliberately
                // does NOT occlude — a second right-click re-anchors it (standard
                // menu behavior). Both trees are scanned: the overlay layer and the
                // page (whose open Select panels also occlude).
                // The press goes into the trees first, with the button on it: a widget that
                // declares `on_right_click` owns its own menu, and the host only falls back to
                // its page-level one when nothing claimed the press.
                let ev = Event::Raw(state.raw(RawPointerKind::Pressed, PointerButton::Right));
                let claimed = state.focus_ov.dispatch(&mut state.overlays, &ev) == Handled::Yes
                    || state.focus.dispatch(&mut state.ui, &ev) == Handled::Yes;
                let occluded = heca_grid_ui::overlay_occluded_at(&state.overlays, state.cursor)
                    || heca_grid_ui::overlay_occluded_at(&state.ui, state.cursor);
                if !claimed && !occluded {
                    state.menu_anchor.set(state.cursor);
                    state.menu_open.set(true);
                    state.layout_dirty = true;
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                // Deliver the release to BOTH trees so a grabbed widget (a ScrollRegion
                // thumb drag in the page, a pressed Dialog button in the overlay layer)
                // always ends its grab — a release must never be swallowed by one tree
                // away from the other.
                let ev = Event::Raw(state.raw(RawPointerKind::Released, PointerButton::Left));
                state.focus_ov.dispatch(&mut state.overlays, &ev);
                state.focus.dispatch(&mut state.ui, &ev);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Device delta → lines (positive = down / right).
                let (dx_raw, dy_raw) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (-x, -y),
                    MouseScrollDelta::PixelDelta(p) => (-(p.x as f32) / 20.0, -(p.y as f32) / 20.0),
                };
                if state.zoom_mode || state.accel() {
                    // Wheel zooms the whole UI while in zoom mode, or with the
                    // accelerator held (scroll up = zoom in); vertical component only.
                    state.nudge_zoom(-dy_raw * ZOOM_STEP);
                } else {
                    // Shift+wheel scrolls horizontally: a plain mouse wheel only
                    // reports the vertical axis, so remap it to X while Shift is held
                    // (host owns the modifier→axis mapping; `Event::Scroll` carries
                    // both axes). A trackpad's native horizontal delta already fills X.
                    let (dx, dy) = if state.shift && dx_raw == 0.0 {
                        (dy_raw, 0.0)
                    } else {
                        (dx_raw, dy_raw)
                    };
                    // Route scroll through grid-ui: the overlay layer first (an open
                    // Dialog/palette may scroll its own body), then the page tree — an
                    // open Select dropdown takes it via the page's overlay scan, an
                    // embedded ScrollRegion under the cursor consumes it, and otherwise
                    // the ROOT ScrollRegion scrolls the whole page (a real scroll
                    // viewport).
                    let mut raw = state.raw(RawPointerKind::Wheel, PointerButton::Left);
                    raw.delta_x = dx;
                    raw.delta_y = dy;
                    let ev = Event::Raw(raw);
                    if state.focus_ov.dispatch(&mut state.overlays, &ev) == Handled::No {
                        state.focus.dispatch(&mut state.ui, &ev);
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                state.shift = s.shift_key();
                state.ctrl = s.control_key();
                state.alt = s.alt_key();
                state.meta = s.super_key();
                // Broadcast to BOTH trees so text widgets can do word-wise editing
                // (and the command palette — in the overlay layer — can track Ctrl
                // for Ctrl+J/K nav).
                let ev = Event::ModifiersChanged(Modifiers {
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    shift: s.shift_key(),
                    meta: s.super_key(),
                });
                heca_grid_ui::dispatch(&mut state.ui, &ev);
                heca_grid_ui::dispatch(&mut state.overlays, &ev);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // **Committed text goes in as text.** A field types from `Event::TextInput` and
                // from nothing else — the character the platform actually produced,
                // case and shifted symbols intact — while the key beside it is what a shortcut
                // reads. This host was never taught to send it, so the palette and every `Input`
                // here were unreachable from the keyboard while the identical widgets typed fine in
                // the app, which sends both.
                if let Some(gk) = heca_renderer::input::grid_key(&event.logical_key) {
                    match gk {
                        // ── tmux-style prefix + modes (checked FIRST) ──
                        // heca hosts other apps, so our chords go through a prefix
                        // (default Ctrl+B); bare keys fall through to the hosted app.
                        // Zoom mode owns j/k/+/-/0 until Esc — see `prefix +`.
                        _ if state.zoom_mode => state.zoom_mode_key(gk),
                        _ if state.prefix_pending => state.prefix_command(gk),
                        GridKey::Char('b') if state.ctrl => state.prefix_pending = true,
                        // An open overlay gets first dibs on keys, but only swallows
                        // the ones it actually consumes: a Modal/palette/Select eats the
                        // keys it uses (nav via `WidgetIntent`, Esc/Enter/typing), while the
                        // ToastStack eats none — so global keys (`t`, `[`, …) still work
                        // while toasts show. `route_overlay_key` resolves the key via the widget
                        // `Keymap` (the demo host's stand-in for config-driven `[keys.widgets]`).
                        gk if state.route_overlay_key(&heca_grid_ui::KeyPress {
                            key: gk,
                            text: event.text.as_ref().map(|t| t.to_string()),
                            mods: state.grid_mods(),
                        }) => {}
                        // Ctrl+K opens the command palette (a host-bound chord).
                        GridKey::Char('k') if state.ctrl => {
                            state.palette_open.set(true);
                        }
                        // Tab / Shift+Tab move keyboard focus across buttons.
                        GridKey::Tab => state.focus.advance(&mut state.ui, !state.shift),
                        // Escape clears focus and cancels an open rail pick.
                        GridKey::Escape => {
                            state.set_rail_pick(false);
                            state.focus.clear(&mut state.ui);
                        }
                        // `[` collapses the sidebar to its icon rail (G5) and back,
                        // but only when nothing is focused — so it still types into
                        // a focused Input.
                        GridKey::Char('[') if !heca_grid_ui::holds_keyboard(&state.ui) => {
                            let next = match state.sidebar_mode.get_untracked() {
                                RegionMode::CollapsedRail => RegionMode::Expanded,
                                _ => RegionMode::CollapsedRail,
                            };
                            state.sidebar_mode.set(next);
                        }
                        // `p` opens/closes a workspaces-rail pick: every pane cell
                        // shows its letter keycap (the generic KeyHint overlay).
                        GridKey::Char('p') if !heca_grid_ui::holds_keyboard(&state.ui) => {
                            let on = !state.rail_pick;
                            state.set_rail_pick(on);
                        }
                        // While a pick is open, a letter selects its pane cell.
                        GridKey::Char(c) if state.rail_pick => state.rail_pick_select(c),
                        // `c` toggles the centered clip-viewport demo (task B).
                        GridKey::Char('c') if !heca_grid_ui::holds_keyboard(&state.ui) => {
                            state.clip_demo = !state.clip_demo;
                        }
                        // `n` fires a "needs attention" pulse on a pane. The widget
                        // flashes; the *host* plays the sound (grid-ui is audio-free)
                        // — here, the terminal bell.
                        GridKey::Char('n') if !heca_grid_ui::holds_keyboard(&state.ui) => {
                            state.attention_req.set(true);
                            print!("\x07");
                            use std::io::Write;
                            let _ = std::io::stdout().flush();
                        }
                        // `t` pushes a new toast onto the host-owned list; the ToastStack slides
                        // it in and × dismisses (removes the id). Each push rotates the severity
                        // AND the action's button variant, because **the developer raising a
                        // notification chooses how its action reads** — the stack builds what the
                        // spec says and picks no face of its own.
                        GridKey::Char('t') if !heca_grid_ui::holds_keyboard(&state.ui) => {
                            state.toasts.update(|v| {
                                let id = v.iter().map(|s| s.id).max().unwrap_or(0) + 1;
                                let (severity, variant, how) = match id % 4 {
                                    1 => (ToastSeverity::Info, ButtonVariant::Primary, "primary"),
                                    2 => (
                                        ToastSeverity::Success,
                                        ButtonVariant::Secondary,
                                        "secondary",
                                    ),
                                    3 => {
                                        (ToastSeverity::Warning, ButtonVariant::Outline, "outline")
                                    }
                                    _ => (ToastSeverity::Danger, ButtonVariant::Ghost, "ghost"),
                                };
                                v.push(
                                    ToastSpec::new(id, format!("Event #{id}"))
                                        .severity(severity)
                                        .body(format!("Its action is a {how} button"))
                                        .action_with(
                                            ToastAction::new("view", "View").variant(variant),
                                        )
                                        // Every other push carries a SECOND action, so the row
                                        // that wraps and the per-action variant are both visible:
                                        // "one action" was a widget limit, not a real rule.
                                        .action_with(
                                            ToastAction::new("dismiss", "Not now")
                                                .variant(ButtonVariant::Ghost),
                                        ),
                                );
                            });
                            state.window.request_redraw();
                        }
                        // Space/Enter/typing and nav go to the focused widget — through the one
                        // funnel (`Keymap::deliver_press`): committed text, then the key, then the
                        // resolved `WidgetIntent`s. This surface writes none of that order.
                        other => {
                            let press = heca_grid_ui::KeyPress {
                                key: other,
                                text: event.text.as_ref().map(|t| t.to_string()),
                                mods: state.grid_mods(),
                            };
                            state.route_focused_key(&press);
                        }
                    }
                    state.layout_dirty = true; // a key can change content/size
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                // Throttle to the ~30fps cap. A widget that invalidates itself (the
                // spinner) requests a redraw immediately via `mark_needs_paint`, which
                // would otherwise render at full vsync. Input/resize frames (force_full)
                // still render at once for responsiveness.
                let min = Duration::from_millis(33);
                let elapsed = Instant::now().saturating_duration_since(state.last_frame);
                if !state.force_full && elapsed < min {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(state.last_frame + min));
                    return;
                }
                state.render();
                // Schedule the next frame: a continuous animation runs at the ~30fps
                // cap, a timed wake (e.g. caret blink) sleeps until its next change,
                // and a fully idle UI waits for input.
                match state.next_frame_in {
                    Some(secs) => {
                        let delay = Duration::from_secs_f32(secs.max(0.033));
                        event_loop
                            .set_control_flow(ControlFlow::WaitUntil(state.last_frame + delay));
                    }
                    None => event_loop.set_control_flow(ControlFlow::Wait),
                }
            }
            _ => {}
        }
    }
}

