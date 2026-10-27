//! A terminal view over received data. Every process mutation becomes a typed request.

use heca_core::backend::{BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType, TerminalDamage, TerminalSnapshot};
use crate::{app::backend_store::{TerminalOwner, TerminalSpec}, server::wire::{ClientMsg, terminal::ProcessInput}};
use super::{outbox::Outbox, replica::SharedReplica};

pub(super) struct TerminalView {
    owner: TerminalOwner,
    data: SharedReplica,
    outbox: Outbox,
    title: String,
    offset: usize,
    sequence: u64,
    painted: u64,
    dirty: bool,
    grid: (usize, usize),
    cell: (f32, f32),
    scale: f32,
    last_agent: Option<heca_core::agent::AgentState>,
    live_sequence: u64,
    live_top: Option<isize>,
    visible: Option<crate::server::wire::terminal::TerminalFrame>,
}

impl TerminalView {
    pub(super) fn new(owner: TerminalOwner, spec: &TerminalSpec, data: SharedReplica, outbox: Outbox, cell: (f32, f32)) -> Self {
        Self { owner, data, outbox, title: String::new(), offset: 0, sequence: 0, painted: 0,
            dirty: true, grid: spec.grid, cell, scale: 1.0, last_agent: None,
            live_sequence: 0, live_top: None, visible: None }
    }

    fn tell(&self, input: ProcessInput) {
        self.outbox.send(ClientMsg::Terminal { owner: self.owner.clone(), input });
    }

    fn resize(&self) {
        self.tell(ProcessInput::Resize { cols: self.grid.0, rows: self.grid.1, cell: self.cell, scale: self.scale });
    }

    fn show(&mut self, offset: usize) {
        let maximum = self.data.lock().unwrap_or_else(|error| error.into_inner()).frame(&self.owner, 0)
            .map_or(0, |(_, frame)| frame.screen.scrollback_rows.saturating_sub(frame.screen.rows));
        let offset = offset.min(maximum);
        if self.offset == offset { return; }
        self.offset = offset;
        self.dirty = true;
        self.outbox.send(ClientMsg::Viewport { owner: self.owner.clone(), offset: self.offset });
    }
}

impl PaneBackend for TerminalView {
    fn pane_type(&self) -> PaneType { PaneType::Terminal }
    fn title(&self) -> &str { &self.title }
    fn set_size(&mut self, cols: usize, rows: usize) {
        if self.grid != (cols, rows) { self.grid = (cols, rows); self.resize(); }
    }
    fn set_cell_size(&mut self, w: f32, h: f32) {
        if self.cell != (w, h) { self.cell = (w, h); self.resize(); }
    }
    fn set_scale_factor(&mut self, scale: f32) {
        if self.scale != scale { self.scale = scale; self.resize(); }
    }
    fn cell_size(&self) -> (f32, f32) { self.cell }
    fn process_input(&mut self, bytes: &[u8]) { self.tell(ProcessInput::Bytes(bytes.to_vec())); }
    fn process_key_event(&mut self, key: &BackendKeyEvent) -> bool { self.tell(ProcessInput::Key(*key)); true }
    fn process_mouse_event(&mut self, mouse: &BackendMouseEvent) -> bool { self.tell(ProcessInput::Mouse(*mouse)); true }
    fn paste(&mut self, text: &str) { self.tell(ProcessInput::Paste(text.into())); }
    fn focus_changed(&mut self, focused: bool) { self.tell(ProcessInput::Focus(focused)); }
    fn should_close(&self) -> bool { false }
    fn update(&mut self) -> bool {
        let data = self.data.lock().unwrap_or_else(|error| error.into_inner());
        let Some((live_sequence, live)) = data.exact_frame(&self.owner, 0) else { return false };
        let live_changed = self.live_sequence != *live_sequence;
        let refresh = live_changed && self.offset > 0;
        if refresh {
            if let Some(top) = self.live_top {
                self.offset = self.offset.saturating_add_signed(live.screen.viewport_top_stable_row.saturating_sub(top));
            }
            self.offset = self.offset.min(live.screen.scrollback_rows.saturating_sub(live.screen.rows));
        }
        self.live_sequence = *live_sequence;
        self.live_top = Some(live.screen.viewport_top_stable_row);
        self.title.clone_from(&live.title);
        let mut changed = live_changed;
        if let Some((sequence, frame)) = data.exact_frame(&self.owner, self.offset) {
            changed |= self.sequence != *sequence;
            self.sequence = *sequence;
            self.visible = Some(frame.clone());
        }
        drop(data);
        if refresh { self.outbox.send(ClientMsg::Viewport { owner: self.owner.clone(), offset: self.offset }); }
        changed || self.dirty
    }
    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let data = self.data.lock().unwrap_or_else(|error| error.into_inner());
        let frame = self.visible.as_ref().or_else(|| data.frame(&self.owner, self.offset).map(|(_, frame)| frame))?;
        let images = frame.screen.graphics.iter().map(|placement| placement.image_id)
            .collect::<std::collections::HashSet<_>>().into_iter().filter_map(|id| data.images.get(&id).cloned()).collect();
        let mut snapshot = frame.screen.clone().into_snapshot(images);
        snapshot.cell_w = self.cell.0;
        snapshot.cell_h = self.cell.1;
        Some(snapshot)
    }
    fn take_terminal_damage(&mut self) -> TerminalDamage {
        let changed = self.dirty || self.painted != self.sequence;
        self.painted = self.sequence;
        self.dirty = false;
        if changed { TerminalDamage::Full } else { TerminalDamage::None }
    }
    fn render_data(&self) -> BackendRenderData {
        let snapshot = self.terminal_snapshot();
        BackendRenderData::Terminal {
            cursor_col: snapshot.as_ref().map_or(0, |s| s.cursor.col),
            cursor_row: snapshot.as_ref().map_or(0, |s| s.cursor.row),
            lines: snapshot.map_or_else(Vec::new, |s| s.lines), cell_w: self.cell.0, cell_h: self.cell.1,
        }
    }
    fn runtime(&self) -> heca_core::runtime::PaneRuntime {
        self.data.lock().unwrap_or_else(|error| error.into_inner()).frame(&self.owner, 0)
            .map_or_else(Default::default, |(_, frame)| frame.runtime.clone())
    }
    fn take_agent_updates(&mut self) -> Vec<Option<heca_core::agent::AgentState>> {
        let agent = self.runtime().agent;
        if agent == self.last_agent { return Vec::new(); }
        self.last_agent = agent.clone();
        vec![agent]
    }
    fn is_mouse_grabbed(&self) -> bool {
        self.data.lock().unwrap_or_else(|error| error.into_inner()).frame(&self.owner, 0)
            .is_some_and(|(_, frame)| frame.mouse_grabbed)
    }
    fn at_bottom(&self) -> bool { self.offset == 0 }
    fn scroll_viewport(&mut self, delta: i32) { self.show(self.offset.saturating_add_signed(delta as isize)); }
    fn scroll_to_top(&mut self) { self.show(usize::MAX); }
    fn scroll_to_bottom(&mut self) { self.show(0); }
}
