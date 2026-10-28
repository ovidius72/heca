//! Process requests and plain snapshots, independent of any client's view.

use crate::app::backend_store::TerminalOwner;
use heca_core::backend::{TerminalDamage, TerminalSnapshot};
use super::sizing::Grid;
use super::{ServerState, wire::terminal::{ProcessInput, TerminalFrame}};

pub(super) fn valid_grid(cols: usize, rows: usize, cell: (f32, f32), scale: f32, limit: usize) -> bool {
    cols > 0 && rows > 0 && cols.checked_mul(rows).is_some_and(|cells| cells <= limit)
        && [cell.0, cell.1, scale, cell.0 * scale, cell.1 * scale].iter().all(|value| value.is_finite() && *value > 0.0)
}

impl ServerState {
    /// Input from the window identified by `client`. Typing and resizing make that window the active
    /// one and drive pane-size arbitration ([`sizing`](super::sizing)); a resize never goes straight
    /// to the process — the `pane_size` setting decides the size among every window showing the pane.
    pub(super) fn process_input(&mut self, owner: &TerminalOwner, client: u64, input: ProcessInput) {
        let Some(id) = self.backends.find_owner(owner) else { return };
        match input {
            ProcessInput::Bytes(bytes) => {
                if let Some(backend) = self.backends.get_mut_by_id(id) { backend.process_input(&bytes); }
                self.after_typing(owner, client);
            }
            ProcessInput::Key(key) => {
                if let Some(backend) = self.backends.get_mut_by_id(id) { backend.process_key_event(&key); }
                self.after_typing(owner, client);
            }
            ProcessInput::Paste(text) => {
                if let Some(backend) = self.backends.get_mut_by_id(id) { backend.paste(&text); }
                self.after_typing(owner, client);
            }
            ProcessInput::Mouse(mouse) => {
                if let Some(backend) = self.backends.get_mut_by_id(id) { backend.process_mouse_event(&mouse); }
            }
            ProcessInput::Focus(focused) => {
                if let Some(backend) = self.backends.get_mut_by_id(id) { backend.focus_changed(focused); }
            }
            ProcessInput::Resize { cols, rows, cell, scale } => {
                self.sizing.request(owner, client, Grid { cols, rows, cell, scale });
                self.sizing.touch(owner, client);
                self.apply_pane_size(owner);
            }
        }
    }

    /// Typing in a window makes it the active one; under `latest` the shared process follows it, so
    /// the window you work in gets the full size.
    fn after_typing(&mut self, owner: &TerminalOwner, client: u64) {
        self.sizing.touch(owner, client);
        self.apply_pane_size(owner);
    }

    /// Resize the one process to the grid the `pane_size` policy picks among the windows showing it,
    /// but only when that grid actually moved.
    fn apply_pane_size(&mut self, owner: &TerminalOwner) {
        let Some(grid) = self.sizing.resolve(owner, self.terminal_settings.pane_size) else { return };
        if !self.sizing.mark_applied(owner, grid) { return; }
        let Some(id) = self.backends.find_owner(owner) else { return };
        let Some(backend) = self.backends.get_mut_by_id(id) else { return };
        backend.set_cell_size(grid.cell.0, grid.cell.1);
        backend.set_scale_factor(grid.scale);
        backend.set_size(grid.cols, grid.rows);
    }

    /// A window closed: forget its claims on every terminal and re-apply the ones it was holding, so
    /// a pane kept small (or driven) by that window returns to the remaining windows' size.
    pub(super) fn client_gone(&mut self, client: u64) {
        for owner in self.sizing.forget_client(client) {
            self.apply_pane_size(&owner);
        }
    }

    /// A terminal ended: forget its sizing so a later terminal reusing the owner starts clean.
    pub(super) fn terminal_gone(&mut self, owner: &TerminalOwner) {
        self.sizing.forget_terminal(owner);
    }

    pub(super) fn viewport(&self, owner: &TerminalOwner, offset: usize) -> Option<(TerminalFrame, TerminalSnapshot)> {
        let id = self.backends.find_owner(owner)?;
        let backend = self.backends.get_by_id(id)?;
        let screen = backend.terminal_snapshot_at(offset)?;
        let frame = TerminalFrame {
            id: id.0, owner: owner.clone(), title: backend.title().into(),
            spec: self.backends.spec_of(id)?.clone(),
            screen: screen.to_wire(), damage: TerminalDamage::Full,
            runtime: backend.runtime(), mouse_grabbed: backend.is_mouse_grabbed(),
        };
        Some((frame, screen))
    }
}
