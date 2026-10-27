//! Construct the process owner from configuration, before or without any window.

use heca_config::loader::AppConfig;
use super::{ServerState, ServerWaker};
use crate::{app::{backend_factory::TerminalSettings, backend_store::BackendStore}, notification::NotificationRuntime};
use heca_core::layout::{Pane, PaneId, Rectangle, Session, SessionId, Size};
use super::{Asker, run::ServerLoop};

impl ServerState {
    pub(crate) fn from_config(config: &AppConfig, wake: ServerWaker) -> Self {
        let notifications = &config.config.settings.notification_system;
        Self::new(
            BackendStore::new(),
            NotificationRuntime::with_capacity(
                // Existing history policy; moving ownership must not change retention.
                50,
                std::time::Duration::from_millis(notifications.auto_dismiss_ms),
                notifications.mode,
                notifications.max_visible,
            ),
            config.config.programs.clone(),
            TerminalSettings::from_config(config),
            Some(wake),
        )
    }
}

impl ServerLoop {
    pub(crate) fn from_config(config: &AppConfig, wake: ServerWaker) -> std::io::Result<Self> {
        let settings = &config.config.settings;
        let mut state = ServerState::from_config(config, wake);
        let mut session = Session::new(SessionId(1), crate::app::startup::layout_options_from(config));
        let pane_id = PaneId(session.next_id());
        let pane = Pane::new(pane_id, crate::pane_name(pane_id));
        // Session::new creates the initial workspace.
        let _ = session.add_column_with(0, 0, pane);
        let asker = Asker {
            focused_pane: Some(pane_id), workspace: 0, column: 0,
            area: Rectangle::from_size(Size::new(settings.window_width.into(), settings.window_height.into())),
            cell: heca_core::backend::DEFAULT_CELL_SIZE,
        };
        let grid = asker.grid();
        if !super::stream::valid_grid(grid.0, grid.1, asker.cell, 1.0, settings.connection.max_terminal_cells.get()) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "configured launch size exceeds the terminal limit"));
        }
        state.terminals().start_shell(pane_id, grid, asker.cell);
        let mut server = Self::new(state, session);
        server.configure_rpc(asker, config.config.confirm.clone(), settings.connection);
        Ok(server)
    }
}
