//! **Moving through history** — the window onto the scrollback: where it is, how far it can go,
//! and the animated jumps.

use super::*;

impl TerminalEngine {
    /// Current host viewport offset in rows above the live bottom.
    ///
    /// Test-only inspection helper; production reads the viewport from the
    /// `TerminalSnapshot` (the rendering source of truth per Q1).
    #[cfg(test)]
    pub(in crate::backend::terminal) fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    pub(in crate::backend::terminal) fn at_bottom(&self) -> bool {
        self.viewport_offset == 0
    }

    /// Whether the hosted application has enabled terminal mouse reporting modes
    /// (X10/SGR/any-event). When `true`, wheel and click events should be forwarded
    /// to the terminal instead of being handled by heca's host chrome.
    pub(in crate::backend::terminal) fn is_mouse_grabbed(&self) -> bool {
        self.terminal.is_mouse_grabbed()
    }

    /// Test-only setter that bypasses the clamp, used to construct a stale stored
    /// offset for `reconcile_viewport_offset` unit tests (the write paths all clamp
    /// on entry, so stale state can only arise from a scrollback shrink).
    #[cfg(test)]
    pub(in crate::backend::terminal) fn set_viewport_offset_for_test(&mut self, offset: usize) {
        self.viewport_offset = offset;
    }

    /// Enable or disable backend-side viewport animations.
    pub(in crate::backend::terminal) fn set_scroll_animations_enabled(&mut self, enabled: bool) {
        self.viewport_anim_enabled = enabled;
        if !enabled {
            self.viewport_anim = None;
        }
    }

    /// Reload the terminal emulation config (palette + scrollback size) live.
    /// Used by `prefix+Shift+r` so theme/config changes apply to existing PTYs.
    pub(in crate::backend::terminal) fn reload_config(
        &mut self,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) {
        self.scrollback_size
            .store(scrollback_size, Ordering::Relaxed);
        let config = terminal_config(palette_defaults, &self.scrollback_size);
        // Force the freshly-configured palette to win. `set_config` only swaps the config
        // Arc — wezterm keeps any palette a program forked via dynamic-color escapes (OSC
        // 4/10/11, which nvim and many shell prompts emit), and `palette()` (read by the
        // snapshot) returns that fork over the config. So without this, a theme reload
        // (`prefix+Shift+r`) appears to do nothing for panes that touched colors. Overwrite
        // the fork with the new config palette; programs reassert their own colors on their
        // next redraw. (`implicit_palette_reset_if_same_as_configured` is insufficient — it
        // only resets when the fork already equals the config, i.e. never after a theme change.)
        let palette = config.color_palette();
        self.terminal.set_config(config);
        *self.terminal.palette_mut() = palette;
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
    pub(in crate::backend::terminal) fn set_viewport_anim_config_for_test(
        &mut self,
        config: AnimationConfig,
    ) {
        self.viewport_anim_config = config;
    }

    /// Maximum valid viewport offset = total retained rows minus the visible
    /// row count. Returns `0` when there is no scrollback yet.
    pub(super) fn max_viewport_offset(&self) -> usize {
        let screen = self.terminal.screen();
        let visible_count = self.rows.min(screen.physical_rows.max(1));
        screen.scrollback_rows().saturating_sub(visible_count)
    }

    /// Scroll the host viewport by `delta_rows`.
    ///
    /// Positive values move toward history (offset increases); negative values
    /// move toward the live bottom (offset decreases). The offset is clamped to
    /// `[0, max_viewport_offset()]`. No-op if the clamped value does not change.
    pub(in crate::backend::terminal) fn scroll_viewport(&mut self, delta_rows: i32) {
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
    pub(in crate::backend::terminal) fn scroll_to_top(&mut self) {
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset != max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    /// Snap the viewport to the live bottom (`viewport_offset = 0`).
    pub(in crate::backend::terminal) fn scroll_to_bottom(&mut self) {
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
    pub(in crate::backend::terminal) fn advance_animation(&mut self) -> bool {
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
    pub(in crate::backend::terminal) fn scroll_viewport_animated(&mut self, delta_rows: i32) {
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
    pub(in crate::backend::terminal) fn scroll_to_top_animated(&mut self) {
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
    pub(in crate::backend::terminal) fn scroll_to_bottom_animated(&mut self) {
        if !self.viewport_anim_enabled {
            self.scroll_to_bottom();
            return;
        }
        let current = self
            .viewport_anim
            .as_ref()
            .map_or(self.viewport_offset as f64, |a| a.value());
        if (current.round() as usize) != 0 {
            self.viewport_anim = Some(Animation::new(current, 0.0, self.viewport_anim_config));
        }
    }

    /// Drain the viewport-changed flag. Returns `true` when the viewport moved
    /// since the last call, which the host must treat as `TerminalDamage::Full`
    /// for the retained terminal layer (Q6).
    pub(in crate::backend::terminal) fn take_viewport_changed(&mut self) -> bool {
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
    /// Called at the end of [`update`](crate::backend::PaneBackend::update) (right after PTY output is
    /// drained, where alt-screen/clear transitions happen), so the stored offset is
    /// consistent before any downstream `snapshot()` read.
    pub(in crate::backend::terminal) fn reconcile_viewport_offset(&mut self) -> bool {
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
}
