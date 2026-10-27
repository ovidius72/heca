//! **Everything the terminal answers as a pane**: input, keys, mouse, paste, the update that drains
//! the program's output, damage, scrolling, search and configuration.

use super::TerminalBackend;
use crate::backend::{
    BackendAlert, BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType,
    TerminalDamage, TerminalLine, TerminalPaletteDefaults, TerminalRowRange, TerminalSnapshot,
};
use crate::runtime::PaneRuntime;

impl PaneBackend for TerminalBackend {
    fn reload_agent_config(&mut self, options: Option<crate::agent::AgentOptions>) -> Result<(), crate::agent::AgentError> {
        self.configure_agents(options)
    }

    fn pane_type(&self) -> PaneType {
        PaneType::Terminal
    }

    fn title(&self) -> &str {
        self.engine.title()
    }

    fn set_size(&mut self, cols: usize, rows: usize) {
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.engine.resize(cols, rows);
        self.force_full_damage = true;

        // The model is already the new size; the **child** is told at a paced rate.
        self.pending_pty_size = Some((cols, rows));
        self.flush_pending_pty_size();
    }

    fn process_input(&mut self, data: &[u8]) {
        let _ = self.pty.write_input(data);
    }

    fn process_key_event(&mut self, event: &BackendKeyEvent) -> bool {
        self.engine.process_key_event(event)
    }

    fn process_mouse_event(&mut self, event: &BackendMouseEvent) -> bool {
        self.engine.process_mouse_event(event)
    }

    fn focus_changed(&mut self, focused: bool) {
        self.engine.focus_changed(focused);
    }

    fn take_alerts(&mut self) -> Vec<BackendAlert> {
        self.engine.take_alerts()
    }

    fn take_clipboard_writes(&mut self) -> Vec<String> {
        self.engine.take_clipboard_writes()
    }

    fn paste(&mut self, text: &str) {
        // Wrap in bracketed-paste markers when the program enabled DECSET 2004, so
        // editors/shells treat the whole blob as literal pasted text (no auto-indent,
        // no executing newlines). Otherwise forward verbatim.
        if self.engine.bracketed_paste_enabled() {
            let mut framed = Vec::with_capacity(text.len() + 12);
            framed.extend_from_slice(b"\x1b[200~");
            framed.extend_from_slice(text.as_bytes());
            framed.extend_from_slice(b"\x1b[201~");
            self.process_input(&framed);
        } else {
            self.process_input(text.as_bytes());
        }
    }

    fn set_cell_size(&mut self, cell_w: f32, cell_h: f32) {
        // **Unchanged means do nothing** — the guard `set_size` has, which this was missing.
        //
        // `push_pixel_metrics` issues a `TIOCSWINSZ`, and the kernel answers every one of those
        // with a `SIGWINCH` to the child's process group. The host calls this from
        // `sync_terminal_backend_size` on **every frame, for every pane**, with a cell size it
        // recomputes each time — so heca was signalling every shell it hosts at frame rate,
        // forever, whether or not anything had changed.
        //
        // The comparison is exact on purpose: these are recomputed from the same inputs each
        // frame, so an unchanged layout produces bit-identical values, and a real change of any
        // size still gets through.
        if self.cell_w == cell_w && self.cell_h == cell_h {
            return;
        }
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.push_pixel_metrics();
        self.force_full_damage = true;
    }

    fn set_scale_factor(&mut self, scale: f32) {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        if (self.cell_scale - scale).abs() < f32::EPSILON {
            return;
        }
        self.cell_scale = scale;
        self.push_pixel_metrics();
    }

    fn update(&mut self) -> bool {
        let before = self.runtime.clone();
        // Deliver a size the child has not been told about yet — a drag holds them back, and this
        // is what makes sure the last one always lands (see `PTY_RESIZE_MIN_INTERVAL`).
        self.flush_pending_pty_size();
        let output = self.read_output();
        self.notice_exit();
        self.refresh_foreground_after(output);
        self.keep_the_view_on_output(output.had_data);
        output.had_data || output.agent_changed || self.runtime != before
    }

    fn next_wake(&self) -> Option<std::time::Instant> {
        self.pending_pty_size.map(|_| self.last_pty_resize + super::size::PTY_RESIZE_MIN_INTERVAL)
    }

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let snapshot = self.engine.snapshot(self.cell_size());
        snapshot.debug_assert_valid();
        Some(snapshot)
    }

    fn terminal_snapshot_at(&self, offset: usize) -> Option<TerminalSnapshot> {
        Some(self.engine.snapshot_at(self.cell_size(), offset))
    }

    fn take_terminal_damage(&mut self) -> TerminalDamage {
        let current_seqno = self.engine.current_seqno();
        let viewport_top = self.engine.visible_top_stable_row();
        // Q6: host viewport motion produces `TerminalDamage::Full` for the
        // retained terminal layer (incremental viewport damage is the separate
        // `terminal-01` phase). Drain the flag so only motion since the last read
        // arms the next frame.
        let viewport_changed = self.engine.take_viewport_changed();

        let damage = if self.force_full_damage || viewport_changed {
            TerminalDamage::Full
        } else if self.last_damage_seqno == Some(current_seqno)
            && self.last_damage_viewport_top == Some(viewport_top)
        {
            TerminalDamage::None
        } else if self.last_damage_viewport_top != Some(viewport_top) {
            // Viewport motion invalidates visible row identity, so per-row
            // damage is unsafe until retained-content work can reason about it.
            TerminalDamage::Full
        } else if let Some(last_seqno) = self.last_damage_seqno {
            coalesce_terminal_damage_rows(self.engine.changed_visible_rows_since(last_seqno))
        } else {
            TerminalDamage::Full
        };

        self.force_full_damage = false;
        self.last_damage_seqno = Some(current_seqno);
        self.last_damage_viewport_top = Some(viewport_top);
        damage
    }

    fn render_data(&self) -> BackendRenderData {
        let snapshot = self
            .terminal_snapshot()
            .unwrap_or_else(|| unreachable!("TerminalBackend always produces a terminal snapshot"));
        BackendRenderData::Terminal {
            lines: snapshot.lines,
            cursor_col: snapshot.cursor.col,
            cursor_row: snapshot.cursor.row,
            cell_w: snapshot.cell_w,
            cell_h: snapshot.cell_h,
        }
    }

    fn should_close(&self) -> bool {
        self.exited && self.auto_close_on_exit
    }

    fn cell_size(&self) -> (f32, f32) {
        (self.cell_w, self.cell_h)
    }

    fn runtime(&self) -> PaneRuntime {
        self.runtime.clone()
    }

    fn take_agent_updates(&mut self) -> Vec<Option<crate::agent::AgentState>> {
        self.agent_session.as_mut().map(|session| session.take_updates()).unwrap_or_default()
    }

    fn take_exit_code(&mut self) -> Option<i32> {
        self.pending_exit.take()
    }

    fn is_mouse_grabbed(&self) -> bool {
        self.engine.is_mouse_grabbed()
    }

    fn at_bottom(&self) -> bool {
        self.engine.at_bottom()
    }

    fn scroll_viewport(&mut self, delta_rows: i32) {
        self.engine.scroll_viewport(delta_rows);
    }

    fn scroll_to_top(&mut self) {
        self.engine.scroll_to_top();
    }

    fn scroll_to_bottom(&mut self) {
        self.engine.scroll_to_bottom();
    }

    fn tick_animation(&mut self) -> bool {
        self.engine.advance_animation()
    }

    fn scroll_viewport_animated(&mut self, delta_rows: i32) {
        self.engine.scroll_viewport_animated(delta_rows);
    }

    fn scroll_to_top_animated(&mut self) {
        self.engine.scroll_to_top_animated();
    }

    fn scroll_to_bottom_animated(&mut self) {
        self.engine.scroll_to_bottom_animated();
    }

    fn set_scroll_animations_enabled(&mut self, enabled: bool) {
        self.engine.set_scroll_animations_enabled(enabled);
    }

    fn set_link_detection(&mut self, enabled: bool) {
        self.engine.set_link_detection(enabled);
    }

    fn set_image_capture(&mut self, enabled: bool) {
        self.engine.set_image_capture(enabled);
    }

    fn reload_terminal_config(
        &mut self,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) {
        self.engine.reload_config(palette_defaults, scrollback_size);
        self.force_full_damage = true;
    }
    fn lines_in_stable_range(&self, start: isize, end: isize, cols: usize) -> Vec<TerminalLine> {
        self.engine.lines_in_stable_range(start, end, cols)
    }

    fn search_scrollback(&self, query: &str, cols: usize) -> Vec<crate::backend::SearchMatch> {
        self.engine.search_scrollback(query, cols)
    }

    fn search_stable_range(&self, query: &str, cols: usize, start: isize, end: isize) -> Vec<crate::backend::SearchMatch> {
        self.engine.search_stable_range(query, cols, start, end)
    }
}

impl TerminalBackend {
    /// **Keep the view where new output can be seen.**
    fn keep_the_view_on_output(&mut self, had_data: bool) {
        // Re-clamp the host viewport after draining output. PTY output is the
        // only path that shrinks scrollback without a resize (alt-screen entry,
        // `\x1b[2J` clears), so reconciling here keeps the stored `viewport_offset`
        // self-consistent before any downstream `snapshot()` read and surfaces the
        // correction as `TerminalDamage::Full` via `viewport_changed`.
        self.engine.reconcile_viewport_offset();

        // Snap-to-bottom policy (Q5): when new terminal output arrives and the
        // viewport is already at the live bottom (`at_bottom() == true` after
        // `reconcile_viewport_offset`), defensively re-snap so the growing
        // live region stays visible. The call is a no-op when already at offset 0
        // (the common case) but guards against future code paths that may move
        // the offset during reconciliation without marking the viewport dirty.
        // If the user has scrolled up, `at_bottom()` returns `false` and we do
        // NOT steal their viewport position — they'll snap back on key input.
        if had_data && self.engine.at_bottom() {
            self.engine.scroll_to_bottom();
        }
    }
}

pub(super) fn coalesce_terminal_damage_rows(rows: Vec<usize>) -> TerminalDamage {
    let mut ranges = Vec::new();
    let mut iter = rows.into_iter();
    let Some(mut start) = iter.next() else {
        return TerminalDamage::None;
    };
    let mut end = start + 1;

    for row in iter {
        if row == end {
            end += 1;
        } else {
            ranges.push(TerminalRowRange::new(start, end));
            start = row;
            end = row + 1;
        }
    }
    ranges.push(TerminalRowRange::new(start, end));
    TerminalDamage::Rows(ranges)
}
