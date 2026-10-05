//! Fixtures for the tests of the server's handlers.

use std::time::Duration;

use heca_core::layout::testing::Windowed;
use heca_core::layout::PaneId;

use crate::app::backend_store::BackendStore;
use crate::input::WmAction;
use crate::notification::NotificationRuntime;
use crate::server::{Asker, Change, ServerCx, ServerState};

pub(super) fn server() -> ServerState {
    ServerState::new(
        BackendStore::new(),
        NotificationRuntime::with_capacity(
            50,
            Duration::from_millis(4000),
            Default::default(),
            5,
        ),
        Default::default(),
    )
}

/// A window on a session of two workspaces, each with two columns of one pane: 1, 2 and 11, 12.
pub(super) fn two_workspaces_of_two_columns() -> Windowed {
    Windowed::with_shape(&[&[&[1], &[2]], &[&[11], &[12]]])
}

pub(super) fn run(window: &mut Windowed, action: WmAction) -> Vec<Change> {
    run_for(window, None, action)
}

/// Run `action` for a window whose focused pane is `focused`.
pub(super) fn run_for(window: &mut Windowed, focused: Option<PaneId>, action: WmAction) -> Vec<Change> {
    let mut cx = ServerCx {
        layout: window.m(),
        asker: Asker { focused_pane: focused },
    };
    server().run(&mut cx, &action)
}
