//! **A window asking for a terminal** — the server starts it
//! ([`Terminals`](crate::server::Terminals)); the window adds what only it knows, its cell size.

use crate::app::backend_store::{Ensured, SpawnError, TerminalOwner, TerminalSpec};
use crate::app_state::AppState;

impl AppState {
    /// **Start the terminal for `owner`, or keep the one it has**, for this window's cell size.
    pub(crate) fn start_terminal(
        &mut self,
        owner: impl Into<TerminalOwner>,
        spec: TerminalSpec,
    ) -> Result<Ensured, SpawnError> {
        let cell = self.terminal_cell_size;
        self.server.terminals().start(owner, spec, cell)
    }
}
