//! Construct the process owner from configuration, before or without any window.

use super::{
    Asker,
    run::{Machine, ServerLoop},
};
use super::{
    ServerState, ServerWaker,
    identity::{ServerIdentity, ServerKind},
    persist::SavedSession,
};
use super::{session::SessionRuntime, sessions::SessionInfo};
use crate::{
    app::{
        backend_factory::{Launch, TerminalSettings},
        backend_store::BackendStore,
    },
    notification::NotificationRuntime,
};
use heca_config::loader::AppConfig;
use heca_core::layout::{Pane, PaneId, Rectangle, Session, SessionId, Size};

impl ServerState {
    pub(crate) fn from_config(config: &AppConfig, wake: ServerWaker) -> Self {
        let notifications =
            super::NotificationSettings::from(&config.config.settings.notification_system);
        Self::new(
            BackendStore::new(),
            NotificationRuntime::with_capacity(
                notifications.history_limit,
                notifications.auto_dismiss,
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
    /// A server for `config`, which is the one `identity` — claimed by the entry point that knows
    /// what kind of server this is.
    pub(crate) fn from_config(
        config: &AppConfig,
        identity: ServerIdentity,
        wake: ServerWaker,
    ) -> std::io::Result<Self> {
        Self::boot(config, identity, wake, Machine::default())
    }

    /// [`from_config`](Self::from_config) reaching the machine through `machine`. The sessions this server saved last time are rebuilt here, so
    /// the background server and a standalone window both start with them.
    pub(super) fn boot(
        config: &AppConfig,
        identity: ServerIdentity,
        wake: ServerWaker,
        machine: Machine,
    ) -> std::io::Result<Self> {
        let mut server = Self::empty().on(machine);
        server.open_store(&identity, config);
        // Before the services start, so a factory can see every action there is.
        let conflicts = server.register_actions();
        // A standalone window registers the same declarations in this process and reports what
        // collided; the background server has no window to.
        if identity.kind() == ServerKind::Background {
            conflicts.report();
        }
        server.start_services(config, identity, &wake);
        server.configure_factory(config.clone(), wake);
        server.restore_sessions(config);
        Ok(server)
    }
}

impl SessionRuntime {
    pub(super) fn from_config(
        config: &AppConfig,
        wake: ServerWaker,
        launch: Launch,
        identity: SessionInfo,
    ) -> std::io::Result<Self> {
        let (state, session, asker) =
            build_session(config, wake, launch, identity.id, identity.folder.clone())?;
        let mut runtime = Self::new(state, session);
        runtime.set_identity(identity);
        runtime.configure_recovery(&config.config.settings);
        runtime.configure_rpc(
            asker,
            config.config.confirm.clone(),
            config.config.settings.connection,
        );
        runtime.set_config(config);
        Ok(runtime)
    }
}

impl SessionRuntime {
    /// **A session rebuilt from what was saved**: the same key, name, folder, layout and terminals,
    /// stopped, with nothing started. [`restart`](Self::restart) starts it if it was running.
    pub(super) fn restored(
        config: &AppConfig,
        wake: ServerWaker,
        launch: Launch,
        mut saved: SavedSession,
    ) -> std::io::Result<Self> {
        let settings = &config.config.settings;
        let state = ServerState::from_config(config, wake).launching_with(launch);
        let session = saved
            .content
            .rebuild(saved.id, crate::app::startup::layout_options_from(config));
        let seen = saved
            .focus
            .and_then(|focus| focus.pane)
            .filter(|pane| {
                session
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.find_pane(*pane).is_some())
            })
            .or_else(|| {
                session
                    .workspaces
                    .iter()
                    .flat_map(|workspace| workspace.panes())
                    .next()
                    .map(|pane| pane.id)
            });
        let workspace = saved
            .focus
            .map_or(0, |focus| focus.workspace)
            .min(session.workspaces.len().saturating_sub(1));
        let asker = launch_asker(settings, seen, workspace)?;
        // A grid the file asks for is held to the same limit as one a window asks for.
        for terminal in &mut saved.terminals {
            let (cols, rows) = terminal.spec.grid;
            if !super::stream::valid_grid(
                cols,
                rows,
                asker.cell,
                1.0,
                settings.connection.max_terminal_cells.get(),
            ) {
                terminal.spec.grid = asker.grid();
            }
        }
        let mut runtime = Self::new(state, session);
        runtime.adopt(saved);
        runtime.configure_recovery(settings);
        runtime.configure_rpc(asker, config.config.confirm.clone(), settings.connection);
        runtime.set_config(config);
        Ok(runtime)
    }
}

/// **The asker a session's own requests are made as** — a window of the configured size, with
/// `focused_pane` focused in `workspace` — and the check that a terminal that size is allowed.
fn launch_asker(
    settings: &heca_config::settings::SettingsConfig,
    focused_pane: Option<PaneId>,
    workspace: usize,
) -> std::io::Result<Asker> {
    let asker = Asker {
        focused_pane,
        workspace,
        column: 0,
        area: Rectangle::from_size(Size::new(
            settings.window_width.into(),
            settings.window_height.into(),
        )),
        cell: heca_core::backend::DEFAULT_CELL_SIZE,
    };
    let grid = asker.grid();
    if !super::stream::valid_grid(
        grid.0,
        grid.1,
        asker.cell,
        1.0,
        settings.connection.max_terminal_cells.get(),
    ) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "configured launch size exceeds the terminal limit",
        ));
    }
    Ok(asker)
}

fn build_session(
    config: &AppConfig,
    wake: ServerWaker,
    launch: Launch,
    id: SessionId,
    folder: Option<std::path::PathBuf>,
) -> std::io::Result<(ServerState, Session, Asker)> {
    let settings = &config.config.settings;
    let mut state = ServerState::from_config(config, wake).launching_with(launch);
    let mut session = Session::new(id, crate::app::startup::layout_options_from(config));
    let pane_id = PaneId(session.next_id());
    let pane = Pane::new(pane_id, crate::pane_name(pane_id));
    // Session::new creates the initial workspace.
    let _ = session.add_column_with(0, 0, pane);
    let asker = launch_asker(settings, Some(pane_id), 0)?;
    let grid = asker.grid();
    let _ = state
        .terminals()
        .start(
            pane_id,
            crate::app::backend_store::TerminalSpec {
                program: crate::app::backend_store::Program::Shell,
                cwd: folder,
                grid,
            },
            asker.cell,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok((state, session, asker))
}
