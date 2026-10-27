//! Canonical process state and exit policy. Neither needs an attached window.

use heca_core::layout::{PaneId, Session};
use crate::{app::backend_store::BackendStore, input::WmAction};
use super::{Asker, Change, ServerState};

pub(crate) fn sync_runtime(session: &mut Session, backends: &mut BackendStore) -> Vec<(PaneId, Option<i32>)> {
    let mut exits = Vec::new();
    for pane in session.workspaces.iter_mut().flat_map(|workspace| workspace.panes_mut()) {
        if let Some(backend) = backends.get_mut(pane.id) {
            pane.runtime = backend.runtime();
            if let Some(code) = backend.take_exit_code() { exits.push((pane.id, Some(code))); }
        }
    }
    exits
}

pub(crate) fn exit_should_close(session: &Session, id: PaneId, code: Option<i32>) -> bool {
    session.workspaces.iter().find_map(|workspace| workspace.find_pane(id))
        .is_some_and(|pane| pane.close_policy.should_close(code))
}

pub(crate) fn notify_exited(pane: PaneId, code: Option<i32>) {
    use crate::notification::{Notification, NotificationAction};
    use heca_view::{Intent, PropValue};
    let focus = NotificationAction::new("Focus", Intent::new("focus_pane")
        .arg("pane_id", PropValue::Int(pane.0 as i64))).dismiss_after(true);
    let notification = match code {
        Some(0) | None => Notification::info(format!("Pane {} process exited", pane.0)),
        Some(code) => Notification::warning(format!("Pane {} process exited", pane.0)).body(format!("exit code {code}")),
    };
    notification.dedup_key(format!("pane.exited:{}", pane.0)).action(focus).send();
}

impl ServerState {
    pub(super) fn reconcile_processes(&mut self, session: &mut Session, mut closing: Vec<PaneId>, asker: Asker) -> (Vec<Change>, Vec<(PaneId, Option<i32>)>) {
        let exits = sync_runtime(session, &mut self.backends);
        for (pane, code) in &exits {
            if exit_should_close(session, *pane, *code) { closing.push(*pane); }
            else if !closing.contains(pane) { notify_exited(*pane, *code); }
        }
        closing.sort_unstable_by_key(|pane| pane.0);
        closing.dedup();
        let mut changes = Vec::new();
        for pane_id in closing {
            changes.extend(self.run(session, asker, &WmAction::ClosePaneById { pane_id }));
        }
        (changes, exits)
    }
}
