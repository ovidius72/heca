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

/// **The name the user knows each pane by**: its custom name, else the program running in it. Read
/// before the runtime is refreshed — once a program has ended, its pane reports no program.
pub(crate) fn known_names(session: &Session) -> std::collections::HashMap<PaneId, String> {
    session.workspaces.iter().flat_map(|workspace| workspace.panes())
        .filter_map(|pane| {
            let name = pane.custom_name.clone().or_else(|| pane.runtime.program.clone())?;
            (!name.is_empty()).then_some((pane.id, name))
        })
        .collect()
}

/// **A pane's own program ended and the pane stays** (its close policy keeps it): say which program,
/// by `name` (from [`known_names`], read before the program ended) — never the internal pane
/// number. An error exit says its code. "Focus" goes to the pane.
pub(crate) fn notify_exited(name: Option<&str>, pane: PaneId, code: Option<i32>) {
    use crate::notification::{Notification, NotificationAction};
    use heca_view::{Intent, PropValue};
    let name = name.unwrap_or("The program");
    let focus = NotificationAction::new("Focus", Intent::new("focus_pane")
        .arg("pane_id", PropValue::Int(pane.0 as i64))).dismiss_after(true);
    let notification = match code {
        Some(0) | None => Notification::info(format!("{name} ended")),
        Some(code) => Notification::warning(format!("{name} ended")).body(format!("exit code {code}")),
    };
    notification.dedup_key(format!("pane.exited:{}", pane.0)).action(focus).send();
}

impl ServerState {
    pub(super) fn reconcile_processes(&mut self, session: &mut Session, mut closing: Vec<PaneId>, asker: Asker) -> (Vec<Change>, Vec<(PaneId, Option<i32>)>) {
        let names = known_names(session);
        let exits = sync_runtime(session, &mut self.backends);
        for (pane, code) in &exits {
            if exit_should_close(session, *pane, *code) { closing.push(*pane); }
            else if !closing.contains(pane) { notify_exited(names.get(pane).map(String::as_str), *pane, *code); }
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

#[cfg(test)]
mod tests {
    use crate::app::backend_store::{Program, TerminalSpec};
    use crate::notification::{NotificationDraft, NotificationId, install_notification_sink};
    use crate::server::{Asker, Change, ServerState};
    use heca_core::backend::FakeBackend;
    use heca_core::layout::{PaneId, testing::Windowed};
    use heca_core::runtime::PaneClosePolicy;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// **What becomes of a pane whose program ended is decided here, once**: a pane whose close
    /// policy says so is closed, with no notification; one that stays tells the user which program
    /// ended — by its name — and with what code.
    #[test]
    fn an_ended_program_closes_its_pane_or_says_which_program_ended() {
        let seen: Rc<RefCell<Vec<NotificationDraft>>> = Rc::default();
        let sink = seen.clone();
        install_notification_sink(move |draft| sink.borrow_mut().push(draft));
        let mut window = Windowed::with_shape(&[&[&[1], &[2]]]);
        let mut server = ServerState::for_tests(Default::default());
        for (id, close, code) in [(1, true, 0), (2, false, 3)] {
            let pane = window.session.workspaces[0].panes_mut().find(|p| p.id == PaneId(id)).unwrap();
            pane.close_policy = PaneClosePolicy { close_pane: close, ..Default::default() };
            pane.runtime.program = Some("htop".into());
            let mut fake = FakeBackend::new(20, 6);
            fake.queue_exit(code);
            server.backends.ensure(PaneId(id), TerminalSpec { program: Program::Shell, cwd: None, grid: (20, 6) },
                |_| Ok(Box::new(fake))).unwrap();
        }
        let asker = Asker::seen_through(window.l(), None, (10.0, 20.0));
        let (changes, exits) = server.reconcile_processes(&mut window.session, Vec::new(), asker);
        assert_eq!(exits.len(), 2);
        assert!(changes.contains(&Change::PaneRemoved { pane: PaneId(1) }), "its policy closes it");
        let seen = seen.borrow();
        assert_eq!(seen.len(), 1, "only the pane that stays is announced");
        let shown = seen[0].clone().build(NotificationId::from_raw(1), std::time::Instant::now());
        assert_eq!(shown.title, "htop ended");
        assert_eq!(shown.body.as_deref(), Some("exit code 3"));
    }
}
