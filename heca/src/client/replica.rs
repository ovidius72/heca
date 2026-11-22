//! Data received from the server. No PTY, emulator, timers or window state.
//!
//! A received frame is shared, never copied: every view of a terminal holds the same one, so a
//! window with many busy terminals does not copy each screen at every wake.

use crate::{app::backend_store::TerminalOwner, server::wire::terminal::TerminalFrame};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(super) struct Replica {
    pub(super) terminals: HashMap<(TerminalOwner, usize), (u64, Arc<TerminalFrame>)>,
    sequence: u64,
    pub(super) images: HashMap<u64, heca_core::backend::TerminalImage>,
}

pub(super) type SharedReplica = Arc<Mutex<Replica>>;

impl Replica {
    pub(super) fn receive(&mut self, frame: Arc<TerminalFrame>) {
        self.sequence = self.sequence.wrapping_add(1);
        if frame.screen.viewport_offset != 0 {
            self.terminals
                .retain(|(owner, offset), _| owner != &frame.owner || *offset == 0);
        }
        self.terminals.insert(
            (frame.owner.clone(), frame.screen.viewport_offset),
            (self.sequence, frame),
        );
    }

    pub(super) fn receive_image(&mut self, image: heca_core::backend::TerminalImage) {
        let id = image.id;
        self.images.insert(id, image);
        self.sequence = self.sequence.wrapping_add(1);
        for (version, frame) in self.terminals.values_mut() {
            if frame
                .screen
                .graphics
                .iter()
                .any(|placement| placement.image_id == id)
            {
                *version = self.sequence;
            }
        }
    }

    pub(super) fn frame(
        &self,
        owner: &TerminalOwner,
        offset: usize,
    ) -> Option<&(u64, Arc<TerminalFrame>)> {
        self.terminals
            .get(&(owner.clone(), offset))
            .or_else(|| self.terminals.get(&(owner.clone(), 0)))
    }

    pub(super) fn exact_frame(
        &self,
        owner: &TerminalOwner,
        offset: usize,
    ) -> Option<&(u64, Arc<TerminalFrame>)> {
        self.terminals.get(&(owner.clone(), offset))
    }
}
