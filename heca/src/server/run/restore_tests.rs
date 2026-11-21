//! **Sessions kept across a restart of the server**: what comes back, what stays stopped, and what
//! never comes back. Every server here starts no process and reads none of the user's files — its
//! terminals are fakes that remember how they were asked for, and its data folder is a temporary one.

use super::service_tests::{config, drain};
use super::*;
use crate::app::backend_factory::LaunchSettings;
use crate::app::backend_store::{Program, SpawnError, TerminalSpec};
use crate::server::identity::{ServerIdentity, ServerKind};
use crate::server::wire::ClientLink;
use heca_core::agent::{AgentDriver, AgentError, AgentInput, AgentLaunchContext, AgentSetup, AgentState, AgentStatus};
use heca_core::backend::{FakeBackend, PaneBackend, ProcessLaunch};
use heca_core::layout::{Pane, PaneId};
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub(super) const SHELL: &str = "/usr/bin/test-shell";
pub(super) const CELL: (f32, f32) = (8.0, 16.0);

thread_local! {
    /// Every terminal the fake machine was asked to start, in order.
    static STARTED: RefCell<Vec<TerminalSpec>> = const { RefCell::new(Vec::new()) };
    /// The native session identity a harness started now reports, and the driver it reports under.
    static HARNESS: RefCell<(Option<String>, String)> = RefCell::new((None, "demo.resuming".into()));
    static CONFIG: RefCell<Option<heca_config::loader::AppConfig>> = const { RefCell::new(None) };
}

/// A terminal that starts nothing, and reports the process a real one would record. A program
/// called `harness` reports the harness identity [`HARNESS`] says.
fn recording(_: &LaunchSettings<'_>, spec: &TerminalSpec) -> Result<Box<dyn PaneBackend>, SpawnError> {
    STARTED.with(|started| started.borrow_mut().push(spec.clone()));
    let launch = match &spec.program {
        Program::Recorded(launch) => launch.clone(),
        Program::Shell => ProcessLaunch { executable: SHELL.into(), arguments: Vec::new(), cwd: spec.cwd.clone() },
        Program::Command(command) => ProcessLaunch {
            executable: SHELL.into(), arguments: vec!["-c".into(), command.into()], cwd: spec.cwd.clone(),
        },
    };
    let mut runtime = PaneRuntime { status: ProcessStatus::Running, ..Default::default() };
    if launch.executable.file_name().is_some_and(|name| name == "harness") {
        let (native, driver) = HARNESS.with(|harness| harness.borrow().clone());
        let mut agent = AgentState::new(driver, "Demo", AgentStatus::new("working"));
        agent.launch = Some(launch.clone());
        agent.session_id = native;
        runtime.agent = Some(agent);
    }
    let mut backend = FakeBackend::new(spec.grid.0, spec.grid.1);
    backend.set_launch(launch);
    backend.set_runtime(runtime);
    Ok(Box::new(backend))
}

fn project(_: &Path) -> heca_config::loader::AppConfig {
    CONFIG.with(|config| config.borrow().clone().expect("a test booted a server"))
}

/// A driver `demo.<id>` that continues a conversation by adding `--resume <id>`, or cannot.
struct Driver {
    id: &'static str,
    resumes: bool,
}

impl AgentDriver for Driver {
    fn id(&self) -> &str { self.id }
    fn name(&self) -> &str { "Demo" }
    fn programs(&self) -> &[String] { &[] }
    fn prepare(&self, _: &AgentLaunchContext) -> Result<AgentSetup, AgentError> { Ok(AgentSetup::default()) }
    fn read_status(&self, _: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> { Ok(None) }
    fn resume(&self, original: &ProcessLaunch, session_id: &str) -> Option<ProcessLaunch> {
        self.resumes.then(|| {
            let mut launch = original.clone();
            launch.arguments.extend(["--resume".into(), session_id.into()]);
            launch
        })
    }
}

/// Declare `demo.resuming` (which can resume) and `demo.plain` (which cannot), for this test.
pub(super) fn declare_drivers() {
    let demo = crate::entry::extension("demo");
    demo.agent_driver(Driver { id: "resuming", resumes: true });
    demo.agent_driver(Driver { id: "plain", resumes: false });
}

/// What a config must say for those two drivers to be on.
pub(super) const DRIVERS_ON: &str =
    "[settings.agents.programs.\"demo.resuming\"]\nenabled = true\n[settings.agents.programs.\"demo.plain\"]\nenabled = true\n";

pub(super) fn set_harness(native: Option<&str>, driver: &str) {
    HARNESS.with(|harness| *harness.borrow_mut() = (native.map(str::to_owned), driver.to_owned()));
}

/// The folder a server called `default` keeps its own files in, under `root`.
pub(super) fn server_folder(root: &Path) -> PathBuf {
    root.join("servers").join("default")
}

pub(super) fn saved_file(root: &Path) -> PathBuf {
    server_folder(root).join("heca.sessions.json")
}

/// A server as the background server starts one, keeping its files under `root`.
pub(super) fn boot(root: &Path, text: &str) -> ServerLoop {
    let config = config(text);
    CONFIG.with(|slot| *slot.borrow_mut() = Some(config.clone()));
    let identity = ServerIdentity::claim_in(ServerKind::Background, Some(root.join("servers")));
    ServerLoop::boot(&config, identity, ServerWaker::new(|| {}), Machine { launch: recording, project_config: project })
        .unwrap()
}

/// Stop `server` the way the user does — everything is written — and start the next one.
pub(super) fn restart(mut server: ServerLoop, root: &Path, text: &str) -> ServerLoop {
    server.shutdown();
    drop(server);
    boot(root, text)
}

pub(super) fn started() -> Vec<TerminalSpec> {
    STARTED.with(|started| started.borrow().clone())
}

pub(super) fn forget_started() {
    STARTED.with(|started| started.borrow_mut().clear());
}

/// What was started in `cwd`, in order.
pub(super) fn started_in(cwd: &Path) -> Vec<Program> {
    started().into_iter().filter(|spec| spec.cwd.as_deref() == Some(cwd)).map(|spec| spec.program).collect()
}

pub(super) fn make(server: &mut ServerLoop, folder: &Path, name: &str) -> SessionInfo {
    match server
        .execute_session(SessionCommand::Create { folder: folder.into(), name: Some(name.into()), label: Some(format!("{name} label")) })
        .unwrap()
    {
        SessionReply::Updated(info) => info,
        other => panic!("create returned {other:?}"),
    }
}

pub(super) fn session<'a>(server: &'a mut ServerLoop, name: &str) -> &'a mut SessionRuntime {
    let index = server.named(name).unwrap_or_else(|_| panic!("no session called {name}"));
    &mut server.sessions[index]
}

pub(super) fn status(server: &ServerLoop, name: &str) -> SessionStatus {
    server.sessions[server.named(name).unwrap()].info().status
}

/// The titles of what a window attached to `name` is shown as notifications.
pub(super) fn notices(server: &mut ServerLoop, name: &str) -> Vec<(String, Option<String>)> {
    shown_to_a_window(server, name).into_iter().map(|(title, body, _)| (title, body)).collect()
}

/// The titles of the notifications shown that stay until dismissed — those with no auto-dismiss timer.
pub(super) fn lasting(server: &mut ServerLoop, name: &str) -> Vec<String> {
    shown_to_a_window(server, name).into_iter().filter(|(_, _, timer)| timer.is_none()).map(|(title, ..)| title).collect()
}

/// Each notification a window attached to `name` is shown: title, body and its auto-dismiss time.
fn shown_to_a_window(server: &mut ServerLoop, name: &str) -> Vec<(String, Option<String>, Option<std::time::Duration>)> {
    let window = server.connect();
    window.send(ClientMsg::AttachTo { name: name.into() }).unwrap();
    server.pump(Instant::now());
    let mut shown = Vec::new();
    for message in drain(&window) {
        if let ServerMsg::Notifications(views) = message {
            shown = views.into_iter().map(|view| (view.title, view.body, view.auto_dismiss)).collect();
        }
    }
    shown
}

/// Put a running terminal for `pane` on `session`, started as `program` in `cwd`.
pub(super) fn start_terminal(state: &mut ServerState, pane: PaneId, program: Program, cwd: &Path) {
    let spec = TerminalSpec { program, cwd: Some(cwd.to_owned()), grid: (80, 24) };
    state.terminals().start(pane, spec, CELL).unwrap();
}

/// Three workspaces' worth of structure: a named workspace of two columns, one holding a named,
/// shortened pane over a second, and a second workspace. Every pane has a terminal. Returns the
/// panes, in the order they were made.
pub(super) fn shape(runtime: &mut SessionRuntime, folder: &Path) -> Vec<PaneId> {
    let mut panes = Vec::new();
    runtime.arrange(|session, state| {
        panes.push(session.workspaces[0].panes().next().unwrap().id);
        let below = session.add_pane_to_column(0, 0, 600.0).unwrap().pane;
        let beside = session.add_column(0, 1).unwrap().pane;
        session.workspaces[0].name = Some("work".into());
        let pane = session.workspaces[0].find_pane_mut(below).unwrap();
        pane.custom_name = Some("logs".into());
        pane.height_share = Some(0.3);
        session.add_workspace();
        let second = session.workspaces.len() - 1;
        let extra = PaneId(session.next_id());
        let monitor = session.add_column_with(second, 0, Pane::new(extra, "monitor")).unwrap().pane;
        for pane in [below, beside, monitor] {
            start_terminal(state, pane, Program::Shell, folder);
            panes.push(pane);
        }
    });
    panes
}

/// **Two sessions with several workspaces and panes come back as they were**: the same names,
/// folders, labels, layout and shares, the same last view and focus — and the same key, which is
/// what an extension keeps its own data against.
#[test]
fn two_sessions_come_back_after_a_restart_exactly_as_they_were() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    let alpha = make(&mut server, &folders.path().join("alpha"), "Alpha");
    let beta = make(&mut server, &folders.path().join("beta"), "Beta");
    let panes = shape(session(&mut server, "Alpha"), &alpha.folder.clone().unwrap());

    // A window looked at the second workspace's pane and reported a view while followed.
    let window = server.connect();
    window.send(ClientMsg::AttachTo { name: "Alpha".into() }).unwrap();
    window.send(ClientMsg::ReportPresence { workspace: 1, pane: panes.last().copied() }).unwrap();
    let view = heca_core::layout::WindowView::new(heca_core::layout::Size::new(800.0, 600.0), 1.0).snapshot();
    window.send(ClientMsg::ReportView(view.clone())).unwrap();
    server.pump(Instant::now());
    drop(window);

    let before: Vec<_> = ["Alpha", "Beta"].map(|name| {
        let runtime = session(&mut server, name);
        (runtime.info(), runtime.service_ref().key, runtime.content(), runtime.saved())
    }).into();
    assert!(before[0].3.content.workspaces.len() > 1, "the test session has several workspaces");
    let workspace_before = session(&mut server, "Alpha").service_ref().workspace;
    assert_ne!(before[0].1, before[1].1);

    forget_started();
    let mut server = restart(server, root.path(), "");
    for (name, (info, key, content, saved)) in ["Alpha", "Beta"].into_iter().zip(&before) {
        let runtime = session(&mut server, name);
        assert_eq!(runtime.service_ref().key, *key, "{name}: the same key");
        assert_eq!(runtime.content(), *content, "{name}: the same workspaces, columns, panes, names and shares");
        let now = runtime.info();
        assert_eq!(
            (now.id, &now.name, &now.folder, &now.label, now.status),
            (info.id, &info.name, &info.folder, &info.label, SessionStatus::Active),
            "{name}: who it is, and running again",
        );
        let restored = runtime.saved();
        assert_eq!((restored.focus, restored.view.as_ref()), (saved.focus, saved.view.as_ref()), "{name}: last view");
    }
    let restored = session(&mut server, "Alpha").saved();
    assert_eq!(restored.focus, Some(crate::server::persist::Focus { workspace: 1, pane: panes.last().copied() }));
    assert_eq!(restored.view, Some(view));
    // The workspace an action is asked from keeps its lasting key across the restart.
    let workspace_after = session(&mut server, "Alpha").service_ref().workspace;
    assert!(workspace_before.is_some(), "a window showed a workspace, so an action has one");
    assert_eq!(workspace_after, workspace_before, "the same workspace key after a restart");
    assert_eq!(started().len(), 5, "each of the five terminals was started again");
    assert_eq!(beta.id, server.sessions[server.named("Beta").unwrap()].info().id);
    // A window with no name opens the session that was used last — Alpha, which the window used after
    // Beta was made — and the next new one is numbered past both.
    assert_eq!(server.last_session, Some(alpha.id));
    assert!(server.next_session > alpha.id.0.max(beta.id.0));
}

/// **A program's arguments come back exactly** — spaces, quotes, an empty argument and, where the
/// system allows it, bytes that are not text — because they are handed back as an argument list,
/// never as a command line to be split again.
#[test]
fn a_programs_exact_arguments_survive_a_restart() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    let folder = folders.path().join("work");
    make(&mut server, &folder, "Work");
    let folder = folder.canonicalize().unwrap();
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut arguments: Vec<std::ffi::OsString> =
        vec!["--name".into(), "two words".into(), "it's \"quoted\" $HOME `x`".into(), "".into(), "tab\there".into(), "ünï".into()];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        arguments.push(std::ffi::OsString::from_vec(vec![b'a', 0xff, b'b']));
    }
    let program = ProcessLaunch { executable: "/opt/tools/serve".into(), arguments, cwd: Some(folder.clone()) };
    session(&mut server, "Work").arrange(|session, state| {
        let pane = session.add_column(0, 1).unwrap().pane;
        start_terminal(state, pane, Program::Recorded(program.clone()), &folder);
    });
    forget_started();
    let mut server = restart(server, root.path(), "");
    let relaunched = started().into_iter().find_map(|spec| match spec.program {
        Program::Recorded(launch) if launch.executable == program.executable => Some(launch),
        _ => None,
    });
    assert_eq!(relaunched, Some(program));
    let again = session(&mut server, "Work").recorded_terminals();
    assert!(again.iter().any(|terminal| terminal.launch.as_ref().is_some_and(|launch| launch.executable.as_path() == Path::new("/opt/tools/serve"))));
}

/// **A stopped session stays stopped** through a restart, whatever the switches say — until Restart
/// is asked for, which starts it the way the switches decide.
#[test]
fn a_stopped_session_stays_stopped_until_it_is_restarted() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    make(&mut server, &folders.path().join("a"), "A");
    make(&mut server, &folders.path().join("b"), "B");
    server.execute_session(SessionCommand::Terminate { name: "A".into() }).unwrap();
    let key = session(&mut server, "A").service_ref().key;
    forget_started();
    let mut server = restart(server, root.path(), "[settings]\nresume_programs = true\nresume_harness = true");
    assert_eq!(status(&server, "A"), SessionStatus::Terminated);
    assert_eq!(status(&server, "B"), SessionStatus::Active);
    assert_eq!(started().len(), 1, "only B's terminal was started");
    assert_eq!(session(&mut server, "A").service_ref().key, key);

    forget_started();
    server.execute_session(SessionCommand::Restart { name: "A".into() }).unwrap();
    assert_eq!(status(&server, "A"), SessionStatus::Active);
    assert_eq!(started().len(), 1, "Restart started A's terminal again");
    assert_eq!(session(&mut server, "A").service_ref().key, key, "and it is still the same session");
}

/// **A deleted session never comes back** — it is out of the file as soon as it is deleted, not at
/// the next write.
#[test]
fn a_deleted_session_does_not_come_back() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    make(&mut server, &folders.path().join("a"), "Doomed");
    make(&mut server, &folders.path().join("b"), "Kept");
    let key = session(&mut server, "Doomed").service_ref().key.to_string();
    assert!(std::fs::read_to_string(saved_file(root.path())).unwrap().contains(&key));
    server.execute_session(SessionCommand::Delete { name: "Doomed".into() }).unwrap();
    assert!(!std::fs::read_to_string(saved_file(root.path())).unwrap().contains(&key), "gone from the file at once");
    let server = restart(server, root.path(), "");
    assert!(server.named("Doomed").is_err());
    assert!(server.named("Kept").is_ok());
    assert_eq!(server.sessions.len(), 1);
}

/// **A session made, renamed, stopped or started is written at once**: a server that dies right
/// after still has it — and still has what is later, once the delay has passed.
#[test]
fn lifecycle_changes_are_written_at_once_and_other_changes_after_the_delay() {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let text = "[settings]\nsession_save_delay_ms = 1000";
    let mut server = boot(root.path(), text);
    make(&mut server, &folders.path().join("a"), "A");
    let read = || std::fs::read_to_string(saved_file(root.path())).unwrap();
    assert!(read().contains("\"A\""), "created: written at once");
    server.execute_session(SessionCommand::Rename { name: "A".into(), new_name: "Renamed".into() }).unwrap();
    assert!(read().contains("\"Renamed\""));
    server.execute_session(SessionCommand::Terminate { name: "Renamed".into() }).unwrap();
    assert!(read().contains("\"terminated\""));

    // A change that is not a lifecycle one is written when the delay has passed, and not before.
    server.execute_session(SessionCommand::Restart { name: "Renamed".into() }).unwrap();
    session(&mut server, "Renamed").arrange(|session, _| { session.workspaces[0].name = Some("moved".into()); });
    let start = Instant::now();
    server.pump(start);
    assert!(!read().contains("moved"), "a turn that changed something only says a write is due");
    server.pump(start + Duration::from_millis(500));
    assert!(!read().contains("moved"), "not before the delay");
    server.pump(start + Duration::from_millis(1100));
    assert!(read().contains("moved"), "once the delay has passed");
    assert_eq!(server.store.as_ref().unwrap().next_wake(), None, "and an idle server wakes for nothing");
}

/// **The last write is at shutdown, from what was running — and a second shutdown cannot replace it
/// with what is left once the processes are gone.**
#[test]
fn shutdown_writes_what_ran_and_a_second_shutdown_changes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    make(&mut server, &folders.path().join("a"), "A");
    server.shutdown();
    let written = std::fs::read_to_string(saved_file(root.path())).unwrap();
    assert!(written.contains("test-shell"), "the terminal that was running is in it");
    server.shutdown();
    assert_eq!(std::fs::read_to_string(saved_file(root.path())).unwrap(), written);
}

/// **Two servers never write the same file**: a second server on the same data folder has a folder of
/// its own.
#[test]
fn two_servers_keep_their_sessions_in_different_files() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut first = boot(root.path(), "");
    let identity = ServerIdentity::claim_in(ServerKind::Standalone, Some(root.path().join("servers")));
    let mut second = ServerLoop::boot(&config(""), identity, ServerWaker::new(|| {}), Machine { launch: recording, project_config: project }).unwrap();
    make(&mut first, &folders.path().join("a"), "First");
    make(&mut second, &folders.path().join("b"), "Second");
    let mine = std::fs::read_to_string(saved_file(root.path())).unwrap();
    let theirs = std::fs::read_to_string(root.path().join("servers").join("standalone-1").join("heca.sessions.json")).unwrap();
    assert!(mine.contains("First") && !mine.contains("Second"));
    assert!(theirs.contains("Second") && !theirs.contains("First"));
}

/// **With saving off nothing is written or read, and a file left from before is removed** — it
/// could otherwise bring back, later, what was deleted in between.
#[test]
fn with_saving_off_nothing_is_kept_and_an_old_file_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    make(&mut server, &folders.path().join("a"), "A");
    server.shutdown();
    drop(server);
    assert!(saved_file(root.path()).exists());
    let mut server = boot(root.path(), "[settings]\nsave_sessions = false");
    assert!(server.sessions.is_empty(), "nothing was read");
    assert!(!saved_file(root.path()).exists(), "and the old file is gone");
    make(&mut server, &folders.path().join("b"), "B");
    server.shutdown();
    assert!(!saved_file(root.path()).exists(), "nothing was written");
}
