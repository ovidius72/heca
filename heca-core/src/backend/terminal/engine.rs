use super::super::{
    BackendAlert, BackendKeyCode, BackendKeyEvent, BackendModifiers, BackendMouseButton,
    BackendMouseEvent, BackendMouseEventKind, GraphicsPlacement, HyperlinkSpan, SearchMatch,
    TerminalCell, TerminalImage, TerminalLine, TerminalPaletteDefaults, TerminalSnapshot,
    TerminalUnderlineStyle,
};
use crate::backend::{TerminalCursor, TerminalCursorShape};
use crate::layout::animation::{Animation, AnimationConfig};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Result as IoResult, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wezterm_surface::CursorVisibility;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::input::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use wezterm_term::{
    Alert, AlertHandler, CellAttributes, Clipboard, ClipboardSelection, Intensity, Terminal,
    TerminalConfiguration, TerminalSize,
};

/// Small wrapper around a shared PTY writer so `wezterm-term` can encode
/// responses and future keyboard/mouse input directly to the PTY.
#[derive(Clone)]
pub(super) struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    pub(super) fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }

    pub(super) fn write_bytes(&self, buf: &[u8]) -> IoResult<()> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.write_all(buf)?;
        writer.flush()
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.flush()
    }
}

#[derive(Debug)]
struct BellHandler {
    pending_bell: Arc<AtomicBool>,
}

impl AlertHandler for BellHandler {
    fn alert(&mut self, alert: Alert) {
        if matches!(alert, Alert::Bell) {
            self.pending_bell.store(true, Ordering::SeqCst);
        }
    }
}

/// Routes `OSC 52` clipboard writes from the terminal program into a shared
/// queue the app drains and forwards to the system clipboard.
///
/// Only **writes** are honored: a program can set the clipboard, but clipboard
/// *reads* over `OSC 52` (the `?` query form) are intentionally unsupported —
/// letting arbitrary terminal output exfiltrate clipboard contents is a known
/// security risk. wezterm parses and base64-decodes the sequence for us; we just
/// capture the decoded text.
struct OscClipboard {
    pending: Arc<Mutex<Vec<String>>>,
}

impl Clipboard for OscClipboard {
    fn set_contents(
        &self,
        _selection: ClipboardSelection,
        data: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(text) = data
            && !text.is_empty()
        {
            self.pending.lock().unwrap().push(text);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct HecaTerminalConfig {
    palette: ColorPalette,
    scrollback_size: usize,
}

impl TerminalConfiguration for HecaTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }

    fn scrollback_size(&self) -> usize {
        self.scrollback_size
    }

    /// Capture Kitty graphics in addition to the Sixel + iTerm2 `OSC 1337`
    /// protocols wezterm parses by default. All three attach `ImageCell`s to the
    /// grid, so enabling this widens inline-image support to every common tool
    /// (`kitten icat`, Yazi's kitty previewer, …) without a protocol-specific
    /// path. See `terminal-09`.
    fn enable_kitty_graphics(&self) -> bool {
        true
    }
}

pub(super) struct TerminalEngine {
    terminal: Terminal,
    cols: usize,
    rows: usize,
    pending_bell: Arc<AtomicBool>,
    /// Host viewport offset in rows above the live bottom.
    ///
    /// `0` = pinned to the live bottom (the default, matching the pre-`terminal-01a`
    /// behaviour of always projecting the bottom viewport). `N` = `N` rows of
    /// history are visible below the cursor row. Clamped to
    /// `[0, scrollback_rows - visible_rows]`.
    ///
    /// The engine is the rendering source of truth for this value (Q1 hybrid
    /// ownership); the app mirrors it into the chrome store for observability.
    viewport_offset: usize,
    /// Set whenever `viewport_offset` changes, so the next damage read forces a
    /// full retained-layer re-render with the new viewport rows (Q6: viewport
    /// motion produces `TerminalDamage::Full`; incremental viewport damage is
    /// the separate `terminal-01` phase).
    viewport_changed: bool,
    /// Optional animation for smooth viewport scrolling (slice 5). Discrete jumps
    /// (page/line/top/bottom) create an Animation; the wheel sets offset directly.
    /// `advance_animation()` snaps the animated f64 to the nearest integer row
    /// and updates `viewport_offset` when the rounded value changes. Re-targeted
    /// (never queued) on consecutive discrete jumps.
    viewport_anim: Option<Animation>,
    /// Config used to build viewport animations. Defaults to `AnimationConfig::default()`
    /// (the same 250ms `ease_out_cubic` `ViewOffset`/`activate_column` use) so terminal
    /// scroll feels like column scroll. Overridable per-engine (tests use 0ms for
    /// instant completion, or a long duration to exercise the ongoing path) without
    /// any real-time `sleep`.
    viewport_anim_config: AnimationConfig,
    /// Global on/off switch for backend-side viewport easing. When false, the
    /// animated APIs degrade to the immediate paths.
    viewport_anim_enabled: bool,
    /// Detect plain-text URLs in the visible grid and emit them as hyperlink
    /// spans (in addition to explicit OSC 8 links). Default `true`.
    link_detection: bool,
    /// Decoded-image cache keyed by wezterm's source content hash. Each unique
    /// inline image is decoded to RGBA exactly once and shared (`Arc`) across
    /// every snapshot that references it; `None` caches a decode failure so a
    /// broken payload is not retried each frame. `RefCell` because snapshotting
    /// is `&self`. See `terminal-09`.
    decoded_images: RefCell<HashMap<[u8; 32], Option<Arc<TerminalImage>>>>,
    /// Physical cell pixel size `(width, height)` reported to wezterm so it can
    /// size inline images and answer pixel-size queries. See [`Self::set_cell_px`].
    cell_px: (f32, f32),
    /// `OSC 52` clipboard-write requests captured from terminal output, drained
    /// by the app and pushed to the system clipboard. See [`OscClipboard`].
    clipboard_writes: Arc<Mutex<Vec<String>>>,
}

impl TerminalEngine {
    pub(super) fn new(
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
        writer: SharedWriter,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) -> Result<Self, super::PtyError> {
        let mut terminal = Terminal::new(
            terminal_size(cols, rows, cell_px),
            terminal_config(palette_defaults, scrollback_size),
            "heca",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer),
        );
        let pending_bell = Arc::new(AtomicBool::new(false));
        terminal.set_notification_handler(Box::new(BellHandler {
            pending_bell: Arc::clone(&pending_bell),
        }));
        let clipboard_writes = Arc::new(Mutex::new(Vec::new()));
        terminal.set_clipboard(&(Arc::new(OscClipboard {
            pending: Arc::clone(&clipboard_writes),
        }) as Arc<dyn Clipboard>));

        Ok(Self {
            terminal,
            cols,
            rows,
            pending_bell,
            viewport_offset: 0,
            viewport_changed: false,
            viewport_anim: None,
            viewport_anim_config: AnimationConfig::default(),
            viewport_anim_enabled: true,
            link_detection: true,
            decoded_images: RefCell::new(HashMap::new()),
            cell_px,
            clipboard_writes,
        })
    }

    /// Update the physical cell pixel size reported to the emulation layer.
    ///
    /// wezterm divides the terminal's pixel dimensions by the cell grid to size
    /// inline images (Sixel/iTerm2/Kitty) and to answer pixel-size queries
    /// (`CSI 14 t` / `CSI 16 t`) that image tools use to scale previews. A stale
    /// or zero size leaves images mis-sized (and divides by zero on Sixel), so we
    /// re-issue the terminal size whenever the renderer's cell metrics change.
    pub(super) fn set_cell_px(&mut self, cell_px: (f32, f32)) {
        if self.cell_px == cell_px {
            return;
        }
        self.cell_px = cell_px;
        self.terminal.resize(terminal_size(self.cols, self.rows, cell_px));
    }

    /// Enable or disable plain-text URL auto-detection (linkify).
    pub(super) fn set_link_detection(&mut self, enabled: bool) {
        self.link_detection = enabled;
    }

    pub(super) fn title(&self) -> &str {
        self.terminal.get_title()
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.terminal.resize(terminal_size(cols, rows, self.cell_px));
        // A resize reflows scrollback (visible rows change, history moves), which
        // changes the max valid offset. Re-clamp AFTER the wezterm resize so the
        // boundary reflects the reflowed scrollback, never pointing past the new top.
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset > max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    pub(super) fn advance_bytes(&mut self, bytes: &[u8]) {
        self.terminal.advance_bytes(bytes);
    }

    /// Drain `OSC 52` clipboard-write requests captured since the last poll. The
    /// app forwards them to the system clipboard.
    pub(super) fn take_clipboard_writes(&self) -> Vec<String> {
        std::mem::take(&mut self.clipboard_writes.lock().unwrap())
    }

    /// Whether the program has enabled bracketed paste (DECSET 2004), so pasted
    /// text must be wrapped in `ESC[200~ … ESC[201~`.
    pub(super) fn bracketed_paste_enabled(&self) -> bool {
        self.terminal.bracketed_paste_enabled()
    }

    pub(super) fn take_alerts(&self) -> Vec<BackendAlert> {
        if self.pending_bell.swap(false, Ordering::SeqCst) {
            vec![BackendAlert::Bell]
        } else {
            Vec::new()
        }
    }

    /// Current host viewport offset in rows above the live bottom.
    ///
    /// Test-only inspection helper; production reads the viewport from the
    /// `TerminalSnapshot` (the rendering source of truth per Q1).
    #[cfg(test)]
    pub(super) fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    pub(super) fn at_bottom(&self) -> bool {
        self.viewport_offset == 0
    }

    /// Whether the hosted application has enabled terminal mouse reporting modes
    /// (X10/SGR/any-event). When `true`, wheel and click events should be forwarded
    /// to the terminal instead of being handled by heca's host chrome.
    pub(super) fn is_mouse_grabbed(&self) -> bool {
        self.terminal.is_mouse_grabbed()
    }

    /// Test-only setter that bypasses the clamp, used to construct a stale stored
    /// offset for `reconcile_viewport_offset` unit tests (the write paths all clamp
    /// on entry, so stale state can only arise from a scrollback shrink).
    #[cfg(test)]
    pub(super) fn set_viewport_offset_for_test(&mut self, offset: usize) {
        self.viewport_offset = offset;
    }

    /// Enable or disable backend-side viewport animations.
    pub(super) fn set_scroll_animations_enabled(&mut self, enabled: bool) {
        self.viewport_anim_enabled = enabled;
        if !enabled {
            self.viewport_anim = None;
        }
    }

    /// Reload the terminal emulation config (palette + scrollback size) live.
    /// Used by `prefix+Shift+r` so theme/config changes apply to existing PTYs.
    pub(super) fn reload_config(
        &mut self,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) {
        self.terminal
            .set_config(terminal_config(palette_defaults, scrollback_size));
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset > max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    /// Test-only override of the viewport-animation config. Lets animation tests
    /// exercise the real code path deterministically: a 0ms duration completes on
    /// the first `advance_animation()` (done path), a long duration stays ongoing
    /// without any real-time `sleep` (running path).
    #[cfg(test)]
    pub(super) fn set_viewport_anim_config_for_test(&mut self, config: AnimationConfig) {
        self.viewport_anim_config = config;
    }

    /// Maximum valid viewport offset = total retained rows minus the visible
    /// row count. Returns `0` when there is no scrollback yet.
    fn max_viewport_offset(&self) -> usize {
        let screen = self.terminal.screen();
        let visible_count = self.rows.min(screen.physical_rows.max(1));
        screen.scrollback_rows().saturating_sub(visible_count)
    }

    /// Scroll the host viewport by `delta_rows`.
    ///
    /// Positive values move toward history (offset increases); negative values
    /// move toward the live bottom (offset decreases). The offset is clamped to
    /// `[0, max_viewport_offset()]`. No-op if the clamped value does not change.
    pub(super) fn scroll_viewport(&mut self, delta_rows: i32) {
        // Wheel path: apply immediately, clear any ongoing animation.
        self.viewport_anim = None;
        let target = if delta_rows >= 0 {
            self.viewport_offset
                .saturating_add(delta_rows.try_into().unwrap_or(usize::MAX))
        } else {
            self.viewport_offset
                .saturating_sub((-delta_rows).try_into().unwrap_or(usize::MAX))
        };
        let clamped = target.min(self.max_viewport_offset());
        if clamped != self.viewport_offset {
            self.viewport_offset = clamped;
            self.viewport_changed = true;
        }
    }

    /// Jump the viewport to the top of scrollback (maximum offset).
    pub(super) fn scroll_to_top(&mut self) {
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset != max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    /// Snap the viewport to the live bottom (`viewport_offset = 0`).
    pub(super) fn scroll_to_bottom(&mut self) {
        if self.viewport_offset != 0 {
            self.viewport_offset = 0;
            self.viewport_changed = true;
        }
    }

    // ── Animated scroll (slice 5) ──

    /// Advance the ongoing viewport animation. Snaps the animated `f64` to the
    /// nearest integer row and updates `viewport_offset` (setting
    /// `viewport_changed`) when the rounded value moves. Returns `true` while
    /// the animation is still running (caller should keep requesting frames).
    pub(super) fn advance_animation(&mut self) -> bool {
        let Some(anim) = self.viewport_anim.as_ref() else {
            return false;
        };
        if anim.is_done() {
            let target = anim.target() as usize;
            if self.viewport_offset != target {
                self.viewport_offset = target;
                self.viewport_changed = true;
            }
            self.viewport_anim = None;
            return false;
        }
        let current = anim.value();
        let rounded = current.round() as usize;
        if rounded != self.viewport_offset {
            self.viewport_offset = rounded;
            self.viewport_changed = true;
        }
        true
    }

    /// Scroll the viewport with animation for discrete user jumps.
    /// Creates (or re-targets) an `Animation` from the current animated value
    /// to the newly computed clamped target. Never queues — a new jump
    /// interrupts the previous animation.
    pub(super) fn scroll_viewport_animated(&mut self, delta_rows: i32) {
        if !self.viewport_anim_enabled {
            self.scroll_viewport(delta_rows);
            return;
        }
        // Re-target: base the delta on where we're heading (the animation's target),
        // not where we currently are. A new jump re-creates the Animation from the
        // current animated value toward the new target (never queues).
        let base = self
            .viewport_anim
            .as_ref()
            .map_or(self.viewport_offset, |a| a.target() as usize);
        let current = self
            .viewport_anim
            .as_ref()
            .map_or(self.viewport_offset as f64, |a| a.value());
        let target = if delta_rows >= 0 {
            base.saturating_add(delta_rows.try_into().unwrap_or(usize::MAX))
        } else {
            base.saturating_sub((-delta_rows).try_into().unwrap_or(usize::MAX))
        };
        let clamped = target.min(self.max_viewport_offset());
        if (current.round() as usize) != clamped {
            self.viewport_anim = Some(Animation::new(
                current,
                clamped as f64,
                self.viewport_anim_config,
            ));
        }
    }

    /// Animate to the top of scrollback.
    pub(super) fn scroll_to_top_animated(&mut self) {
        if !self.viewport_anim_enabled {
            self.scroll_to_top();
            return;
        }
        let current = self
            .viewport_anim
            .as_ref()
            .map_or(self.viewport_offset as f64, |a| a.value());
        let target = self.max_viewport_offset();
        if (current.round() as usize) != target {
            self.viewport_anim = Some(Animation::new(
                current,
                target as f64,
                self.viewport_anim_config,
            ));
        }
    }

    /// Animate to the live bottom (`viewport_offset = 0`).
    pub(super) fn scroll_to_bottom_animated(&mut self) {
        if !self.viewport_anim_enabled {
            self.scroll_to_bottom();
            return;
        }
        let current = self
            .viewport_anim
            .as_ref()
            .map_or(self.viewport_offset as f64, |a| a.value());
        if (current.round() as usize) != 0 {
            self.viewport_anim = Some(Animation::new(
                current,
                0.0,
                self.viewport_anim_config,
            ));
        }
    }

    /// Drain the viewport-changed flag. Returns `true` when the viewport moved
    /// since the last call, which the host must treat as `TerminalDamage::Full`
    /// for the retained terminal layer (Q6).
    pub(super) fn take_viewport_changed(&mut self) -> bool {
        std::mem::take(&mut self.viewport_changed)
    }

    /// Re-clamp the stored `viewport_offset` to the current scrollback boundary and
    /// write the correction back.
    ///
    /// Scrollback can shrink without a resize (alt-screen entry / `\x1b[2J` clears
    /// drop `scrollback_rows` to near zero), which would otherwise leave the stored
    /// offset pointing past the new top. This keeps the engine's stored state
    /// self-consistent — the single source of truth — rather than silently reading a
    /// corrected value in `visible_lines` while the stored field drifts. Any
    /// correction arms `viewport_changed` so it surfaces as `TerminalDamage::Full`
    /// through the damage system.
    ///
    /// Called at the end of [`TerminalBackend::update`] (right after PTY output is
    /// drained, where alt-screen/clear transitions happen), so the stored offset is
    /// consistent before any downstream `snapshot()` read.
    pub(super) fn reconcile_viewport_offset(&mut self) -> bool {
        let max_offset = self.max_viewport_offset();
        // Clear any ongoing animation — it was targeting an offset past the new
        // boundary; jumping to the clamped position is the correct resolution.
        if self.viewport_offset > max_offset {
            self.viewport_anim = None;
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
            true
        } else if let Some(anim) = &self.viewport_anim {
            // The animation's target may also be past the new max.
            if anim.target() as usize > max_offset {
                self.viewport_anim = None;
                self.viewport_offset = self.viewport_offset.min(max_offset);
                self.viewport_changed = true;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(super) fn process_key_event(&mut self, event: &BackendKeyEvent) -> bool {
        self.terminal
            .key_down(map_key_code(event.code), map_modifiers(event.modifiers))
            .is_ok()
    }

    pub(super) fn process_mouse_event(&mut self, event: &BackendMouseEvent) -> bool {
        self.terminal
            .mouse_event(MouseEvent {
                kind: map_mouse_event_kind(event.kind),
                x: event.col,
                y: event.row as i64,
                x_pixel_offset: event.x_pixel_offset,
                y_pixel_offset: event.y_pixel_offset,
                button: map_mouse_button(event.button),
                modifiers: map_modifiers(event.modifiers),
            })
            .is_ok()
    }

    pub(super) fn focus_changed(&mut self, focused: bool) {
        self.terminal.focus_changed(focused);
    }

    pub(super) fn snapshot(&self, cell_size: (f32, f32)) -> TerminalSnapshot {
        let (cell_w, cell_h) = cell_size;
        let palette = self.terminal.palette();
        let blank = blank_cell(&palette);
        let size = self.terminal.get_size();
        let cols = size.cols.max(1);
        let rows = size.rows.max(1);
        let blank_line = TerminalLine {
            cells: vec![blank; cols],
        };
        let (lines, scrollback_rows, viewport_offset, mut hyperlinks, graphics, images) =
            self.visible_lines(cols, rows, &palette, &blank_line);

        // Auto-detect plain-text URLs (echo, logs, …) and add them as link spans
        // alongside the explicit OSC 8 ones. OSC 8 wins: a cell already inside an
        // explicit link is never re-linked.
        if self.link_detection {
            detect_plain_links(&lines, &mut hyperlinks);
        }

        let cursor = self.terminal.cursor_pos();
        let snapshot = TerminalSnapshot {
            cols,
            rows,
            cell_w,
            cell_h,
            default_fg: to_rgba(palette.resolve_fg(ColorAttribute::Default)),
            default_bg: to_rgba(palette.resolve_bg(ColorAttribute::Default)),
            cursor_color: to_rgba(palette.cursor_bg),
            cursor: TerminalCursor {
                col: cursor.x.min(cols.saturating_sub(1)),
                row: (cursor.y.max(0) as usize).min(rows.saturating_sub(1)),
                visible: cols > 0 && rows > 0 && cursor.visibility == CursorVisibility::Visible,
                shape: map_cursor_shape(cursor.shape),
            },
            lines,
            viewport_offset,
            at_bottom: viewport_offset == 0,
            scrollback_rows,
            viewport_top_stable_row: self.visible_top_stable_row(),
            hyperlinks,
            graphics,
            images,
        };
        snapshot.debug_assert_valid();
        snapshot
    }

    /// Fetch the inclusive stable-row range `[start, end]` as renderer-ready
    /// [`TerminalLine`]s, padded/truncated to `cols`.
    ///
    /// Host-grid selections live in stable-row coordinates; copying a selection
    /// that spans history requires fetching content by stable row rather than
    /// indexing the visible snapshot. Each stable row maps 1:1 to a physical
    /// (ring-buffer) index via wezterm's `stable_row_to_phys`; phys indices
    /// ascend monotonically with stable rows, so the clamped range maps to an
    /// ascending phys range consumed by `lines_in_phys_range`.
    ///
    /// Rows outside the current retained range yield `None` and are emitted as
    /// blank lines so the returned count always matches `end - start + 1`.
    pub(super) fn lines_in_stable_range(
        &self,
        start: isize,
        end: isize,
        cols: usize,
    ) -> Vec<TerminalLine> {
        let screen = self.terminal.screen();
        let palette = self.terminal.palette();
        let blank = blank_cell(&palette);
        let blank_line = TerminalLine {
            cells: vec![blank.clone(); cols],
        };
        if start > end || cols == 0 {
            return Vec::new();
        }
        let mut out: Vec<TerminalLine> = Vec::with_capacity((end - start + 1) as usize);
        for stable in start..=end {
            match screen.stable_row_to_phys(stable) {
                Some(phys) => {
                    let phys_lines = screen.lines_in_phys_range(phys..phys + 1);
                    if let Some(mut line) = phys_lines.into_iter().next() {
                        out.push(snapshot_line(&mut line, cols, &palette, &blank_line));
                    } else {
                        out.push(blank_line.clone());
                    }
                }
                None => out.push(blank_line.clone()),
            }
        }
        out
    }

    /// Case-insensitive substring search over the **entire** scrollback (history +
    /// visible), returning every matching run as a [`SearchMatch`] in ascending
    /// stable-row / column order. Reads cell text directly (no color resolution),
    /// fills inter-cell gaps with spaces so internal spaces match and columns stay
    /// aligned, and reports non-overlapping matches.
    pub(super) fn search_scrollback(&self, query: &str, cols: usize) -> Vec<SearchMatch> {
        if query.is_empty() || cols == 0 {
            return Vec::new();
        }
        let needle: Vec<char> = query.to_lowercase().chars().collect();
        let screen = self.terminal.screen();
        let rows = screen.physical_rows.max(1);
        let visible_count = self.rows.min(rows);
        let max_offset = screen.scrollback_rows().saturating_sub(visible_count);
        let top_stable = screen.visible_row_to_stable_row(0) - max_offset as isize;
        let bottom_stable = screen.visible_row_to_stable_row(rows as i64 - 1);

        let mut out = Vec::new();
        for stable in top_stable..=bottom_stable {
            let Some(phys) = screen.stable_row_to_phys(stable) else {
                continue;
            };
            let lines = screen.lines_in_phys_range(phys..phys + 1);
            let Some(line) = lines.into_iter().next() else {
                continue;
            };
            // Lowercased char vector + the column each char sits in.
            let mut chars: Vec<char> = Vec::new();
            let mut col_of: Vec<usize> = Vec::new();
            let mut next_col = 0usize;
            for cell in line.visible_cells() {
                let idx = cell.cell_index();
                if idx >= cols {
                    break;
                }
                // Fill the gap of blank cells before this one with spaces.
                while next_col < idx {
                    chars.push(' ');
                    col_of.push(next_col);
                    next_col += 1;
                }
                for ch in cell.str().chars().flat_map(char::to_lowercase) {
                    chars.push(ch);
                    col_of.push(idx);
                }
                next_col = idx + cell.width().max(1);
            }
            if needle.len() > chars.len() {
                continue;
            }
            let mut i = 0;
            while i + needle.len() <= chars.len() {
                if chars[i..i + needle.len()] == needle[..] {
                    out.push(SearchMatch {
                        stable_row: stable,
                        start_col: col_of[i],
                        end_col: col_of[i + needle.len() - 1] + 1,
                    });
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
        }
        out
    }

    pub(super) fn current_seqno(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub(super) fn visible_top_stable_row(&self) -> isize {
        // wezterm's `visible_row_to_stable_row(0)` reports the LIVE viewport top
        // (offset 0). heca scrolls the host viewport by `viewport_offset` rows
        // ABOVE the live bottom, so the displayed top is that many rows older
        // (a smaller stable row index). Subtracting the offset makes
        // `viewport_top_stable_row` track the scrolled content instead of staying
        // pinned to the live screen — which selection overlays and caret
        // auto-scroll depend on (terminal-task-01g).
        self.terminal.screen().visible_row_to_stable_row(0)
            - self.viewport_offset as isize
    }

    pub(super) fn changed_visible_rows_since(&self, seqno: usize) -> Vec<usize> {
        let screen = self.terminal.screen();
        let rows = screen.physical_rows.max(1);
        let top = screen.visible_row_to_stable_row(0);
        let bottom = screen.visible_row_to_stable_row(rows.saturating_sub(1) as i64);
        screen
            .get_changed_stable_rows(top..bottom.saturating_add(1), seqno)
            .into_iter()
            .filter_map(|stable_row| {
                let visible_row = stable_row - top;
                usize::try_from(visible_row).ok().filter(|row| *row < rows)
            })
            .collect()
    }

    #[allow(clippy::type_complexity)]
    fn visible_lines(
        &self,
        cols: usize,
        rows: usize,
        palette: &ColorPalette,
        blank_line: &TerminalLine,
    ) -> (
        Vec<TerminalLine>,
        usize,
        usize,
        Vec<HyperlinkSpan>,
        Vec<GraphicsPlacement>,
        Vec<TerminalImage>,
    ) {
        let screen = self.terminal.screen();
        let visible_count = rows.min(screen.physical_rows.max(1));
        let total = screen.scrollback_rows();
        let max_offset = total.saturating_sub(visible_count);
        // Defensive read-clamp: `reconcile_viewport_offset` (called at the end of
        // `update`) keeps the stored offset within bounds, so this normally does
        // nothing. It stays as a belt-and-suspenders guarantee that a snapshot is
        // always visually correct even if a read happens outside the update path.
        let offset = self.viewport_offset.min(max_offset);
        // `total` is the bottom of the live viewport; subtract `offset` to walk
        // `offset` rows up into history, then take `visible_count` rows below.
        let visible_end = total.saturating_sub(offset);
        let visible_start = visible_end.saturating_sub(visible_count);
        let mut lines = Vec::with_capacity(rows);
        let mut hyperlinks = Vec::new();
        let mut graphics = GraphicsCollector::default();

        for (row, mut line) in screen
            .lines_in_phys_range(visible_start..visible_end)
            .into_iter()
            .take(rows)
            .enumerate()
        {
            collect_row_hyperlinks(&line, row, cols, &mut hyperlinks);
            self.collect_row_graphics(&line, row, cols, &mut graphics);
            lines.push(snapshot_line(&mut line, cols, palette, blank_line));
        }

        while lines.len() < rows {
            lines.push(blank_line.clone());
        }

        let (placements, images) = graphics.finish();
        (lines, total, offset, hyperlinks, placements, images)
    }

    /// Collect inline-image placements for one visible row into `out`.
    ///
    /// wezterm attaches an [`ImageCell`] to each grid cell an image covers
    /// (identically for Sixel / iTerm2 / Kitty). We group those per-cell slices
    /// by `(content hash, placement id, z-index)` into rectangular blocks — see
    /// [`GraphicsCollector`] — and decode each unique source image once.
    fn collect_row_graphics(
        &self,
        line: &wezterm_term::Line,
        row: usize,
        cols: usize,
        out: &mut GraphicsCollector,
    ) {
        for cell in line.visible_cells() {
            let col = cell.cell_index();
            if col >= cols {
                break;
            }
            let Some(images) = cell.attrs().images() else {
                continue;
            };
            for image in images {
                let data = image.image_data();
                let hash = data.hash();
                // Decode/register the source image once; skip undecodable ones.
                let Some(decoded) = self.decode_image(data) else {
                    continue;
                };
                out.add_cell(
                    hash,
                    image.placement_id(),
                    image.z_index(),
                    row,
                    col,
                    texcoord(image.top_left()),
                    texcoord(image.bottom_right()),
                    &decoded,
                );
            }
        }
    }

    /// Decode (or fetch from cache) the RGBA pixels for one source image. Returns
    /// `None` for an undecodable payload (already-logged, cached so it is not
    /// retried every frame).
    fn decode_image(
        &self,
        data: &Arc<wezterm_term::image::ImageData>,
    ) -> Option<Arc<TerminalImage>> {
        let hash = data.hash();
        if let Some(cached) = self.decoded_images.borrow().get(&hash) {
            return cached.clone();
        }
        let decoded = decode_terminal_image(hash, &data.data()).map(Arc::new);
        self.decoded_images.borrow_mut().insert(hash, decoded.clone());
        decoded
    }
}

fn snapshot_line(
    line: &mut wezterm_term::Line,
    cols: usize,
    palette: &ColorPalette,
    blank_line: &TerminalLine,
) -> TerminalLine {
    let mut cells = blank_line.cells.clone();
    for (idx, cell) in line.cells_mut().iter().enumerate().take(cols) {
        cells[idx] = terminal_cell(" ", 1, cell.attrs(), palette);
    }
    for cell in line.visible_cells() {
        if cell.cell_index() >= cols {
            continue;
        }

        let rendered = terminal_cell(cell.str(), cell.width(), cell.attrs(), palette);
        let span_end = (cell.cell_index() + rendered.width).min(cols);
        for slot in cells.iter_mut().take(span_end).skip(cell.cell_index()) {
            *slot = blank_with_attrs(&rendered);
        }
        cells[cell.cell_index()] = rendered;
    }

    TerminalLine { cells }
}

/// Collect OSC 8 hyperlink spans for one visible row. Consecutive cells that
/// carry the same link URI are merged into a single span (in cell columns), so a
/// later phase can underline / open contiguous links without re-scanning.
fn collect_row_hyperlinks(
    line: &wezterm_term::Line,
    row: usize,
    cols: usize,
    out: &mut Vec<HyperlinkSpan>,
) {
    let mut open: Option<HyperlinkSpan> = None;
    for cell in line.visible_cells() {
        let idx = cell.cell_index();
        if idx >= cols {
            break;
        }
        let end = (idx + cell.width().max(1)).min(cols);
        match cell.attrs().hyperlink().map(|link| link.uri()) {
            Some(uri) => match open.as_mut() {
                // Extend the run only if it is the same link and contiguous.
                Some(span) if span.uri == uri && span.end_col == idx => span.end_col = end,
                _ => {
                    if let Some(span) = open.take() {
                        out.push(span);
                    }
                    open = Some(HyperlinkSpan {
                        row,
                        start_col: idx,
                        end_col: end,
                        uri: uri.to_string(),
                    });
                }
            },
            None => {
                if let Some(span) = open.take() {
                    out.push(span);
                }
            }
        }
    }
    if let Some(span) = open.take() {
        out.push(span);
    }
}

/// Convert a wezterm source texture coordinate to a plain `[x, y]` pair.
fn texcoord(c: wezterm_surface::TextureCoordinate) -> [f32; 2] {
    [c.x.into_inner(), c.y.into_inner()]
}

/// Derive the stable 64-bit image handle the snapshot/renderer use from
/// wezterm's 32-byte content hash. Truncating to 8 bytes of SHA-256 keeps
/// collisions astronomically unlikely while giving a cheap cache key.
fn image_id_from_hash(hash: [u8; 32]) -> u64 {
    u64::from_le_bytes(hash[..8].try_into().expect("hash is 32 bytes"))
}

/// Decode one source image to tightly packed RGBA8. `Rgba8` is used as-is;
/// encoded payloads (iTerm2 `OSC 1337` files, Kitty PNG, …) go through the
/// `image` crate; animations contribute their first frame (full animation is a
/// `terminal-09` Stage 4 follow-up). Returns `None` when the payload cannot be
/// decoded.
fn decode_terminal_image(
    hash: [u8; 32],
    data: &wezterm_term::image::ImageDataType,
) -> Option<TerminalImage> {
    use wezterm_term::image::ImageDataType;
    let (width, height, rgba) = match data {
        ImageDataType::Rgba8 {
            data,
            width,
            height,
            ..
        } => (*width, *height, data.clone()),
        ImageDataType::AnimRgba8 {
            width,
            height,
            frames,
            ..
        } => (*width, *height, frames.first()?.clone()),
        ImageDataType::EncodedFile(bytes) => decode_encoded_image(bytes)?,
        ImageDataType::EncodedLease(lease) => {
            let bytes = lease.get_data().ok()?;
            decode_encoded_image(&bytes)?
        }
    };
    if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    Some(TerminalImage {
        id: image_id_from_hash(hash),
        width,
        height,
        rgba: Arc::from(rgba.into_boxed_slice()),
    })
}

/// Decode an encoded image blob (PNG/JPEG/GIF/WebP/BMP) to RGBA8 bytes.
fn decode_encoded_image(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|err| eprintln!("terminal inline image: decode failed: {err:#}"))
        .ok()?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    Some((width, height, decoded.into_vec()))
}

/// Accumulates per-cell [`ImageCell`] slices into rectangular placement blocks
/// while a snapshot is built row by row.
///
/// wezterm lays an image across a uniform grid of cells, each carrying the
/// source texcoord sub-rect for its slice, and clips whole rows/cols when the
/// image scrolls off-screen. So one block per `(hash, placement id, z-index)`
/// group, sampling from the first (top-left) cell's `top_left` to the last
/// (bottom-right) cell's `bottom_right`, reproduces the visible region exactly —
/// even partially scrolled. Rows arrive top-to-bottom and cells left-to-right,
/// so the first cell seen for a group is its top-left and the last is its
/// bottom-right.
#[derive(Default)]
struct GraphicsCollector {
    groups: Vec<GraphicsGroup>,
    /// Index into `groups` keyed by `(image hash, placement id, z-index)`.
    index: HashMap<([u8; 32], Option<u32>, i32), usize>,
    /// Unique decoded images encountered, deduplicated by id.
    images: HashMap<u64, Arc<TerminalImage>>,
}

struct GraphicsGroup {
    image_id: u64,
    z_index: i32,
    min_row: usize,
    min_col: usize,
    max_row: usize,
    max_col: usize,
    src_top_left: [f32; 2],
    src_bottom_right: [f32; 2],
}

impl GraphicsCollector {
    #[allow(clippy::too_many_arguments)]
    fn add_cell(
        &mut self,
        hash: [u8; 32],
        placement_id: Option<u32>,
        z_index: i32,
        row: usize,
        col: usize,
        src_top_left: [f32; 2],
        src_bottom_right: [f32; 2],
        decoded: &Arc<TerminalImage>,
    ) {
        let image_id = decoded.id;
        self.images.entry(image_id).or_insert_with(|| decoded.clone());

        match self.index.get(&(hash, placement_id, z_index)) {
            Some(&idx) => {
                let group = &mut self.groups[idx];
                group.min_row = group.min_row.min(row);
                group.min_col = group.min_col.min(col);
                group.max_row = group.max_row.max(row);
                group.max_col = group.max_col.max(col);
                // Cells arrive in row-major order, so the latest is the new
                // bottom-right corner.
                group.src_bottom_right = src_bottom_right;
            }
            None => {
                self.index
                    .insert((hash, placement_id, z_index), self.groups.len());
                self.groups.push(GraphicsGroup {
                    image_id,
                    z_index,
                    min_row: row,
                    min_col: col,
                    max_row: row,
                    max_col: col,
                    src_top_left,
                    src_bottom_right,
                });
            }
        }
    }

    fn finish(self) -> (Vec<GraphicsPlacement>, Vec<TerminalImage>) {
        let placements = self
            .groups
            .into_iter()
            .map(|g| GraphicsPlacement {
                row: g.min_row,
                col: g.min_col,
                cols: g.max_col - g.min_col + 1,
                rows: g.max_row - g.min_row + 1,
                image_id: g.image_id,
                src_top_left: g.src_top_left,
                src_bottom_right: g.src_bottom_right,
                z_index: g.z_index,
            })
            .collect();
        let images = self.images.into_values().map(|a| (*a).clone()).collect();
        (placements, images)
    }
}

/// URL schemes we linkify (kept in sync with the open-side allowlist).
const LINK_SCHEMES: &[&str] = &[
    "https://", "http://", "ftps://", "ftp://", "file://", "mailto:",
];

/// Characters allowed inside a detected URL body (RFC 3986 unreserved + reserved
/// + `%`). Whitespace and most quotes/brackets end the URL.
fn is_url_body_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b':'
                | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'%'
        )
}

/// Trailing characters trimmed off a detected URL (sentence punctuation that is
/// almost never part of the link).
fn is_url_trailing_byte(b: u8) -> bool {
    matches!(b, b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b']' | b'}' | b'\'' | b'"' | b'>')
}

/// Scan `text` for URL byte ranges `[start, end)` beginning with a known scheme.
/// Conservative: a scheme only starts a match at a non-URL boundary, needs at
/// least one body char, and trailing sentence punctuation is trimmed.
fn scan_url_byte_ranges(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let scheme_len = LINK_SCHEMES
            .iter()
            .find(|s| text[i..].starts_with(**s))
            .map(|s| s.len());
        if let Some(scheme_len) = scheme_len {
            let boundary = i == 0 || !is_url_body_byte(bytes[i - 1]);
            if boundary {
                let mut j = i + scheme_len;
                while j < text.len() && is_url_body_byte(bytes[j]) {
                    j += 1;
                }
                if j > i + scheme_len {
                    let mut end = j;
                    while end > i + scheme_len && is_url_trailing_byte(bytes[end - 1]) {
                        end -= 1;
                    }
                    out.push((i, end));
                    i = j;
                    continue;
                }
            }
        }
        i += text[i..].chars().next().map_or(1, |c| c.len_utf8());
    }
    out
}

/// Detect plain-text URLs in one snapshot line, mapped to cell columns.
fn detect_row_links(line: &TerminalLine, row: usize, out: &mut Vec<HyperlinkSpan>) {
    // Row string + per-byte (start col, end col) of the owning cell.
    let mut text = String::new();
    let mut byte_start_col: Vec<usize> = Vec::new();
    let mut byte_end_col: Vec<usize> = Vec::new();
    let mut col = 0usize;
    for cell in &line.cells {
        let end = col + cell.width.max(1);
        for _ in 0..cell.text.len() {
            byte_start_col.push(col);
            byte_end_col.push(end);
        }
        text.push_str(&cell.text);
        col = end;
    }
    for (b_start, b_end) in scan_url_byte_ranges(&text) {
        if b_end == 0 || b_end > byte_start_col.len() {
            continue;
        }
        out.push(HyperlinkSpan {
            row,
            start_col: byte_start_col[b_start],
            end_col: byte_end_col[b_end - 1],
            uri: text[b_start..b_end].to_string(),
        });
    }
}

/// Append auto-detected URL spans, skipping any cell already covered by an
/// explicit OSC 8 span on the same row (OSC 8 wins).
fn detect_plain_links(lines: &[TerminalLine], hyperlinks: &mut Vec<HyperlinkSpan>) {
    let explicit = hyperlinks.len();
    let mut detected: Vec<HyperlinkSpan> = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        detect_row_links(line, row, &mut detected);
    }
    detected.retain(|d| {
        !hyperlinks[..explicit].iter().any(|e| {
            e.row == d.row && d.start_col < e.end_col && e.start_col < d.end_col
        })
    });
    hyperlinks.extend(detected);
}

fn terminal_config(
    palette_defaults: Option<TerminalPaletteDefaults>,
    scrollback_size: usize,
) -> Arc<HecaTerminalConfig> {
    // Engines are created once per pane, so a fresh `Arc` per engine is cheap and
    // keeps the `scrollback_size` override correct (a shared `OnceLock` cache would
    // pin the first-seen size and silently ignore later overrides).
    Arc::new(HecaTerminalConfig {
        palette: match palette_defaults {
            Some(defaults) => palette_with_defaults(defaults),
            None => ColorPalette::default(),
        },
        scrollback_size,
    })
}

fn palette_with_defaults(defaults: TerminalPaletteDefaults) -> ColorPalette {
    let mut palette = ColorPalette::default();
    if let Some(foreground) = defaults.foreground {
        palette.foreground = rgba_u8(foreground);
    }
    if let Some(background) = defaults.background {
        palette.background = rgba_u8(background);
    }
    if let Some(cursor_fg) = defaults.cursor_fg {
        palette.cursor_fg = rgba_u8(cursor_fg);
    }
    if let Some(cursor_bg) = defaults.cursor_bg {
        palette.cursor_bg = rgba_u8(cursor_bg);
    }
    if let Some(cursor_border) = defaults.cursor_border {
        palette.cursor_border = rgba_u8(cursor_border);
    }
    if let Some(selection_fg) = defaults.selection_fg {
        palette.selection_fg = rgba_u8(selection_fg);
    }
    if let Some(selection_bg) = defaults.selection_bg {
        palette.selection_bg = rgba_u8(selection_bg);
    }
    if let Some(ansi) = defaults.ansi {
        for (idx, color) in ansi.into_iter().enumerate() {
            palette.colors.0[idx] = rgba_u8(color);
        }
    }
    if let Some(brights) = defaults.brights {
        for (idx, color) in brights.into_iter().enumerate() {
            palette.colors.0[idx + 8] = rgba_u8(color);
        }
    }
    palette
}

fn rgba_u8(color: [u8; 4]) -> wezterm_term::color::SrgbaTuple {
    wezterm_term::color::SrgbaTuple(
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    )
}

fn map_key_code(code: BackendKeyCode) -> KeyCode {
    match code {
        BackendKeyCode::Char(ch) => KeyCode::Char(ch),
        BackendKeyCode::Enter => KeyCode::Enter,
        BackendKeyCode::Backspace => KeyCode::Backspace,
        BackendKeyCode::Tab => KeyCode::Tab,
        BackendKeyCode::Escape => KeyCode::Escape,
        BackendKeyCode::LeftArrow => KeyCode::LeftArrow,
        BackendKeyCode::RightArrow => KeyCode::RightArrow,
        BackendKeyCode::UpArrow => KeyCode::UpArrow,
        BackendKeyCode::DownArrow => KeyCode::DownArrow,
        BackendKeyCode::Home => KeyCode::Home,
        BackendKeyCode::End => KeyCode::End,
        BackendKeyCode::PageUp => KeyCode::PageUp,
        BackendKeyCode::PageDown => KeyCode::PageDown,
        BackendKeyCode::Insert => KeyCode::Insert,
        BackendKeyCode::Delete => KeyCode::Delete,
        BackendKeyCode::Function(number) => KeyCode::Function(number),
    }
}

fn map_modifiers(modifiers: BackendModifiers) -> KeyModifiers {
    let mut mapped = KeyModifiers::NONE;
    if modifiers.ctrl {
        mapped |= KeyModifiers::CTRL;
    }
    if modifiers.shift {
        mapped |= KeyModifiers::SHIFT;
    }
    if modifiers.alt {
        mapped |= KeyModifiers::ALT;
    }
    if modifiers.super_ {
        mapped |= KeyModifiers::SUPER;
    }
    mapped
}

fn map_mouse_button(button: BackendMouseButton) -> MouseButton {
    match button {
        BackendMouseButton::Left => MouseButton::Left,
        BackendMouseButton::Middle => MouseButton::Middle,
        BackendMouseButton::Right => MouseButton::Right,
        BackendMouseButton::WheelUp(amount) => MouseButton::WheelUp(amount),
        BackendMouseButton::WheelDown(amount) => MouseButton::WheelDown(amount),
        BackendMouseButton::WheelLeft(amount) => MouseButton::WheelLeft(amount),
        BackendMouseButton::WheelRight(amount) => MouseButton::WheelRight(amount),
        BackendMouseButton::None => MouseButton::None,
    }
}

fn map_mouse_event_kind(kind: BackendMouseEventKind) -> MouseEventKind {
    match kind {
        BackendMouseEventKind::Press => MouseEventKind::Press,
        BackendMouseEventKind::Release => MouseEventKind::Release,
        BackendMouseEventKind::Move => MouseEventKind::Move,
    }
}

fn terminal_size(cols: usize, rows: usize, cell_px: (f32, f32)) -> TerminalSize {
    let cols = cols.max(1);
    let rows = rows.max(1);
    // wezterm derives the per-cell pixel size as `pixel_width / cols`, so keep at
    // least one pixel per cell: a zero would mis-size inline images and divide by
    // zero on Sixel attachment.
    let cell_w = cell_px.0.round().max(1.0) as usize;
    let cell_h = cell_px.1.round().max(1.0) as usize;
    TerminalSize {
        cols,
        rows,
        pixel_width: cols * cell_w,
        pixel_height: rows * cell_h,
        dpi: 96,
    }
}

fn blank_cell(palette: &ColorPalette) -> TerminalCell {
    TerminalCell {
        text: " ".to_string(),
        fg: to_rgba(palette.resolve_fg(ColorAttribute::Default)),
        bg: to_rgba(palette.resolve_bg(ColorAttribute::Default)),
        bold: false,
        italic: false,
        underline: TerminalUnderlineStyle::None,
        width: 1,
    }
}

fn blank_with_attrs(cell: &TerminalCell) -> TerminalCell {
    TerminalCell {
        text: " ".to_string(),
        fg: cell.fg,
        bg: cell.bg,
        bold: cell.bold,
        italic: cell.italic,
        underline: TerminalUnderlineStyle::None,
        width: 1,
    }
}

fn terminal_cell(
    text: &str,
    width: usize,
    attrs: &CellAttributes,
    palette: &ColorPalette,
) -> TerminalCell {
    let mut fg = to_rgba(resolve_terminal_fg(attrs, palette));
    let mut bg = to_rgba(palette.resolve_bg(attrs.background()));
    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.invisible() {
        fg[3] = 0.0;
    } else if attrs.intensity() == Intensity::Half {
        fg[0] *= 0.7;
        fg[1] *= 0.7;
        fg[2] *= 0.7;
    }

    TerminalCell {
        text: if text.is_empty() {
            " ".to_string()
        } else {
            text.to_string()
        },
        fg,
        bg,
        bold: attrs.intensity() == Intensity::Bold,
        italic: attrs.italic(),
        underline: map_underline_style(attrs.underline()),
        width: width.max(1),
    }
}

fn map_underline_style(underline: wezterm_term::Underline) -> TerminalUnderlineStyle {
    match underline {
        wezterm_term::Underline::None => TerminalUnderlineStyle::None,
        wezterm_term::Underline::Single => TerminalUnderlineStyle::Single,
        wezterm_term::Underline::Double => TerminalUnderlineStyle::Double,
        wezterm_term::Underline::Curly => TerminalUnderlineStyle::Curly,
        wezterm_term::Underline::Dotted => TerminalUnderlineStyle::Dotted,
        wezterm_term::Underline::Dashed => TerminalUnderlineStyle::Dashed,
    }
}

fn resolve_terminal_fg(
    attrs: &CellAttributes,
    palette: &ColorPalette,
) -> wezterm_term::color::SrgbaTuple {
    match attrs.foreground() {
        ColorAttribute::PaletteIndex(idx) if idx < 8 && attrs.intensity() == Intensity::Bold => {
            palette.resolve_fg(ColorAttribute::PaletteIndex(idx + 8))
        }
        fg => palette.resolve_fg(fg),
    }
}

/// Convert wezterm's straight-alpha SRGBA tuple into the renderer's linear-ish
/// float RGBA array without premultiplying alpha.
fn to_rgba(color: wezterm_term::color::SrgbaTuple) -> [f32; 4] {
    let (r, g, b, a) = color.to_tuple_rgba();
    [r, g, b, a]
}

fn map_cursor_shape(shape: wezterm_surface::CursorShape) -> TerminalCursorShape {
    match shape {
        wezterm_surface::CursorShape::Default => TerminalCursorShape::Default,
        wezterm_surface::CursorShape::BlinkingBlock | wezterm_surface::CursorShape::SteadyBlock => {
            TerminalCursorShape::Block
        }
        wezterm_surface::CursorShape::BlinkingUnderline
        | wezterm_surface::CursorShape::SteadyUnderline => TerminalCursorShape::Underline,
        wezterm_surface::CursorShape::BlinkingBar | wezterm_surface::CursorShape::SteadyBar => {
            TerminalCursorShape::Bar
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TerminalUnderlineStyle;

    struct SinkWriter;

    impl Write for SinkWriter {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    #[test]
    fn snapshot_preserves_background_colored_blank_cells_after_clear() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine =
            TerminalEngine::new(6, 2, (8.0, 16.0), writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
                .expect("engine should initialize");

        engine.advance_bytes(b"\x1b[48;2;30;30;46m\x1b[2J");

        let snapshot = engine.snapshot((8.4, 14.0));
        let expected_bg = [30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0];

        assert!(
            snapshot.lines.iter().all(|line| {
                line.cells.iter().all(|cell| {
                    (cell.bg[0] - expected_bg[0]).abs() < 0.001
                        && (cell.bg[1] - expected_bg[1]).abs() < 0.001
                        && (cell.bg[2] - expected_bg[2]).abs() < 0.001
                        && (cell.bg[3] - expected_bg[3]).abs() < 0.001
                })
            }),
            "clear-screen background should be preserved across blank cells"
        );
    }

    #[test]
    fn bell_alert_is_captured_once() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(80, 24, (8.0, 16.0), writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
            .expect("terminal engine should initialize");

        engine.advance_bytes(b"\x07");
        assert_eq!(engine.take_alerts(), vec![BackendAlert::Bell]);
        assert!(engine.take_alerts().is_empty());
    }

    #[test]
    fn captures_osc52_clipboard_write_once() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(80, 24, (8.0, 16.0), writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
            .expect("terminal engine should initialize");

        // OSC 52: set clipboard `c` to base64("hello"). wezterm decodes it for us.
        engine.advance_bytes(b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(engine.take_clipboard_writes(), vec!["hello".to_string()]);
        // Drained on read.
        assert!(engine.take_clipboard_writes().is_empty());
    }

    #[test]
    fn bracketed_paste_mode_tracks_decset_2004() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(80, 24, (8.0, 16.0), writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
            .expect("terminal engine should initialize");

        assert!(!engine.bracketed_paste_enabled(), "off by default");
        engine.advance_bytes(b"\x1b[?2004h");
        assert!(engine.bracketed_paste_enabled(), "DECSET 2004 enables it");
        engine.advance_bytes(b"\x1b[?2004l");
        assert!(!engine.bracketed_paste_enabled(), "DECRST 2004 disables it");
    }

    #[test]
    fn search_scrollback_matches_case_insensitively_with_columns() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(
            80,
            24,
            (8.0, 16.0),
            writer,
            None,
            crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
        )
        .expect("terminal engine should initialize");
        engine.advance_bytes(b"Hello World\r\nfoo BAR baz\r\n");

        // Case-insensitive, with inclusive start / exclusive end columns.
        let world = engine.search_scrollback("world", 80);
        assert_eq!(world.len(), 1);
        assert_eq!((world[0].start_col, world[0].end_col), (6, 11));

        // Lowercase query matches uppercase content.
        let bar = engine.search_scrollback("bar", 80);
        assert_eq!(bar.len(), 1);
        assert_eq!((bar[0].start_col, bar[0].end_col), (4, 7));

        // No match, and an empty query, both yield nothing.
        assert!(engine.search_scrollback("zzz", 80).is_empty());
        assert!(engine.search_scrollback("", 80).is_empty());
    }

    #[test]
    fn scan_url_byte_ranges_detects_and_trims() {
        let text = "see https://example.com/p?q=1, and http://a.b please";
        let ranges = super::scan_url_byte_ranges(text);
        let urls: Vec<&str> = ranges.iter().map(|&(s, e)| &text[s..e]).collect();
        assert_eq!(urls, vec!["https://example.com/p?q=1", "http://a.b"]);
    }

    #[test]
    fn scan_url_byte_ranges_ignores_plain_text_and_bare_scheme() {
        assert!(super::scan_url_byte_ranges("just some words example.com").is_empty());
        assert!(super::scan_url_byte_ranges("https://").is_empty());
    }

    #[test]
    fn snapshot_auto_detects_plain_urls() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(
            40,
            2,
            (8.0, 16.0),
            writer,
            None,
            crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
        )
        .expect("engine init");
        engine.advance_bytes(b"go https://example.com now");
        let snap = engine.snapshot((8.0, 16.0));
        assert_eq!(snap.hyperlinks.len(), 1);
        assert_eq!(snap.hyperlinks[0].uri, "https://example.com");
        assert_eq!(snap.hyperlinks[0].row, 0);
        assert_eq!(snap.hyperlinks[0].start_col, 3);

        // Toggling detection off clears the auto-detected link.
        engine.set_link_detection(false);
        assert!(engine.snapshot((8.0, 16.0)).hyperlinks.is_empty());
    }

    #[test]
    fn snapshot_captures_osc8_hyperlink_spans() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(
            80,
            4,
            (8.0, 16.0),
            writer,
            None,
            crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
        )
        .expect("terminal engine should initialize");

        // OSC 8 hyperlink: open `https://example.com`, write "link", close.
        engine.advance_bytes(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");

        let snapshot = engine.snapshot((8.0, 16.0));
        assert_eq!(snapshot.hyperlinks.len(), 1, "one captured link span");
        let span = &snapshot.hyperlinks[0];
        assert_eq!(span.uri, "https://example.com");
        assert_eq!(span.row, 0);
        assert_eq!(span.start_col, 0);
        assert_eq!(span.end_col, 4, "the 4 cells of \"link\" form the span");
        assert!(
            snapshot.graphics.is_empty() && snapshot.images.is_empty(),
            "plain text emits no inline-image placements"
        );
    }

    #[test]
    fn captures_sixel_inline_image_placement() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(
            20,
            4,
            (8.0, 16.0),
            writer,
            None,
            crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
        )
        .expect("terminal engine should initialize");

        // Minimal Sixel: register color 0 = red, then paint a 16px-wide, 6px-tall
        // band (`!16~` = repeat the all-6-pixels-on column 16 times). Sixel decodes
        // to RGBA with no base64, so this exercises the full capture path.
        engine.advance_bytes(b"\x1bPq#0;2;100;0;0#0!16~\x1b\\");

        let snapshot = engine.snapshot((8.0, 16.0));
        assert_eq!(
            snapshot.graphics.len(),
            1,
            "one inline-image placement captured"
        );
        let placement = &snapshot.graphics[0];
        // 16px / 8px cells = 2 cells wide; 6px / 16px cells = 1 cell tall.
        assert_eq!(placement.cols, 2, "16px image spans 2 cells of 8px");
        assert_eq!(placement.rows, 1, "6px image fits in 1 cell of 16px");
        assert_eq!(placement.row, 0);
        assert_eq!(placement.col, 0);

        // The placement resolves to a decoded image in the registry.
        assert_eq!(snapshot.images.len(), 1, "one decoded image registered");
        let image = &snapshot.images[0];
        assert_eq!(image.id, placement.image_id, "placement references the image");
        assert!(image.width >= 16 && image.height >= 6, "decoded at source size");
        assert_eq!(
            image.rgba.len(),
            (image.width as usize) * (image.height as usize) * 4,
            "RGBA buffer is tightly packed"
        );
    }

    #[test]
    fn decode_terminal_image_handles_rgba_and_rejects_garbage() {
        use wezterm_term::image::ImageDataType;
        // A 2x1 opaque-red RGBA image is used as-is.
        let rgba = vec![255, 0, 0, 255, 255, 0, 0, 255];
        let data = ImageDataType::new_single_frame(2, 1, rgba.clone());
        let hash = [7u8; 32];
        let decoded = decode_terminal_image(hash, &data).expect("rgba decodes");
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert_eq!(&*decoded.rgba, rgba.as_slice());
        assert_eq!(decoded.id, image_id_from_hash(hash));

        // Undecodable encoded bytes yield None rather than panicking.
        let junk = ImageDataType::EncodedFile(vec![0xde, 0xad, 0xbe, 0xef]);
        assert!(decode_terminal_image([1u8; 32], &junk).is_none());
    }

    #[test]
    fn graphics_collector_coalesces_block_and_dedups_images() {
        let image = Arc::new(TerminalImage {
            id: 42,
            width: 4,
            height: 2,
            rgba: Arc::from(vec![0u8; 4 * 2 * 4].into_boxed_slice()),
        });
        let hash = [9u8; 32];
        let mut collector = GraphicsCollector::default();
        // A 2x2 cell block, fed in row-major order. Corner texcoords come from the
        // first (top-left) and last (bottom-right) cells.
        collector.add_cell(hash, Some(1), 0, 0, 0, [0.0, 0.0], [0.5, 0.5], &image);
        collector.add_cell(hash, Some(1), 0, 0, 1, [0.5, 0.0], [1.0, 0.5], &image);
        collector.add_cell(hash, Some(1), 0, 1, 0, [0.0, 0.5], [0.5, 1.0], &image);
        collector.add_cell(hash, Some(1), 0, 1, 1, [0.5, 0.5], [1.0, 1.0], &image);

        let (placements, images) = collector.finish();
        assert_eq!(placements.len(), 1, "the 4 cells coalesce into one block");
        let p = &placements[0];
        assert_eq!((p.row, p.col, p.cols, p.rows), (0, 0, 2, 2));
        assert_eq!(p.image_id, 42);
        assert_eq!(p.src_top_left, [0.0, 0.0], "top-left from the first cell");
        assert_eq!(p.src_bottom_right, [1.0, 1.0], "bottom-right from the last cell");
        assert_eq!(images.len(), 1, "the shared image is registered once");
    }

    #[test]
    fn map_underline_style_preserves_all_wezterm_variants() {
        assert_eq!(
            map_underline_style(wezterm_term::Underline::None),
            TerminalUnderlineStyle::None
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Single),
            TerminalUnderlineStyle::Single
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Double),
            TerminalUnderlineStyle::Double
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Curly),
            TerminalUnderlineStyle::Curly
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Dotted),
            TerminalUnderlineStyle::Dotted
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Dashed),
            TerminalUnderlineStyle::Dashed
        );
    }

    /// Build an engine with enough scrollback to make viewport motion observable.
    fn viewport_engine(cols: usize, rows: usize, scrollback_size: usize) -> TerminalEngine {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        TerminalEngine::new(cols, rows, (8.0, 16.0), writer, None, scrollback_size)
            .expect("viewport test engine should initialize")
    }

    /// Fill the scrollback with `count` distinct printable lines so the viewport
    /// has real history to scroll into. Each line is `cols` cells wide.
    fn fill_scrollback(engine: &mut TerminalEngine, cols: usize, count: usize) {
        for i in 0..count {
            // Print a marker followed by spaces, then a newline so each line lands
            // in scrollback as the next output row scrolls up.
            let marker = format!("L{i:03}");
            let mut line = marker.into_bytes();
            line.truncate(cols);
            while line.len() < cols {
                line.push(b' ');
            }
            engine.advance_bytes(&line);
            engine.advance_bytes(b"\r\n");
        }
    }

    /// Read the current max valid viewport offset directly from the engine's own
    /// clamp logic (`scrollback_rows - visible_rows`). Deriving the expected
    /// boundary from the live engine avoids hardcoding fragile row counts (the
    /// exact retained-row total depends on wezterm's scroll timing).
    fn max_offset(engine: &TerminalEngine) -> usize {
        engine.max_viewport_offset()
    }

    #[test]
    fn viewport_starts_pinned_to_bottom_with_zero_offset() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);

        assert_eq!(engine.viewport_offset(), 0, "fresh engine pins to live bottom");
        assert!(engine.at_bottom(), "at_bottom is true at offset 0");
        assert!(!engine.take_viewport_changed(), "no motion yet ⇒ no viewport damage");

        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.viewport_offset, 0);
        assert!(snapshot.at_bottom);
        assert!(
            snapshot.scrollback_rows > 4,
            "scrollback_rows should retain history above the visible rows"
        );
        assert_eq!(
            snapshot.scrollback_rows.saturating_sub(4),
            max_offset(&engine),
            "snapshot scrollback_rows minus visible equals the max offset"
        );
    }

    #[test]
    fn scroll_viewport_clamps_to_max_offset_and_sets_damage_flag() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let max_offset = max_offset(&engine);
        assert!(max_offset > 0, "fixture should produce some scrollback history");

        engine.scroll_viewport(3);
        assert_eq!(engine.viewport_offset(), 3);
        assert!(!engine.at_bottom());
        assert!(engine.take_viewport_changed(), "motion sets the viewport-changed flag");
        assert!(!engine.take_viewport_changed(), "flag drains once");

        // Overscroll clamps to max_offset.
        engine.scroll_viewport(100);
        assert_eq!(engine.viewport_offset(), max_offset);
        assert!(engine.take_viewport_changed());

        // Scrolling back down clamps at bottom (offset 0).
        engine.scroll_viewport(-100);
        assert_eq!(engine.viewport_offset(), 0);
        assert!(engine.at_bottom());
        assert!(engine.take_viewport_changed());

        // No-op scrolls (already at boundary) do not arm damage.
        engine.scroll_viewport(-1);
        assert_eq!(engine.viewport_offset(), 0);
        assert!(!engine.take_viewport_changed(), "clamped no-op must not arm damage");
    }

    #[test]
    fn scroll_to_top_and_bottom_jump_the_viewport() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let max_offset = max_offset(&engine);
        assert!(max_offset > 0);

        engine.scroll_to_top();
        assert_eq!(engine.viewport_offset(), max_offset);
        assert!(engine.take_viewport_changed());

        engine.scroll_to_top();
        assert!(!engine.take_viewport_changed(), "already-at-top no-op must not arm damage");

        engine.scroll_to_bottom();
        assert_eq!(engine.viewport_offset(), 0);
        assert!(engine.at_bottom());
        assert!(engine.take_viewport_changed());

        engine.scroll_to_bottom();
        assert!(!engine.take_viewport_changed(), "already-at-bottom no-op must not arm damage");
    }

    #[test]
    fn animated_scroll_settles_at_target() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 20);
        let max = max_offset(&engine);
        let target = 5.min(max);
        assert!(target > 0, "fixture must have enough scrollback");
        // 0ms duration ⇒ animation completes on the first advance (done path),
        // with no real-time sleep.
        engine.set_viewport_anim_config_for_test(AnimationConfig {
            duration_ms: 0,
            easing: crate::layout::animation::linear,
        });

        engine.scroll_viewport_animated(target as i32);
        // Single advance settles: the animation reports done and snaps to target.
        assert!(!engine.advance_animation(), "0ms animation completes on first advance");
        assert_eq!(engine.viewport_offset(), target, "offset settles at target");
        assert!(!engine.advance_animation(), "no animation left after settle");
    }

    #[test]
    fn animated_scroll_re_targets_on_new_jump_without_queueing() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 20);
        let max = max_offset(&engine);
        let expected = 10.min(max);
        assert!(expected >= 5, "fixture must have enough scrollback");
        engine.set_viewport_anim_config_for_test(AnimationConfig {
            duration_ms: 0,
            easing: crate::layout::animation::linear,
        });

        // Jump up 5, then immediately jump up 5 more → should target 10 (re-target,
        // not 5). The second call bases its delta on the first animation's target.
        engine.scroll_viewport_animated(5);
        engine.scroll_viewport_animated(5);
        assert!(!engine.advance_animation(), "0ms animation completes on first advance");
        assert_eq!(engine.viewport_offset(), expected, "re-targeted jump lands at 10, not 5");
    }

    #[test]
    fn disabled_animation_degrades_to_immediate_scroll() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 20);
        let max = max_offset(&engine);
        let target = 5.min(max);
        assert!(target > 0, "fixture must have enough scrollback");
        engine.set_scroll_animations_enabled(false);

        engine.scroll_viewport_animated(target as i32);
        assert_eq!(engine.viewport_offset(), target, "disabled animation jumps immediately");
        assert!(!engine.advance_animation(), "no animation left when disabled");
    }

    #[test]
    fn wheel_scroll_does_not_animate_and_clears_ongoing_animation() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 20);
        let max = max_offset(&engine);
        assert!(max >= 10, "fixture must have enough scrollback");
        // A long duration keeps the animation "ongoing" without any sleep, so we
        // can observe the wheel clearing it.
        engine.set_viewport_anim_config_for_test(AnimationConfig {
            duration_ms: 60_000,
            easing: crate::layout::animation::linear,
        });

        // Start an animated jump; it is still running (long duration).
        engine.scroll_viewport_animated(10);
        assert!(engine.advance_animation(), "long animation is still running");
        // Wheel scroll (immediate path) clears the animation and applies at once.
        engine.scroll_viewport(3);
        assert_eq!(engine.viewport_offset(), 3, "wheel applied immediately");
        assert!(!engine.advance_animation(), "wheel cleared the animation");
    }

    #[test]
    fn snapshot_projects_history_rows_when_viewport_is_scrolled() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        // Scrolling up 4 rows should reveal history rows instead of the live bottom.
        engine.scroll_viewport(4);
        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.viewport_offset, 4);
        assert!(!snapshot.at_bottom);
        assert_eq!(snapshot.lines.len(), 4, "snapshot still reports exactly `rows` lines");
        snapshot.debug_assert_valid();
    }

    #[test]
    fn resize_re_clamps_viewport_offset_when_scrollback_shrinks() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let total = engine.snapshot((8.0, 14.0)).scrollback_rows;
        engine.scroll_to_top();
        assert_eq!(engine.viewport_offset(), total.saturating_sub(4));
        let _ = engine.take_viewport_changed();

        // Grow visible rows so max_offset shrinks; the stored offset must clamp.
        engine.resize(20, 8);
        assert_eq!(
            engine.viewport_offset(),
            total.saturating_sub(8),
            "resize must re-clamp offset to new max (total - visible 8)"
        );
        assert!(engine.take_viewport_changed(), "re-clamp on resize arms damage");
    }

    #[test]
    fn scrollback_size_override_replaces_default_capacity() {
        // Drive the engine with a non-default scrollback size and confirm it is
        // plumbed through `HecaTerminalConfig` by filling past the wezterm default
        // (3500) up to the override. Total retained rows should be bounded by the
        // formula `scrollback_size + visible_rows` (the override caps history).
        let scrollback_size = 50;
        let rows = 2;
        let mut engine = viewport_engine(10, rows, scrollback_size);
        fill_scrollback(&mut engine, 10, 80);
        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.rows, rows);
        assert_eq!(snapshot.lines.len(), rows);
        assert!(
            snapshot.scrollback_rows <= scrollback_size + rows,
            "scrollback_size override should cap retained history at scrollback_size + visible_rows"
        );
    }

    #[test]
    fn reconcile_viewport_offset_snaps_to_bottom_when_scrollback_shrinks() {
        // The drift fix targets the case where scrollback shrinks without a resize
        // (alt-screen entry / clear dropping `scrollback_rows`), leaving the stored
        // offset pointing past the new top. `scroll_viewport`/`scroll_to_top`/resize
        // all clamp on write, so the only way to construct a stale stored offset is
        // to set it directly — this exercises `reconcile_viewport_offset` as a unit,
        // decoupled from wezterm escape-sequence semantics.
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let top = max_offset(&engine);
        assert!(top > 0, "fixture should produce scrollback history");

        // Force the stored offset past the boundary (simulating a shrink that
        // hasn't been reconciled yet).
        engine.set_viewport_offset_for_test(top + 5);
        assert_eq!(engine.viewport_offset(), top + 5);
        let _ = engine.take_viewport_changed();

        let changed = engine.reconcile_viewport_offset();
        assert_eq!(
            engine.viewport_offset(),
            top,
            "reconcile must write the stored offset back to the current max"
        );
        assert!(
            changed,
            "reconcile must report a correction when the stored offset was stale"
        );
        assert!(engine.take_viewport_changed(), "correction arms viewport damage");

        // A second reconcile on already-consistent state is a no-op.
        assert!(!engine.reconcile_viewport_offset());
        assert!(!engine.take_viewport_changed());

        // When scrollback has fully collapsed (max_offset == 0), reconcile snaps to
        // the live bottom — the alt-screen case.
        engine.set_viewport_offset_for_test(3);
        // Collapse max_offset to 0 by clearing scrollback via the alt screen.
        engine.advance_bytes(b"\x1b[?1049h");
        let collapsed = max_offset(&engine);
        if collapsed == 0 {
            assert!(engine.reconcile_viewport_offset(), "alt-screen shrink must correct");
            assert_eq!(engine.viewport_offset(), 0, "reconcile snaps to bottom on alt screen");
            assert!(engine.take_viewport_changed());
        }
        // Leave the alt screen so the shared default-config cache / other tests are unaffected.
        engine.advance_bytes(b"\x1b[?1049l");
    }
}
