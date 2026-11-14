use super::*;
use crate::app::conflicts::Conflicts;
use crate::server::wire::service::{ServiceUpdate, ServiceUpdateKind};

/// A counter: the snapshot is the total, a delta what was added.
pub(crate) struct Counter;

impl Service for Counter {
    type Message = ();
    type Snapshot = u64;
    type Delta = u64;
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> {
        Ok(())
    }

    fn project(&self, _: &ProjectCx<'_>) -> Option<Projection<u64, u64>> {
        None
    }
}

fn fresh() -> super::super::Extension {
    reset();
    super::super::extension("demo")
}

fn registered() -> (crate::actions::ActionCatalog, Conflicts) {
    let mut registry = crate::actions::ActionRegistry::new();
    let mut catalog = crate::actions::ActionCatalog::with_builtins();
    let mut conflicts = Conflicts::default();
    crate::entry::register_queued_actions(&mut registry, &mut catalog, &mut conflicts);
    (catalog, conflicts)
}

fn update(revision: u64, kind: ServiceUpdateKind) -> ServiceUpdate {
    ServiceUpdate { service: "demo.counter".into(), schema_version: 1, server_epoch: 7, revision, kind }
}

/// **A service's actions are in the one catalog, under the extension's name** — and a second
/// service of the same name is refused.
#[test]
fn a_services_actions_join_the_catalog_and_a_second_service_of_the_same_name_is_refused() {
    let demo = fresh();
    let counter = demo.service("counter", |_| Ok(Counter));
    counter.action(demo.action("add").label("Add"), |_, _, _| ActionResult::Done(None));
    let again = demo.service("counter", |_| Ok(Counter));
    again.action(demo.action("other"), |_, _, _| ActionResult::Done(None));
    let (catalog, conflicts) = registered();
    assert_eq!(catalog.find("demo.add").expect("in the catalog").label, "Add");
    assert!(catalog.find("demo.other").is_none(), "what hangs off a refused service is dropped");
    assert!(conflicts.is_empty());
    assert_eq!(declared().len(), 1);
}

/// **A service action whose name is taken is refused out loud, not swapped in.**
#[test]
fn a_service_action_whose_name_is_taken_is_refused_and_reported() {
    let demo = fresh();
    demo.action("add").label("Plain").run(|_, _| heca_grid_ui::Handled::Yes);
    let counter = demo.service("counter", |_| Ok(Counter));
    counter.action(demo.action("add").label("Service"), |_, _, _| ActionResult::Done(None));
    let (catalog, conflicts) = registered();
    assert_eq!(catalog.find("demo.add").unwrap().label, "Plain");
    assert_eq!(conflicts.actions.len(), 1);
    assert_eq!(conflicts.actions[0].id, "demo.add");
}

/// **After a server started, nothing more can be hung off a service.**
#[test]
fn an_action_added_after_a_server_started_is_dropped() {
    let demo = fresh();
    let counter = demo.service("counter", |_| Ok(Counter));
    let _ = declared();
    counter.action(demo.action("late"), |_, _, _| ActionResult::Done(None));
    assert!(declared()[0].action_metas().is_empty());
}

/// **A window's value follows snapshots and deltas, and asks again when a delta does not follow.**
#[test]
fn the_window_value_follows_snapshots_and_deltas_and_asks_again_when_one_is_missed() {
    let demo = fresh();
    let counter = demo.service("counter", |_| Ok(Counter));
    let shown = counter.client(0u64, |value, update| match update {
        Update::Snapshot(total) => *value = total,
        Update::Delta(added) => *value += added,
    });
    let mut asked = Vec::new();
    client::apply_updates(false, vec![
        update(3, ServiceUpdateKind::Snapshot(5.into())),
        update(4, ServiceUpdateKind::Delta { base_revision: 3, value: 2.into() }),
    ], |service| asked.push(service.to_owned()));
    assert_eq!(shown.get(), 7);
    assert!(asked.is_empty());

    client::apply_updates(false, vec![update(9, ServiceUpdateKind::Delta { base_revision: 8, value: 1.into() })],
        |service| asked.push(service.to_owned()));
    assert_eq!(shown.get(), 7, "a delta on a revision this window lacks is not applied");
    assert_eq!(asked, ["demo.counter"], "it asks for a fresh snapshot instead");

    let mut other_run = update(5, ServiceUpdateKind::Delta { base_revision: 4, value: 1.into() });
    other_run.server_epoch = 8;
    client::apply_updates(false, vec![other_run], |service| asked.push(service.to_owned()));
    assert_eq!(asked.len(), 2, "the same revision from another server run does not follow either");
}

/// **Removal, a session change and an unknown shape all leave the initial value.**
#[test]
fn removal_and_a_session_change_reset_the_value_and_another_shape_is_not_read() {
    let demo = fresh();
    let counter = demo.service("counter", |_| Ok(Counter));
    let shown = counter.client(0u64, |value, update| if let Update::Snapshot(total) = update { *value = total });
    let ignore = |_: &str| {};
    client::apply_updates(false, vec![update(1, ServiceUpdateKind::Snapshot(5.into()))], ignore);
    client::apply_updates(false, vec![update(0, ServiceUpdateKind::Removed)], ignore);
    assert_eq!(shown.get(), 0);
    client::apply_updates(false, vec![update(2, ServiceUpdateKind::Snapshot(6.into()))], ignore);
    client::apply_updates(true, Vec::new(), ignore);
    assert_eq!(shown.get(), 0, "a new session starts from nothing");
    let mut newer = update(3, ServiceUpdateKind::Snapshot(9.into()));
    newer.schema_version = 2;
    let (_, reports) = crate::notification::collect_notifications(|| client::apply_updates(false, vec![newer], ignore));
    assert_eq!(shown.get(), 0, "a shape this window was not built for is not read");
    assert_eq!(reports.len(), 1, "and it says so");
}
