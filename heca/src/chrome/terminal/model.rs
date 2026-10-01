//! What identifies a terminal, as plain data.

/// **Which terminal process** — an opaque id handed out by whoever owns the processes, never a
/// name the caller has to get right. A pane's terminal takes its identity from the pane.
///
/// "Terminal process", not "session": a session already means an instance of heca (F012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TerminalId(pub(crate) u64);
