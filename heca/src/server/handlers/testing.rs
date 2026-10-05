//! Fixtures for the tests of the server's handlers.

use std::time::Duration;

use heca_core::layout::testing::Windowed;
use heca_core::layout::{Pane, PaneId, Size};

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

/// A window on a session of two workspaces, each with two columns.
pub(super) fn two_workspaces_of_two_columns() -> Windowed {
    let mut window = Windowed::new(Size::new(1000.0, 800.0), 1.0);
    window.m().add_workspace();
    for idx in 0..2 {
        window.show(idx);
        for n in 0..2u64 {
            window
                .m()
                .add_pane(Pane::new(PaneId(10 * idx as u64 + n + 1), "p"), None, true);
        }
    }
    window.show(0);
    window
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
