//! Action policies and the requests a service hands in for others, through a real server loop: one
//! rule for a window, a script, a helper and a chain's follow-ups; a queue that answers at once,
//! keeps what is open and ends every request exactly once.

use super::service_tests::{attached, config, drain, register_actions, rpc, server};
use super::*;
use crate::entry::service::{
    ActionResult, ExternalRequest, Permit, ProjectCx, Projection, RequestOutcome, RequestRef,
    Requester, Sender, Service, ServiceCx, ServiceInbox, ServiceResult, ServiceTicket, SessionKey,
    SubmitError,
};
use crate::server::identity::{ServerIdentity, ServerKind};
use crate::server::wire::ClientLink;
use heca_core::layout::LayoutOptions;
use std::cell::RefCell;

/// What one policy question was about.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    requester: String,
    action: String,
    parent: Option<String>,
    session: Option<SessionKey>,
}

#[derive(Debug)]
enum Msg {
    Submit(ExternalRequest),
    Deny(&'static str),
    Settle,
}

/// Everything a test wants to look at after the service did it.
#[derive(Clone, Default)]
struct Log {
    asked: Rc<RefCell<Vec<Asked>>>,
    denied: Rc<RefCell<Vec<&'static str>>>,
    submitted: Rc<RefCell<Vec<Result<RequestRef, SubmitError>>>>,
    done: Rc<RefCell<Vec<(RequestRef, RequestOutcome)>>>,
    /// `"<action> in <session number> for <sender>"`, for each action that ran.
    ran: Rc<RefCell<Vec<String>>>,
    inboxes: Rc<RefCell<Vec<ServiceInbox<Msg>>>>,
}

impl Log {
    fn inbox(&self) -> ServiceInbox<Msg> {
        self.inboxes.borrow().last().cloned().expect("started")
    }

    fn ran(&self) -> Vec<String> {
        self.ran.borrow().clone()
    }

    fn done(&self) -> Vec<(String, RequestOutcome)> {
        self.done
            .borrow()
            .iter()
            .map(|(request, outcome)| (request.request_id().to_owned(), outcome.clone()))
            .collect()
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked.borrow().clone()
    }
}

struct Relay {
    log: Log,
    tickets: Vec<ServiceTicket>,
}

impl Service for Relay {
    type Message = Msg;
    type Snapshot = u8;
    type Delta = u8;
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, cx: &mut ServiceCx<'_, Msg>, message: Msg) -> ServiceResult<()> {
        match message {
            Msg::Submit(request) => {
                let result = cx.submit(request);
                self.log.submitted.borrow_mut().push(result);
            }
            Msg::Deny(action) => self.log.denied.borrow_mut().push(action),
            Msg::Settle => {
                for ticket in self.tickets.drain(..) {
                    cx.complete(ticket, Ok(Some("settled".into())))
                }
            }
        }
        Ok(())
    }

    fn project(&self, _: &ProjectCx<'_>) -> Option<Projection<u8, u8>> {
        None
    }

    fn request_done(
        &mut self,
        _: &mut ServiceCx<'_, Msg>,
        request: &RequestRef,
        outcome: RequestOutcome,
    ) {
        self.log.done.borrow_mut().push((request.clone(), outcome));
    }
}

fn who(requester: Requester<'_>) -> String {
    match requester {
        Requester::Local { window: None } => "local".into(),
        Requester::Local {
            window: Some(window),
        } => format!("window {window}"),
        Requester::External { service, sender } => format!("{service}/{sender}"),
    }
}

/// `demo.relay`, which declares a policy when asked, and the actions the tests run. A second
/// service, `demo.judge`, declares a policy that allows everything, when asked.
fn declare(policy: bool, judge: bool) -> Log {
    crate::entry::service::reset();
    let log = Log::default();
    let demo = crate::entry::extension("demo");
    let started = log.clone();
    let relay = demo.service("relay", move |cx| {
        started.inboxes.borrow_mut().push(cx.inbox());
        Ok(Relay {
            log: started.clone(),
            tickets: Vec::new(),
        })
    });
    if policy {
        let log = log.clone();
        relay.action_policy(move |_, _, request| {
            log.asked.borrow_mut().push(Asked {
                requester: who(request.requester()),
                action: request.action().to_owned(),
                parent: request.parent().map(str::to_owned),
                session: request.session(),
            });
            if log.denied.borrow().contains(&request.action()) {
                Permit::Deny(format!("{} is not allowed", request.action()))
            } else {
                Permit::Allow
            }
        });
    }
    if judge {
        let other = demo.service("judge", |_| {
            Ok(Relay {
                log: Log::default(),
                tickets: Vec::new(),
            })
        });
        other.action_policy(|_, _, _| Permit::Allow);
    }
    let ran = log.ran.clone();
    relay.action(demo.action("add"), move |_, cx, _| {
        ran.borrow_mut().push(format!(
            "add in {} for {:?}",
            cx.session(),
            cx.sender().map(Sender::as_str)
        ));
        ActionResult::Done(Some("added".into()))
    });
    relay.action(demo.action("later"), |relay, cx, _| {
        let ticket = cx.later();
        relay.tickets.push(ticket);
        ActionResult::Later(ticket)
    });
    let ran = log.ran.clone();
    relay.action(demo.action("wipe").destructive(), move |_, _, _| {
        ran.borrow_mut().push("wipe".into());
        ActionResult::Done(Some("wiped".into()))
    });
    relay.action(demo.action("refuse"), |_, _, _| {
        ActionResult::Refused("not now".into())
    });
    demo.action("chain").run(|cx, _| {
        cx.dispatch("demo.add", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    demo.action("chain_then_refuse").run(|cx, _| {
        cx.dispatch("demo.add", crate::chrome::PropMap::new());
        cx.dispatch("demo.refuse", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    demo.action("chain_kill").run(|cx, _| {
        cx.dispatch("kill_server", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    demo.action("chain_wipe").run(|cx, _| {
        cx.dispatch("demo.wipe", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    demo.action("chain_later").run(|cx, _| {
        cx.dispatch("demo.later", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    log
}

fn key_of(server: &ServerLoop, index: usize) -> SessionKey {
    server.sessions[index].service_ref().key
}

fn request(session: SessionKey, id: &str, line: &str) -> ExternalRequest {
    ExternalRequest {
        sender: Sender::new("alice"),
        session,
        request_id: id.into(),
        line: line.into(),
    }
}

fn pump(server: &mut ServerLoop) {
    server.pump(Instant::now());
}

/// Hand `requests` in through the service's own inbox and let the server take its turn once.
fn submit(server: &mut ServerLoop, log: &Log, requests: impl IntoIterator<Item = ExternalRequest>) {
    for request in requests {
        log.inbox().send(Msg::Submit(request)).unwrap();
    }
    pump(server);
}

fn refused(reason: &str) -> RequestOutcome {
    RequestOutcome::Refused(reason.into())
}

fn two_sessions(settings: &str) -> ServerLoop {
    let mut server = ServerLoop::from_config(
        &config(settings),
        ServerIdentity::for_tests(ServerKind::Background),
        ServerWaker::new(|| {}),
    )
    .unwrap()
    .with_session(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(1), LayoutOptions::default()),
    )
    .with_session(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(2), LayoutOptions::default()),
    );
    register_actions(&mut server);
    server
}

fn notified(window: &impl crate::server::wire::ClientLink, title: &str, reason: &str) -> bool {
    drain(window).iter().any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains(title) && format!("{view:?}").contains(reason))))
}

fn split(window: &impl crate::server::wire::ClientLink, id: u64) {
    window
        .send(ClientMsg::Act {
            action: crate::input::WmAction::SplitHorizontal,
            asker: super::tests::test_asker(),
            request_id: Some(id),
            terminal_owner: None,
        })
        .unwrap();
}

fn reply_changes(window: &impl crate::server::wire::ClientLink) -> Option<usize> {
    drain(window).into_iter().find_map(|message| match message {
        ServerMsg::Reply { changes, .. } => Some(changes.len()),
        _ => None,
    })
}

/// **One rule for every way in.** A window, a script (service action and built-in) and a helper's
/// request are all refused by the same policy with the same reason, and nothing runs; a script
/// is told `error: …` and a window is shown a notification.
#[test]
fn the_same_policy_judges_a_window_a_script_and_a_helper() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    let window = attached(&mut server);
    drain(&window);

    split(&window, 1);
    pump(&mut server);
    assert!(
        reply_changes(&window).is_some_and(|changes| changes > 0),
        "allowed, it splits"
    );
    assert_eq!(server.rpc("split_horizontal"), "ok");

    for action in ["demo.add", "split_horizontal"] {
        log.inbox().send(Msg::Deny(action)).unwrap();
    }
    pump(&mut server);
    log.ran.borrow_mut().clear();

    assert_eq!(
        rpc(&mut server, "demo.add").try_recv().unwrap(),
        "error: not permitted: demo.add is not allowed",
        "a script, service action"
    );
    assert_eq!(
        server.rpc("split_horizontal"),
        "error: not permitted: split_horizontal is not allowed",
        "a script, built-in"
    );

    window
        .send(ClientMsg::Invoke {
            intent: crate::chrome::Intent::new("demo.add"),
        })
        .unwrap();
    pump(&mut server);
    assert!(
        notified(&window, "demo.add was refused", "demo.add is not allowed"),
        "a window, service action"
    );
    split(&window, 2);
    pump(&mut server);
    let messages = drain(&window);
    assert!(messages.iter().any(|message| matches!(message, ServerMsg::Reply { request_id: 2, changes } if changes.is_empty())),
        "the window's request is answered with nothing changed");
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains("split_horizontal was refused")))),
        "a window, built-in"
    );

    // A plain extension action from a window is judged before it hands its follow-ups back.
    log.inbox().send(Msg::Deny("demo.chain")).unwrap();
    pump(&mut server);
    window
        .send(ClientMsg::Invoke {
            intent: crate::chrome::Intent::new("demo.chain"),
        })
        .unwrap();
    pump(&mut server);
    let messages = drain(&window);
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, ServerMsg::Dispatch { .. })),
        "its follow-ups are not handed back"
    );
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains("demo.chain was refused")))),
        "a window, plain extension action"
    );

    submit(
        &mut server,
        &log,
        [
            request(key, "r1", "demo.add"),
            request(key, "r2", "split_horizontal"),
        ],
    );
    assert_eq!(
        log.done(),
        [
            ("r1".into(), refused("demo.add is not allowed")),
            ("r2".into(), refused("split_horizontal is not allowed"))
        ],
        "a helper"
    );
    assert!(log.ran().is_empty(), "nothing ran");
}

/// **The requester is carried to every follow-up, and a follow-up is asked about as a follow-up.**
#[test]
fn the_requester_reaches_a_chains_follow_ups_and_a_chain_never_escalates() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(&mut server, &log, [request(key, "r1", "demo.chain")]);
    let who = |action: &str, parent: Option<&str>| Asked {
        requester: "demo.relay/alice".into(),
        action: action.into(),
        parent: parent.map(str::to_owned),
        session: Some(key),
    };
    assert_eq!(
        log.asked(),
        [who("demo.chain", None), who("demo.add", Some("demo.chain"))]
    );
    assert_eq!(
        log.ran(),
        ["add in 1 for Some(\"alice\")"],
        "the action saw who it was run for"
    );
    assert_eq!(
        log.done(),
        [("r1".into(), RequestOutcome::Done(Some("added".into())))]
    );

    // The same chain from a script is the user's, all the way down; nothing turns it into a sender.
    log.asked.borrow_mut().clear();
    assert_eq!(server.rpc("demo.chain"), "ok");
    let local = |action: &str, parent: Option<&str>| Asked {
        requester: "local".into(),
        ..who(action, parent)
    };
    assert_eq!(
        log.asked(),
        [
            local("demo.chain", None),
            local("demo.add", Some("demo.chain"))
        ]
    );

    // A policy that refuses the follow-up for this sender stops the chain there: the first step
    // had already run, so the request has failed, not been refused.
    log.inbox().send(Msg::Deny("demo.add")).unwrap();
    submit(&mut server, &log, [request(key, "r2", "demo.chain")]);
    assert_eq!(
        log.done()[1],
        (
            "r2".into(),
            RequestOutcome::Failed("demo.add is not allowed".into())
        )
    );
}

/// **A refusal part way through a chain is a failure, and what ran before stays done.**
#[test]
fn a_refusal_mid_chain_fails_the_request_and_undoes_nothing() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(
        &mut server,
        &log,
        [request(key, "r1", "demo.chain_then_refuse")],
    );
    assert_eq!(
        log.done(),
        [("r1".into(), RequestOutcome::Failed("not now".into()))]
    );
    assert_eq!(log.ran().len(), 1, "the first follow-up ran");
}

/// **A rule that changed before a queued request ran applies to it.**
#[test]
fn a_revocation_applies_to_a_request_already_queued() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    // One turn takes the submit and then the revocation, so the request is judged after it.
    log.inbox()
        .send(Msg::Submit(request(key, "r1", "demo.add")))
        .unwrap();
    log.inbox().send(Msg::Deny("demo.add")).unwrap();
    pump(&mut server);
    assert_eq!(log.submitted.borrow().len(), 1);
    assert!(log.submitted.borrow()[0].is_ok(), "it was taken");
    assert_eq!(
        log.done(),
        [("r1".into(), refused("demo.add is not allowed"))]
    );
    assert!(log.ran().is_empty());
}

/// **A request runs in the session it names — never in another, never in a made-up one.**
#[test]
fn the_session_is_explicit_and_never_substituted() {
    let log = declare(true, false);
    let mut server = two_sessions("");
    let (first, second) = (key_of(&server, 0), key_of(&server, 1));
    submit(
        &mut server,
        &log,
        [
            request(first, "r1", "demo.add"),
            request(second, "r2", "demo.add"),
        ],
    );
    assert_eq!(
        log.ran(),
        [
            "add in 1 for Some(\"alice\")",
            "add in 2 for Some(\"alice\")"
        ],
        "the most recent session is 2"
    );

    submit(
        &mut server,
        &log,
        [request(SessionKey::generate(), "r3", "demo.add")],
    );
    assert_eq!(log.submitted.borrow()[2], Err(SubmitError::UnknownSession));
    assert_eq!(log.ran().len(), 2, "and it ran nowhere");

    // A session deleted before its request arrives is no longer known: it is not moved elsewhere.
    log.inbox()
        .send(Msg::Submit(request(first, "r4", "demo.add")))
        .unwrap();
    let name = server.sessions[0].info().name;
    server
        .execute_session(SessionCommand::Delete { name })
        .unwrap();
    submit(&mut server, &log, []);
    assert!(
        log.submitted.borrow()[3].is_err(),
        "the key is no longer known"
    );
    assert_eq!(log.ran().len(), 2);
}

/// **The queue answers at once when it is full, and refuses an id that is still open.**
#[test]
fn a_full_queue_and_a_duplicate_open_id_are_turned_away() {
    let log = declare(true, false);
    let mut server = server(&config("[settings.services]\nrequest_queue_capacity = 2"));
    let key = key_of(&server, 0);
    // `demo.later` stays open: its answer has not come.
    submit(
        &mut server,
        &log,
        [
            request(key, "a", "demo.later"),
            request(key, "a", "demo.later"),
            request(key, "b", "demo.later"),
            request(key, "c", "demo.later"),
        ],
    );
    let results: Vec<_> = log
        .submitted
        .borrow()
        .iter()
        .map(|result| result.clone().map(|r| r.request_id().to_owned()))
        .collect();
    assert_eq!(
        results,
        [
            Ok("a".into()),
            Err(SubmitError::DuplicateId),
            Ok("b".into()),
            Err(SubmitError::QueueFull)
        ]
    );

    // Once they end, the room and the ids are free again.
    log.inbox().send(Msg::Settle).unwrap();
    pump(&mut server);
    assert_eq!(log.done().len(), 2);
    submit(
        &mut server,
        &log,
        [request(key, "a", "demo.add"), request(key, "c", "demo.add")],
    );
    assert!(
        log.submitted.borrow()[4].is_ok() && log.submitted.borrow()[5].is_ok(),
        "a finished id is forgotten"
    );
}

/// **A line that is empty or longer than a frame is refused at once.**
#[test]
fn an_empty_or_oversized_line_is_refused_at_once() {
    let log = declare(true, false);
    let mut server = server(&config("[settings.connection]\nmax_frame_bytes = 4096"));
    let key = key_of(&server, 0);
    let long = format!("demo.add {}", "x".repeat(5000));
    submit(
        &mut server,
        &log,
        [request(key, "r1", "  "), request(key, "r2", &long)],
    );
    assert!(matches!(
        log.submitted.borrow()[0],
        Err(SubmitError::Refused(_))
    ));
    assert!(
        matches!(&log.submitted.borrow()[1], Err(SubmitError::Refused(reason)) if reason.contains("limit"))
    );
    assert!(log.ran().is_empty());
}

/// **A deferred answer ends the request it belongs to, correlated by what `submit` returned.**
#[test]
fn a_deferred_result_arrives_with_the_request_it_belongs_to() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(&mut server, &log, [request(key, "r1", "demo.chain_later")]);
    assert!(
        log.done().is_empty(),
        "accepted, not over: a ticket is owed"
    );
    log.inbox().send(Msg::Settle).unwrap();
    pump(&mut server);
    let sent = log.submitted.borrow()[0].clone().unwrap();
    assert_eq!(
        log.done.borrow().as_slice(),
        [(sent, RequestOutcome::Done(Some("settled".into())))]
    );
    pump(&mut server);
    assert_eq!(log.done().len(), 1, "told once");
}

/// **At shutdown a request that never started is refused and one that started is unknown — and
/// neither is run again by the next server.**
#[test]
fn shutdown_ends_open_requests_and_nothing_runs_again() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(&mut server, &log, [request(key, "started", "demo.later")]);
    // Taken by the service while it stops, and never given a turn to run.
    log.inbox()
        .send(Msg::Submit(request(key, "waiting", "demo.add")))
        .unwrap();
    server.shutdown();
    assert_eq!(
        log.done(),
        [
            ("started".into(), RequestOutcome::Unknown),
            ("waiting".into(), refused("server stopped, not executed"))
        ]
    );
    assert!(
        log.ran().is_empty(),
        "the one that never started did not run"
    );
    server.shutdown();
    assert_eq!(log.done().len(), 2, "each told once");

    let again = declare(true, false);
    let mut restarted = super::service_tests::server(&config(""));
    pump(&mut restarted);
    assert!(
        again.done().is_empty() && again.ran().is_empty(),
        "a new server starts with nothing to redo"
    );
}

/// **With no policy declared, the user's requests go through and a service's are refused.** A
/// service with no policy of its own is refused even when another declares one.
#[test]
fn without_a_policy_local_passes_and_external_is_refused() {
    let log = declare(false, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    assert_eq!(rpc(&mut server, "demo.add").try_recv().unwrap(), "added");
    assert_eq!(server.rpc("split_horizontal"), "ok");
    submit(&mut server, &log, [request(key, "r1", "demo.add")]);
    assert!(
        matches!(&log.submitted.borrow()[0], Err(SubmitError::Refused(reason)) if reason.contains("no action policy"))
    );
    assert_eq!(log.ran().len(), 1, "only the script's ran");

    let log = declare(false, true);
    let mut server = super::service_tests::server(&config(""));
    let key = key_of(&server, 0);
    submit(&mut server, &log, [request(key, "r1", "demo.add")]);
    assert!(
        matches!(&log.submitted.borrow()[0], Err(SubmitError::Refused(_))),
        "another service's policy does not stand in for its own"
    );
    assert_eq!(
        rpc(&mut server, "demo.add").try_recv().unwrap(),
        "added",
        "and the user's request is judged by it, and allowed"
    );
}

const NO_CONFIRMATION: &str =
    "'demo.wipe' asks for a confirmation, which a request handed in by a service cannot give";

/// **An action that declares a confirmation is never run for a sender — even when the user turned
/// that confirmation off for themselves** — and the same holds for a chain's follow-up. The user's
/// own call is judged by their settings as before.
#[test]
fn a_declared_confirmation_is_refused_for_a_sender_whatever_the_users_settings() {
    let log = declare(true, false);
    let off = config("[confirm]\n\"demo.wipe\" = false\nterminate_session = false");
    let mut server = server(&off);
    let key = key_of(&server, 0);
    submit(
        &mut server,
        &log,
        [
            request(key, "direct", "demo.wipe"),
            request(key, "chained", "demo.chain_wipe"),
            request(key, "built-in", "terminate_session session=nobody"),
        ],
    );
    assert_eq!(
        log.done(),
        [
            ("direct".into(), refused(NO_CONFIRMATION)),
            (
                "chained".into(),
                RequestOutcome::Failed(NO_CONFIRMATION.into())
            ),
            (
                "built-in".into(),
                refused(
                    "'terminate_session' asks for a confirmation, which a request handed in by a service cannot give"
                )
            ),
        ],
        "refused with the prompt off; a follow-up fails the chain that had started"
    );
    assert!(log.ran().is_empty());

    assert_eq!(
        rpc(&mut server, "demo.wipe").try_recv().unwrap(),
        "wiped",
        "the user, with that prompt off, runs it"
    );
    assert_eq!(
        server.rpc("demo.chain_wipe"),
        "ok",
        "and so does their chain"
    );
    assert_eq!(log.ran(), ["wipe", "wipe"]);

    // With the prompt on, the user is refused as before, and so is the sender.
    let log = declare(true, false);
    let mut server = super::service_tests::server(&config(""));
    let key = key_of(&server, 0);
    assert_eq!(
        rpc(&mut server, "demo.wipe").try_recv().unwrap(),
        "error: action 'demo.wipe' is not allowed right now"
    );
    submit(&mut server, &log, [request(key, "r1", "demo.wipe")]);
    assert_eq!(log.done(), [("r1".into(), refused(NO_CONFIRMATION))]);
    assert!(log.ran().is_empty());
}

/// **A read is judged like anything else**, for the user and for a sender: the policy is asked about
/// listing sessions, and its refusal stops it.
#[test]
fn a_read_action_is_judged_for_the_user_and_for_a_sender() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    assert!(
        server.rpc("list_sessions").starts_with('['),
        "allowed, the user reads the list"
    );
    submit(&mut server, &log, [request(key, "r1", "list_sessions")]);
    assert_eq!(
        log.done()[0].0,
        "r1",
        "a sender is asked about too, and a session list is not one it can run"
    );
    assert!(
        matches!(&log.done()[0].1, RequestOutcome::Refused(reason) if reason.contains("no runnable owner"))
    );
    let asked: Vec<_> = log
        .asked()
        .into_iter()
        .map(|asked| (asked.requester, asked.action))
        .collect();
    assert_eq!(
        asked,
        [
            ("local".into(), "list_sessions".into()),
            ("demo.relay/alice".into(), "list_sessions".into())
        ]
    );

    log.inbox().send(Msg::Deny("list_sessions")).unwrap();
    pump(&mut server);
    assert_eq!(
        server.rpc("list_sessions"),
        "error: not permitted: list_sessions is not allowed"
    );
    submit(&mut server, &log, [request(key, "r2", "list_sessions")]);
    assert_eq!(
        log.done()[1],
        ("r2".into(), refused("list_sessions is not allowed")),
        "the policy's word, not the catalog's"
    );
}

/// **A query is not an action**: it answers from the catalog and no policy would see it, so it
/// cannot be handed in.
#[test]
fn a_query_cannot_be_handed_in() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(
        &mut server,
        &log,
        [
            request(key, "a", "list-actions"),
            request(key, "b", "  Describe-Action demo.add"),
        ],
    );
    assert!(log.submitted.borrow().iter().all(|result| matches!(result, Err(SubmitError::Refused(reason)) if reason.contains("not an action"))));
    assert!(log.done().is_empty() && log.asked().is_empty() && log.ran().is_empty());
}

/// **A request handed in never reaches a window.** A script's line for an action only a window can
/// run is sent to the attached window; the same line handed in by a service is not run and not sent.
#[test]
fn a_request_handed_in_is_never_forwarded_to_a_window() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    let window = attached(&mut server);
    drain(&window);

    let _ = rpc(&mut server, "focus_left");
    assert!(
        drain(&window)
            .iter()
            .any(|message| matches!(message, ServerMsg::Rpc { .. })),
        "the script's line goes to the window"
    );

    submit(&mut server, &log, [request(key, "r1", "focus_left")]);
    assert!(
        matches!(&log.done()[0], (id, RequestOutcome::Refused(reason)) if id == "r1" && reason.contains("no runnable owner"))
    );
    assert!(
        !drain(&window)
            .iter()
            .any(|message| matches!(message, ServerMsg::Rpc { .. })),
        "nothing was sent to the window"
    );
}

/// **What belongs to no session is asked about with no session**, and everything else with its own.
#[test]
fn a_global_action_is_asked_with_no_session() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    assert!(server.rpc("list_sessions").starts_with('['));
    assert_eq!(server.rpc("split_horizontal"), "ok");
    let asked = log.asked();
    assert_eq!(
        (asked[0].action.as_str(), asked[0].session),
        ("list_sessions", None)
    );
    assert_eq!(
        (asked[1].action.as_str(), asked[1].session),
        ("split_horizontal", Some(key))
    );
    assert_eq!(asked[0].requester, "local");
}

fn kill(window: &impl crate::server::wire::ClientLink, id: u64) {
    window
        .send(ClientMsg::Act {
            action: crate::input::WmAction::KillServer,
            asker: super::tests::test_asker(),
            request_id: Some(id),
            terminal_owner: None,
        })
        .unwrap();
}

/// **Stopping the server is asked about like anything else**: a policy that forbids `kill_server`
/// stops the script and the window alike, the server stays up and the window is told why; allowed
/// by a policy, or with none, both stop it.
#[test]
fn a_policy_can_forbid_kill_server_for_a_script_and_a_window() {
    let log = declare(true, false);
    let mut server = server(&config(""));
    let window = attached(&mut server);
    drain(&window);
    log.inbox().send(Msg::Deny("kill_server")).unwrap();
    pump(&mut server);

    assert_eq!(
        server.rpc("kill_server"),
        "error: not permitted: kill_server is not allowed",
        "a script"
    );
    kill(&window, 1);
    pump(&mut server);
    assert!(
        notified(
            &window,
            "kill_server was refused",
            "kill_server is not allowed"
        ),
        "a window"
    );
    assert!(!server.wants_shutdown(), "the server is still up");
    assert_eq!(server.clients.len(), 1, "and the window is still attached");
    assert_eq!(server.rpc("split_horizontal"), "ok", "and answering");
    assert_eq!(
        log.asked()
            .iter()
            .filter(|asked| asked.action == "kill_server" && asked.requester == "local")
            .count(),
        1
    );
    assert_eq!(
        log.asked()
            .iter()
            .filter(|asked| asked.action == "kill_server" && asked.requester == "window 1")
            .count(),
        1
    );

    // The policy allows it: the script stops the server and is told so.
    let log = declare(true, false);
    let mut server = super::service_tests::server(&config(""));
    assert_eq!(server.rpc("kill_server"), "heca: server stopped\n");
    assert!(server.wants_shutdown());
    assert_eq!(
        log.asked()
            .iter()
            .filter(|asked| asked.action == "kill_server")
            .count(),
        1
    );

    // The policy allows it: the window's request stops the server.
    declare(true, false);
    let mut server = super::service_tests::server(&config(""));
    let window = attached(&mut server);
    kill(&window, 1);
    pump(&mut server);
    assert!(server.wants_shutdown(), "a window");
}

/// **With no policy anywhere nothing changes**: the palette's `kill_server` stops the server.
#[test]
fn without_a_policy_a_window_stops_the_server() {
    declare(false, false);
    let mut server = super::service_tests::server(&config(""));
    let window = attached(&mut server);
    kill(&window, 1);
    pump(&mut server);
    assert!(server.wants_shutdown());
    assert!(
        server.clients.is_empty(),
        "every window is dropped with the server"
    );
}

/// **A sender can never stop the server**, whatever the policy says: handed in directly or reached by a
/// chain it started, it is not run.
#[test]
fn a_sender_can_never_stop_the_server() {
    let log = declare(true, true);
    let mut server = server(&config(""));
    let key = key_of(&server, 0);
    submit(
        &mut server,
        &log,
        [
            request(key, "r1", "kill_server"),
            request(key, "r2", "demo.chain_kill"),
        ],
    );
    let done = log.done();
    assert!(
        matches!(&done[0].1, RequestOutcome::Refused(reason) if reason.contains("no runnable owner")),
        "direct: {done:?}"
    );
    assert!(
        matches!(&done[1].1, RequestOutcome::Failed(reason) if reason.contains("kill_server")),
        "chain: {done:?}"
    );
    assert!(!server.wants_shutdown(), "the server is still up");
    assert!(
        log.asked().iter().any(|asked| asked.action == "kill_server"
            && asked.parent.as_deref() == Some("demo.chain_kill")),
        "the follow-up was put to the policy first"
    );
}
