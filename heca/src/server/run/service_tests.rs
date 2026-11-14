//! Extension services driven through a real server loop: one per server, windows kept in step,
//! actions from windows and scripts, deferred answers, and a single stop.

use super::*;
use crate::entry::service::{
    ActionResult, ProjectCx, Projection, Service, ServiceCx, ServiceInbox, ServiceResult, ServiceTicket, Stop,
};
use crate::server::wire::{ClientLink, service::ServiceUpdateKind};
use heca_core::layout::LayoutOptions;
use std::cell::{Cell, RefCell};
use std::sync::mpsc::channel;

#[derive(Debug)]
enum Msg {
    Add(u64),
    Settle,
    WorkerDone,
}

/// A counter with a worker: a snapshot is the total, a delta what was last added.
struct Rooms {
    total: u64,
    revision: u64,
    last: Option<u64>,
    pending: Vec<ServiceTicket>,
    busy: bool,
    stops: Rc<Cell<u32>>,
    asked: Rc<Cell<u32>>,
}

impl Service for Rooms {
    type Message = Msg;
    type Snapshot = u64;
    type Delta = u64;
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, cx: &mut ServiceCx<'_, Msg>, message: Msg) -> ServiceResult<()> {
        match message {
            Msg::Add(n) => {
                self.total += n;
                self.revision += 1;
                self.last = Some(n);
                cx.republish();
            }
            Msg::Settle => for ticket in self.pending.drain(..) { cx.complete(ticket, Ok(Some("settled".into()))) },
            Msg::WorkerDone => self.busy = false,
        }
        Ok(())
    }

    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<u64, u64>> {
        self.asked.set(self.asked.get() + 1);
        Some(match (cx.since(), self.last) {
            (Some(since), _) if since == self.revision => Projection::Unchanged,
            (Some(since), Some(added)) if since + 1 == self.revision =>
                Projection::Delta { base: since, revision: self.revision, value: added },
            _ => Projection::Snapshot { revision: self.revision, value: self.total },
        })
    }

    fn stop(&mut self, cx: &mut ServiceCx<'_, Msg>) -> Stop {
        self.stops.set(self.stops.get() + 1);
        if !self.busy { return Stop::Done; }
        let inbox = cx.inbox();
        std::thread::spawn(move || { let _ = inbox.send(Msg::WorkerDone); });
        Stop::Pending
    }
}

struct Declared {
    starts: Rc<Cell<u32>>,
    stops: Rc<Cell<u32>>,
    asked: Rc<Cell<u32>>,
    inboxes: Rc<RefCell<Vec<ServiceInbox<Msg>>>>,
}

impl Declared {
    fn inbox(&self) -> ServiceInbox<Msg> {
        self.inboxes.borrow().last().cloned().expect("started")
    }
}

/// `demo.rooms` with its actions, and `demo.broken` when asked, declared as a program would.
fn declare(busy: bool, broken: bool) -> Declared {
    crate::entry::service::reset();
    let declared = Declared { starts: Rc::default(), stops: Rc::default(), asked: Rc::default(), inboxes: Rc::default() };
    let demo = crate::entry::extension("demo");
    let (starts, stops, asked, inboxes) =
        (declared.starts.clone(), declared.stops.clone(), declared.asked.clone(), declared.inboxes.clone());
    let rooms = demo.service("rooms", move |cx| {
        starts.set(starts.get() + 1);
        inboxes.borrow_mut().push(cx.inbox());
        Ok(Rooms { total: 0, revision: 0, last: None, pending: Vec::new(), busy, stops: stops.clone(), asked: asked.clone() })
    });
    rooms.action(demo.action("add"), |rooms, cx, _| {
        rooms.total += 1;
        rooms.revision += 1;
        rooms.last = Some(1);
        cx.republish();
        ActionResult::Done(None)
    });
    rooms.action(demo.action("later"), |rooms, cx, _| {
        let ticket = cx.later();
        rooms.pending.push(ticket);
        ActionResult::Later(ticket)
    });
    rooms.action(demo.action("who"), |_, cx, _| ActionResult::Done(Some(format!("{} {:?}", cx.session(), cx.client()))));
    rooms.action(demo.action("total"), |rooms, _, _| ActionResult::Done(Some(rooms.total.to_string())));
    rooms.action(demo.action("refuse"), |_, _, _| ActionResult::Refused("not now".into()));
    // A plain extension action: its follow-up names a service action.
    demo.action("chain").run(|cx, _| { cx.dispatch("demo.add", crate::chrome::PropMap::new()); heca_grid_ui::event::Handled::Yes });
    if broken {
        demo.service::<Rooms>("broken", |_| Err("no database".into()));
    }
    declared
}

fn config(text: &str) -> heca_config::loader::AppConfig {
    heca_config::loader::AppConfig { config: heca_config::loader::config_from_layers(text).unwrap(), theme: Default::default() }
}

/// A server started the way the background server and a standalone window start theirs, with a
/// test session in it and the extensions' actions registered.
fn server(config: &heca_config::loader::AppConfig) -> ServerLoop {
    server_waking(config, ServerWaker::new(|| {}))
}

fn server_waking(config: &heca_config::loader::AppConfig, wake: ServerWaker) -> ServerLoop {
    let mut server = ServerLoop::from_config(config, wake).unwrap()
        .with_session(ServerState::for_tests(Default::default()), Session::new(SessionId(1), LayoutOptions::default()));
    let mut registry = crate::actions::ActionRegistry::new();
    let mut catalog = crate::actions::ActionCatalog::with_builtins();
    crate::entry::register_queued_actions(&mut registry, &mut catalog, &mut Default::default());
    server.install_actions(registry, catalog);
    server
}

fn attached(server: &mut ServerLoop) -> crate::server::wire::InProcessClient {
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    client
}

fn drain(client: &impl ClientLink) -> Vec<ServerMsg> {
    std::iter::from_fn(|| client.poll().unwrap()).collect()
}

/// What the window was sent by services, as (revision, kind).
fn updates(client: &impl ClientLink) -> Vec<(u64, ServiceUpdateKind)> {
    drain(client).into_iter().filter_map(|message| match message {
        ServerMsg::Service(update) => Some((update.revision, update.kind)),
        _ => None,
    }).collect()
}

fn rpc(server: &mut ServerLoop, line: &str) -> std::sync::mpsc::Receiver<String> {
    let (reply, answer) = channel();
    server.begin_rpc(line.into(), reply);
    answer
}

/// **One instance per server** — two servers in one process get one each, and a window coming
/// and going never starts another.
#[test]
fn every_server_starts_each_service_once_and_windows_never_restart_it() {
    let declared = declare(false, false);
    let config = config("");
    let mut first = server(&config);
    let _second = server(&config);
    assert_eq!(declared.starts.get(), 2, "one per server");
    let window = attached(&mut first);
    window.send(ClientMsg::LeaveSession).unwrap();
    first.pump(Instant::now());
    window.send(ClientMsg::Attach).unwrap();
    first.pump(Instant::now());
    assert_eq!(declared.starts.get(), 2, "detaching and reattaching restart nothing");
}

/// **A window gets a snapshot, then deltas; reattaching or asking again gets a snapshot.** A
/// message from another thread is what moves it.
#[test]
fn a_window_is_kept_in_step_by_snapshots_and_deltas() {
    let declared = declare(false, false);
    let mut server = server(&config(""));
    let window = attached(&mut server);
    assert_eq!(updates(&window), [(0, ServiceUpdateKind::Snapshot(0.into()))], "the first thing a window gets");

    let inbox = declared.inbox();
    std::thread::spawn(move || inbox.send(Msg::Add(2)).ok()).join().unwrap();
    server.pump(Instant::now());
    assert_eq!(updates(&window), [(1, ServiceUpdateKind::Delta { base_revision: 0, value: 2.into() })]);
    let asked = declared.asked.get();
    server.pump(Instant::now());
    assert!(updates(&window).is_empty(), "nothing new, nothing sent");
    assert_eq!(declared.asked.get(), asked, "and nothing asked: a window that has the latest is left alone");

    window.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert_eq!(updates(&window), [(1, ServiceUpdateKind::Snapshot(2.into()))], "reattaching starts again");
    window.send(ClientMsg::ServiceResync { service: "demo.rooms".into() }).unwrap();
    server.pump(Instant::now());
    assert_eq!(updates(&window), [(1, ServiceUpdateKind::Snapshot(2.into()))], "so does asking");
}

/// **A script's call answers at once or when the ticket is settled** — and says who asked.
#[test]
fn a_script_gets_an_immediate_answer_or_a_deferred_one() {
    let declared = declare(false, false);
    let mut server = server(&config(""));
    assert_eq!(rpc(&mut server, "demo.add").try_recv().unwrap(), "ok");
    assert_eq!(rpc(&mut server, "demo.who").try_recv().unwrap(), "1 None", "a script has a session, no window");
    assert_eq!(rpc(&mut server, "demo.refuse").try_recv().unwrap(), "error: not now");
    let later = rpc(&mut server, "demo.later");
    assert!(later.try_recv().is_err(), "accepted, not answered yet");
    declared.inbox().send(Msg::Settle).unwrap();
    server.pump(Instant::now());
    assert_eq!(later.try_recv().unwrap(), "settled");
}

/// **A script's plain extension action can follow up with a service action.** The chain runs the
/// service, and the script is told "ok".
#[test]
fn a_scripts_chain_can_end_in_a_service_action() {
    declare(false, false);
    let mut server = server(&config(""));
    assert_eq!(rpc(&mut server, "demo.chain").try_recv().unwrap(), "ok");
    assert_eq!(rpc(&mut server, "demo.total").try_recv().unwrap(), "1", "the follow-up ran the service");
}

/// **A window's call reaches the service** — and a refusal is shown to the windows.
#[test]
fn a_windows_call_reaches_the_service_and_a_refusal_is_shown() {
    declare(false, false);
    let mut server = server(&config(""));
    let window = attached(&mut server);
    drain(&window);
    window.send(ClientMsg::Invoke { intent: crate::chrome::Intent::new("demo.add") }).unwrap();
    server.pump(Instant::now());
    assert_eq!(updates(&window), [(1, ServiceUpdateKind::Delta { base_revision: 0, value: 1.into() })]);
    window.send(ClientMsg::Invoke { intent: crate::chrome::Intent::new("demo.refuse") }).unwrap();
    server.pump(Instant::now());
    assert!(drain(&window).iter().any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains("demo.refuse")))), "the refusal is visible");
}

/// **Shutdown stops each service once, delivering what its worker sends while it finishes,** and
/// answers a caller still waiting.
#[test]
fn shutdown_waits_for_a_stopping_service_and_happens_once() {
    let declared = declare(true, false);
    let mut server = server(&config(""));
    let waiting = rpc(&mut server, "demo.later");
    let inbox = declared.inbox();
    server.shutdown();
    assert_eq!(declared.stops.get(), 2, "asked again once its worker said it was done");
    assert_eq!(waiting.try_recv().unwrap(), "error: the server stopped");
    server.shutdown();
    assert_eq!(declared.stops.get(), 2, "stopped once");
    assert!(inbox.send(Msg::Add(1)).is_err(), "a stopped service's inbox is closed");
}

/// **A failing factory is reported and the server runs without it.**
#[test]
fn a_failing_factory_is_reported_and_the_rest_still_runs() {
    let declared = declare(false, true);
    let mut server = server(&config(""));
    assert_eq!(declared.starts.get(), 1);
    let window = attached(&mut server);
    let messages = drain(&window);
    assert!(messages.iter().any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains("demo.broken")))), "the failure is shown");
    assert!(messages.iter().any(|message| matches!(message, ServerMsg::Service(_))), "demo.rooms still runs");
}

/// **An update over the frame limit is refused out loud and not sent.**
#[test]
fn an_update_over_the_frame_limit_is_not_sent() {
    declare(false, false);
    let mut server = server(&config("[settings.connection]\nmax_frame_bytes = 16"));
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    let (_, reports) = crate::notification::collect_notifications(|| server.pump(Instant::now()));
    assert!(updates(&client).is_empty());
    assert!(reports.iter().any(|draft| format!("{draft:?}").contains("demo.rooms")), "and it says so");
}

/// **The background server's own loop**: a worker's message wakes it, and when the loop ends the
/// service is stopped.
#[test]
fn the_owner_loop_wakes_for_a_workers_message_and_stops_services_when_it_ends() {
    let (handle, inbox) = crate::server::runtime::channel(Default::default());
    let (started, take) = channel();
    let waker = handle.waker();
    let owner = std::thread::spawn(move || {
        // Declarations and the server live on the owner thread, as in a program's `main`.
        let declared = declare(false, false);
        let server = server_waking(&config(""), waker);
        started.send(declared.inbox()).unwrap();
        inbox.run(server);
        declared.stops.get()
    });
    let rooms = take.recv().unwrap();
    rooms.send(Msg::Add(5)).unwrap();
    let (reply, answer) = channel();
    handle.rpc("demo.total".into(), reply).unwrap();
    assert_eq!(answer.recv_timeout(std::time::Duration::from_secs(3)).unwrap(), "5");
    handle.stop().unwrap();
    assert_eq!(owner.join().unwrap(), 1, "stopped once, as the loop ended");
}
