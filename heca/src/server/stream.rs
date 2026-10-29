//! Process requests and plain snapshots, independent of any client's view.

use crate::app::backend_store::TerminalOwner;
use heca_core::backend::{TerminalDamage, TerminalSnapshot};
use super::{ServerState, wire::terminal::{ProcessInput, TerminalFrame}};

pub(super) fn valid_grid(cols: usize, rows: usize, cell: (f32, f32), scale: f32, limit: usize) -> bool {
    cols > 0 && rows > 0 && cols.checked_mul(rows).is_some_and(|cells| cells <= limit)
        && [cell.0, cell.1, scale, cell.0 * scale, cell.1 * scale].iter().all(|value| value.is_finite() && *value > 0.0)
}

impl ServerState {
    pub(super) fn process_input(&mut self, _client_id: u64, owner: &TerminalOwner, input: ProcessInput) {
        let Some(id) = self.backends.find_owner(owner) else { return };
        let Some(backend) = self.backends.get_mut_by_id(id) else { return };
        match input {
            ProcessInput::Bytes(bytes) => backend.process_input(&bytes),
            ProcessInput::Key(key) => { backend.process_key_event(&key); },
            ProcessInput::Mouse(mouse) => { backend.process_mouse_event(&mouse); },
            ProcessInput::Paste(text) => backend.paste(&text),
            ProcessInput::Focus(focused) => backend.focus_changed(focused),
            ProcessInput::Resize { cols, rows, cell, scale } => {
                backend.set_cell_size(cell.0, cell.1);
                backend.set_scale_factor(scale);
                backend.set_size(cols, rows);
            }
        }
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
