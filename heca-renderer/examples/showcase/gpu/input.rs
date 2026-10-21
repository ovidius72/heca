use super::*;

impl GpuState {
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        // Keep the renderers' scissor-clamp size + the persistent scene texture in
        // sync with the framebuffer.
        self.grid
            .set_target_size(self.config.width, self.config.height);
        self.text
            .set_target_size(self.config.width, self.config.height);
        self.compositor
            .resize(&self.device, self.config.width, self.config.height);
        self.layout_dirty = true; // window size feeds layout
        self.window.request_redraw();
    }

    /// Light up (or clear) the workspaces-rail pick keycaps — the move/swap/select
    /// prefix. The host writes the per-cell hint signals; a real app would also
    /// reach this from RPC, hence the signal-driven path (input parity).
    pub(crate) fn set_rail_pick(&mut self, on: bool) {
        self.rail_pick = on;
        for (i, h) in self.rail_hints.iter().enumerate() {
            h.set(on.then(|| self.rail_letters[i].to_string()));
        }
    }

    /// Resolve a pressed letter to its rail cell: select that pane + end the pick.
    pub(crate) fn rail_pick_select(&mut self, c: char) {
        if let Some(i) = self.rail_letters.iter().position(|&l| l == c) {
            for (j, s) in self.rail_states.iter().enumerate() {
                s.set(j == i);
            }
            self.set_rail_pick(false);
        }
    }

    /// A raw pointer event at the current cursor, carrying the button and the live modifier
    /// state.
    ///
    /// A host builds **one** kind of pointer event and the framework works out what it means —
    /// which widget it is for, whether the pair of them was a click, whether the pointer just
    /// left something. The modifiers ride on the event rather than being read from state later,
    /// because an event that carries its own cannot be read against a state that has moved on.
    pub(crate) fn raw(&self, kind: RawPointerKind, button: PointerButton) -> RawPointer {
        RawPointer::new(kind, self.cursor)
            .with_button(button)
            .with_modifiers(Modifiers {
                ctrl: self.ctrl,
                alt: self.alt,
                shift: self.shift,
                meta: self.meta,
            })
    }

    /// The logical→physical scale actually used: the window's HiDPI factor times the
    /// global UI zoom. Everything (layout viewport, glyph rasterization, scissor +
    /// cursor mapping) goes through this, so changing zoom scales the whole UI.
    pub(crate) fn effective_scale(&self) -> f64 {
        let level = self.ctl.zoom.get_untracked();
        self.scale_factor * ZOOM_RATIO.powf(level - ZOOM_DEFAULT) as f64
    }

    /// The platform "accelerator" modifier: Cmd (⌘) on macOS, Ctrl elsewhere — so
    /// zoom uses the native chord (⌘ +/-/0 on macOS, Ctrl +/-/0 on Windows/Linux).
    pub(crate) fn accel(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }

    /// Nudge the zoom level (keys / accel+wheel), clamped to the dial range.
    pub(crate) fn nudge_zoom(&self, delta: f32) {
        let level = (self.ctl.zoom.get_untracked() + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        self.ctl.zoom.set(level);
        self.window.request_redraw();
    }

    /// Reset zoom to the neutral level.
    pub(crate) fn reset_zoom(&self) {
        self.ctl.zoom.set(ZOOM_DEFAULT);
        self.window.request_redraw();
    }

    pub(crate) fn enter_zoom_mode(&mut self) {
        self.zoom_mode = true;
        self.window.set_title(&format!(
            "{APP_TITLE} — ZOOM  (k/+ in · j/- out · 0 reset · Esc exit)"
        ));
        self.window.request_redraw();
    }

    pub(crate) fn exit_zoom_mode(&mut self) {
        self.zoom_mode = false;
        self.window.set_title(APP_TITLE);
        self.window.request_redraw();
    }

    /// The renderer-agnostic modifier state for the widget keymap.
    pub(crate) fn grid_mods(&self) -> Modifiers {
        Modifiers {
            ctrl: self.ctrl,
            alt: self.alt,
            shift: self.shift,
            meta: self.meta,
        }
    }

    /// Route a key to the active **overlay** (Select / ContextMenu / CommandPalette / Modal) via
    /// the widget [`Keymap`]. `dispatch` offers the raw key first (palette typing, quick-pick
    /// letters, a Modal's own keys), then the semantic `WidgetIntent`(s) the chord resolves to.
    /// `offer_to_overlay` no-ops when no overlay is active, so this returns `false` and the key
    /// falls through to the host's global bindings.
    pub(crate) fn route_overlay_key(&mut self, press: &heca_grid_ui::KeyPress) -> bool {
        let keymap = self.keymap.clone();
        keymap.deliver_press(press, |ev| {
            // The overlay layer first (Dialog / palette / menu live above the page);
            // when nothing there is open, an overlay INSIDE the page (an open Select
            // dropdown) still gets its keys via the page tree's own overlay scan.
            match self.focus_ov.offer_to_overlay(&mut self.overlays, ev) {
                Handled::Yes => Handled::Yes,
                Handled::No => self.focus.offer_to_overlay(&mut self.ui, ev),
            }
        }) == Handled::Yes
    }

    /// Route a key to the **focused** (non-overlay) widget via the widget [`Keymap`] — the demo
    /// host's stand-in for the app's config-driven resolution. `dispatch` delivers the raw key
    /// first (an `Input`'s typing / caret / Backspace), then the semantic `WidgetIntent`(s); the
    /// focused widget consumes whichever it understands — an `Input` takes the `Edit*` shortcut, a
    /// `Tabs` takes the horizontal `Item*` nav (so a shared `Ctrl+h` disambiguates by focus).
    pub(crate) fn route_focused_key(&mut self, press: &heca_grid_ui::KeyPress) {
        let keymap = self.keymap.clone();
        keymap.deliver_press(press, |ev| self.focus.deliver_event(&mut self.ui, ev));
    }

    /// A key pressed while in zoom mode. The mode stays active (so you can keep
    /// pressing j/k) until Esc/Enter/q.
    pub(crate) fn zoom_mode_key(&mut self, gk: GridKey) {
        match gk {
            GridKey::Char('k' | '+' | '=') => self.nudge_zoom(ZOOM_STEP),
            GridKey::Char('j' | '-' | '_') => self.nudge_zoom(-ZOOM_STEP),
            GridKey::Char('0') => self.reset_zoom(),
            GridKey::Escape | GridKey::Enter | GridKey::Char('q') => self.exit_zoom_mode(),
            _ => {} // swallow other keys while the mode is held
        }
    }

    /// The key after the tmux-style prefix: a heca command (the showcase only wires
    /// the zoom mode; the real app dispatches its full keymap here).
    pub(crate) fn prefix_command(&mut self, gk: GridKey) {
        self.prefix_pending = false;
        if matches!(gk, GridKey::Char('+' | '=')) {
            self.enter_zoom_mode();
        }
    }

}
