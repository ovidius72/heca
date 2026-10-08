//! Fixtures for the tests of the server's handlers.

use std::time::Duration;

use heca_core::layout::testing::Windowed;
use heca_core::layout::PaneId;

use crate::app::backend_store::BackendStore;
use crate::input::WmAction;
use crate::notification::NotificationRuntime;
use crate::server::{Asker, Change, ServerCx, ServerLayout, ServerState};

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

/// Run `action` for a window whose focused pane is `focused`, and that shows what it shows.
pub(super) fn run_for(window: &mut Windowed, focused: Option<PaneId>, action: WmAction) -> Vec<Change> {
    let asker = Asker::seen_through(window.l(), focused);
    run_as(window, asker, action)
}

/// Run `action` for this asker — which need not be where the window's own view is, as it is not
/// when two windows share a session.
pub(super) fn run_as(window: &mut Windowed, asker: Asker, action: WmAction) -> Vec<Change> {
    let mut cx = ServerCx {
        layout: ServerLayout::new(window.m()),
        asker,
    };
    server().run(&mut cx, &action)
}
