//! Paged history reads. Copy and search never wait for socket I/O on the window thread.

use std::{collections::HashMap, io};
use heca_core::backend::{SearchMatch, TerminalLine, TerminalSnapshot};
use crate::{app::backend_store::TerminalOwner, chrome::panes::terminal::TerminalId,
    server::wire::{ClientMsg, terminal::ReadData}};
use super::outbox::Outbox;

pub(crate) enum ReadEffect {
    Clipboard { text: Option<String>, selection: crate::app::terminal_host::selection::SelectionState },
    Matches { terminal: TerminalId, matches: Vec<SearchMatch> },
}

enum Kind {
    Lines { lines: Vec<TerminalLine>, complete: Box<dyn FnOnce(Vec<TerminalLine>) -> ReadEffect> },
    Search { terminal: TerminalId, query: String, matches: Vec<SearchMatch> },
}

struct Pending {
    owner: TerminalOwner, next: isize, end: isize, page_end: isize, cols: usize, rows: usize, kind: Kind,
}

#[derive(Default)]
pub(super) struct Reads {
    next: u64,
    pending: HashMap<u64, Pending>,
    latest_search: HashMap<TerminalId, u64>,
}

impl Pending {
    fn send(&mut self, id: u64, outbox: &Outbox) {
        self.page_end = self.next.saturating_add(isize::try_from(self.rows.saturating_sub(1)).unwrap_or(isize::MAX)).min(self.end);
        let query = match &self.kind { Kind::Search { query, .. } => Some(query.clone()), Kind::Lines { .. } => None };
        outbox.send(ClientMsg::Read { request_id: id, owner: self.owner.clone(), start: self.next,
            end: self.page_end, cols: self.cols, query });
    }
}

impl Reads {
    pub(super) fn lines(&mut self, owner: TerminalOwner, snapshot: &TerminalSnapshot, start: isize, end: isize,
        complete: impl FnOnce(Vec<TerminalLine>) -> ReadEffect + 'static, outbox: &Outbox) {
        self.next += 1;
        let mut pending = Pending { owner, next: start, end, page_end: start, cols: snapshot.cols,
            rows: snapshot.rows.max(1), kind: Kind::Lines { lines: Vec::new(), complete: Box::new(complete) } };
        pending.send(self.next, outbox);
        self.pending.insert(self.next, pending);
    }

    pub(super) fn search(&mut self, terminal: TerminalId, owner: TerminalOwner, snapshot: &TerminalSnapshot, query: String, outbox: &Outbox) {
        self.next += 1;
        self.latest_search.insert(terminal, self.next);
        let live_top = snapshot.viewport_top_stable_row.saturating_add(isize::try_from(snapshot.viewport_offset).unwrap_or(isize::MAX));
        let start = live_top.saturating_sub(isize::try_from(snapshot.scrollback_rows.saturating_sub(snapshot.rows)).unwrap_or(isize::MAX));
        let end = live_top.saturating_add(isize::try_from(snapshot.rows.saturating_sub(1)).unwrap_or(isize::MAX));
        let mut pending = Pending { owner, next: start, end, page_end: start, cols: snapshot.cols,
            rows: snapshot.rows.max(1), kind: Kind::Search { terminal, query, matches: Vec::new() } };
        pending.send(self.next, outbox);
        self.pending.insert(self.next, pending);
    }

    pub(super) fn cancel_search(&mut self, terminal: TerminalId) { self.latest_search.remove(&terminal); }

    pub(super) fn receive(&mut self, id: u64, start: isize, data: ReadData, outbox: &Outbox) -> io::Result<Option<ReadEffect>> {
        let Some(mut pending) = self.pending.remove(&id) else { return Ok(None) };
        if let Kind::Search { terminal, .. } = &pending.kind
            && self.latest_search.get(terminal) != Some(&id) { return Ok(None); }
        if start != pending.next { return Err(io::Error::new(io::ErrorKind::InvalidData, "history page is out of order")); }
        match (&mut pending.kind, data) {
            (Kind::Lines { lines, .. }, ReadData::Lines(page)) => lines.extend(page),
            (Kind::Search { matches, .. }, ReadData::Matches(page)) => matches.extend(page),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected history response")),
        }
        if pending.page_end < pending.end {
            pending.next = pending.page_end + 1;
            pending.send(id, outbox);
            self.pending.insert(id, pending);
            return Ok(None);
        }
        Ok(Some(match pending.kind {
            Kind::Lines { lines, complete } => complete(lines),
            Kind::Search { terminal, matches, .. } => ReadEffect::Matches { terminal, matches },
        }))
    }
}
