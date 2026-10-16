//! **Telling the program its size** — in cells and in pixels — at a paced rate, so dragging a
//! divider does not signal the shell at frame rate.

// Named only in the docs below.
#[cfg(doc)]
use crate::backend::PaneBackend;
use super::TerminalBackend;
use std::time::Instant;

/// **How often the child may be told the terminal changed size.**
///
/// Every `TIOCSWINSZ` makes the kernel send `SIGWINCH` to the child's process group, and a shell's
/// line editor redraws its prompt on each one. Dragging a pane divider changes the row count on
/// **every frame**, so an un-paced resize signals the shell at frame rate — and zsh dies in that
/// storm with `zsh: error on TTY read: no such process`, one of its own TTY reads having failed
/// while it was busy redrawing. heca then closes the pane, because its child really did exit.
///
/// The **terminal model** still resizes immediately, so the pane reflows as you drag; only the
/// signal to the child is paced. The last size is always delivered once the gesture settles
/// ([`flush_pending_pty_size`](TerminalBackend::flush_pending_pty_size) runs on every wake), so the
/// child never ends up disagreeing with what is on screen.
pub(super) const PTY_RESIZE_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

impl TerminalBackend {
    /// Physical cell pixel size `(cell × scale)` reported to the emulation model
    /// and the PTY, so inline images render at the real on-screen resolution.
    pub(super) fn physical_cell_px(&self) -> (f32, f32) {
        (self.cell_w * self.cell_scale, self.cell_h * self.cell_scale)
    }

    /// Push the current physical cell pixel size to both reporters: the wezterm
    /// model (image sizing + `CSI 14/16 t` answers) and the PTY winsize
    /// (`TIOCGWINSZ`, read by image tools like Yazi / kitten).
    pub(super) fn push_pixel_metrics(&mut self) {
        let px = self.physical_cell_px();
        self.engine.set_cell_px(px);
        // Paced like the grid size beside it — this is the same `TIOCSWINSZ`, and the same
        // `SIGWINCH` to the child (see `PTY_RESIZE_MIN_INTERVAL`).
        self.pending_pty_size = Some((self.cols, self.rows));
        self.flush_pending_pty_size();
    }

    /// Tell the child about a size change it has not been told about yet, if enough time has passed
    /// since the last one. Called from [`set_size`](PaneBackend::set_size) and from every `update`, so a size
    /// that was held back mid-gesture is always delivered once the gesture settles.
    pub(super) fn flush_pending_pty_size(&mut self) {
        let Some((cols, rows)) = self.pending_pty_size else {
            return;
        };
        if self.last_pty_resize.elapsed() < PTY_RESIZE_MIN_INTERVAL {
            return;
        }
        self.pending_pty_size = None;
        self.last_pty_resize = Instant::now();
        if self
            .pty
            .resize(cols, rows, self.physical_cell_px())
            .is_err()
        {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to resize PTY to {}x{}; terminal model resized anyway",
                cols, rows
            );
        }
    }
}
