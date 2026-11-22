//! What a service's factory is told about the server's actions, and that building them once for the
//! server leaves a window in the same process its own.

use super::service_tests::{config, server_as};
use super::*;
use crate::actions::{
    ActionCatalog, ActionCatalogSnapshot, ActionMeta, ActionRegistry, register_dynamic,
};
use crate::chrome::{Intent, SharedChromeState};
use crate::entry::service::{
    ActionResult, ProjectCx, Projection, Service, ServiceCx, ServiceResult,
};
use crate::server::identity::{ServerIdentity, ServerKind};
use std::cell::{Cell, RefCell};

struct Quiet;

impl Service for Quiet {
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

/// What each start of `demo.probe` was told, and how often the plain action ran.
#[derive(Default)]
struct Seen {
    snapshots: Rc<RefCell<Vec<ActionCatalogSnapshot>>>,
    ran: Rc<Cell<u32>>,
}

/// An extension with a plain action and two services, as a program declares them in `main`.
fn declare() -> Seen {
    crate::entry::service::reset();
    let seen = Seen::default();
    let demo = crate::entry::extension("demo");
    let ran = seen.ran.clone();
    demo.action("plain").run(move |_, _| {
        ran.set(ran.get() + 1);
        heca_grid_ui::Handled::Yes
    });
    let snapshots = seen.snapshots.clone();
    let probe = demo.service("probe", move |cx| {
        snapshots.borrow_mut().push(cx.action_catalog());
        Ok(Quiet)
    });
    probe.action(demo.action("mine"), |_, _, _| ActionResult::Done(None));
    let other = demo.service("other", |_| Ok(Quiet));
    other.action(demo.action("theirs"), |_, _, _| ActionResult::Done(None));
    seen
}

fn started_as(kind: ServerKind) -> ServerLoop {
    server_as(
        &config(""),
        ServerIdentity::for_tests(kind),
        ServerWaker::new(|| {}),
    )
}

fn names(snapshot: &ActionCatalogSnapshot) -> Vec<String> {
    snapshot.names().map(str::to_owned).collect()
}

/// **A factory sees everything the server will have** — heca's own, `kill_server`, an extension's
/// action, its own service's and another service's — and nothing that does not exist.
#[test]
fn a_factory_sees_every_action_the_server_will_have() {
    let seen = declare();
    let _server = started_as(ServerKind::Background);
    let snapshot = seen.snapshots.borrow()[0].clone();
    for name in [
        "split_horizontal",
        "kill_server",
        "demo.plain",
        "demo.mine",
        "demo.theirs",
    ] {
        assert!(snapshot.contains(name), "{name} is in it");
    }
    assert!(!snapshot.contains("demo.unknown"));
    assert!(!snapshot.contains(""));
    let listed = names(&snapshot);
    assert!(
        listed.windows(2).all(|pair| pair[0] < pair[1]),
        "in order, each once"
    );
    assert!(listed.iter().any(|name| name == "demo.theirs"));
}

/// **The snapshot is a value a worker thread can keep and read.**
#[test]
fn the_snapshot_can_be_read_on_another_thread() {
    let seen = declare();
    let _server = started_as(ServerKind::Background);
    let snapshot = seen.snapshots.borrow()[0].clone();
    let (has, missing) =
        std::thread::spawn(move || (snapshot.contains("demo.mine"), snapshot.contains("nope")))
            .join()
            .unwrap();
    assert!(has && !missing);
}

/// **A dock's own action is a window's, so the server does not have it** — and a window that
/// installs its catalog later does not change what a service was told.
#[test]
fn an_action_only_a_window_declares_is_not_in_the_snapshot() {
    let seen = declare();
    let mut server = started_as(ServerKind::Standalone);
    let told = seen.snapshots.borrow()[0].clone();

    let (mut registry, mut catalog) = (ActionRegistry::new(), ActionCatalog::with_builtins());
    register_dynamic(
        &mut registry,
        &mut catalog,
        ActionMeta::new("dock.restart_selected"),
        None,
    )
    .unwrap();
    server.install_actions(registry, catalog.clone());

    assert!(catalog.snapshot().contains("dock.restart_selected"));
    assert!(
        !told.contains("dock.restart_selected"),
        "what a service was told does not change"
    );
    assert!(!server.catalog.snapshot().contains("demo.unknown"));
}

/// **The standalone server and the background server tell their services the same thing.**
#[test]
fn both_ways_of_starting_a_server_give_a_service_the_same_snapshot() {
    let seen = declare();
    let _background = started_as(ServerKind::Background);
    let _standalone = started_as(ServerKind::Standalone);
    let told = seen.snapshots.borrow();
    assert_eq!(told.len(), 2);
    assert_eq!(names(&told[0]), names(&told[1]));
}

/// **In one process the window still gets the plain extension actions after the server read its
/// own** — one queue, read by both, and the window can run them.
#[test]
fn a_standalone_window_still_registers_and_runs_the_plain_actions_after_the_server() {
    let seen = declare();
    let mut server = started_as(ServerKind::Standalone);
    assert!(server.catalog.snapshot().contains("demo.plain"));

    // What the window does as it starts.
    let (mut registry, mut catalog, mut conflicts) = (
        ActionRegistry::new(),
        ActionCatalog::with_builtins(),
        crate::app::conflicts::Conflicts::default(),
    );
    crate::entry::register_queued_actions(&mut registry, &mut catalog, &mut conflicts);
    assert!(
        conflicts.is_empty(),
        "no conflict from registering again: {conflicts:?}"
    );
    for name in ["demo.plain", "demo.mine", "demo.theirs"] {
        assert!(
            catalog.snapshot().contains(name),
            "{name} reached the window"
        );
    }
    let out = registry.execute_server_dynamic(
        "demo.plain",
        SharedChromeState::new(240.0, 240.0),
        &Intent::new("demo.plain"),
    );
    assert!(out.is_some());
    assert_eq!(seen.ran.get(), 1);

    server.install_actions(registry, catalog);
    assert_eq!(server.rpc("demo.plain"), "ok");
    assert_eq!(
        seen.ran.get(),
        2,
        "and the server still runs it after the window's install"
    );
}

/// **Each host builds its own actions from the same declarations and none reports a collision
/// that is not there.**
#[test]
fn two_hosts_reading_the_same_declarations_report_nothing() {
    let _seen = declare();
    assert!(ServerLoop::empty().register_actions().is_empty());
    assert!(ServerLoop::empty().register_actions().is_empty());
}
