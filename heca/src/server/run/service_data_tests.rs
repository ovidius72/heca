//! What a service is given besides its inbox: a folder of its own, and its own table of the
//! configuration — driven through real servers, the way an entry point starts them.

use super::service_tests::{attached, config, drain, rpc, server, server_as};
use super::*;
use crate::entry::service::{ProjectCx, Projection, Service, ServiceCx, ServiceResult};
use crate::server::identity::{ServerIdentity, ServerKind};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// What an extension keeps in `[extensions.<name>]`. Its defaults are 7, so a read that fell back
/// to a default cannot pass for one that read the table.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(default)]
struct Limits {
    max: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max: 7 }
    }
}

/// Shows each window the `max` its session's configuration gives this extension.
struct Probe;

impl Service for Probe {
    type Message = ();
    type Snapshot = String;
    type Delta = String;
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> {
        Ok(())
    }

    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<String, String>> {
        let value = cx.extension_config::<Limits>().map_or_else(|error| error.to_string(), |limits| limits.max.to_string());
        Some(Projection::Snapshot { revision: 0, value })
    }
}

/// What one service saw when it started.
struct Start {
    service: String,
    dir: PathBuf,
    /// Its table read as `Limits`.
    limits: Result<Limits, String>,
}

/// Every start of every service, oldest first.
#[derive(Default, Clone)]
struct Starts(Rc<RefCell<Vec<Start>>>);

impl Starts {
    fn latest(&self, service: &str) -> (PathBuf, Result<Limits, String>) {
        let all = self.0.borrow();
        let start = all.iter().rev().find(|start| start.service == service).expect("started");
        (start.dir.clone(), start.limits.as_ref().map(|limits| Limits { max: limits.max }).map_err(Clone::clone))
    }

    fn dir(&self, service: &str) -> PathBuf {
        self.latest(service).0
    }

    fn count(&self) -> usize {
        self.0.borrow().len()
    }
}

/// Each `(extension, services)`: every service records how it started and has the actions
/// `<extension>.dir` and `<extension>.max`, which answer from their own `ServiceCx`/`ActionCx`.
fn declare(extensions: &[(&str, &[&str])]) -> Starts {
    crate::entry::service::reset();
    let starts = Starts::default();
    for &(name, services) in extensions {
        let extension = crate::entry::extension(name);
        for &short in services {
            let (full, log) = (format!("{name}.{short}"), starts.0.clone());
            let service = extension.service(short, move |cx| {
                let limits = cx.extension_config::<Limits>().map_err(|error| error.to_string());
                log.borrow_mut().push(Start { service: full.clone(), dir: cx.data_dir().to_owned(), limits });
                Ok(Probe)
            });
            service.action(extension.action(&format!("{short}_dir")), |_, cx, _| {
                crate::entry::service::ActionResult::Done(Some(cx.data_dir().display().to_string()))
            });
            service.action(extension.action(&format!("{short}_max")), |_, cx, _| {
                crate::entry::service::ActionResult::Done(Some(match cx.extension_config::<Limits>() {
                    Ok(limits) => limits.max.to_string(),
                    Err(error) => error.to_string(),
                }))
            });
        }
    }
    starts
}

fn start_under(root: &Path, kind: ServerKind, config: &heca_config::loader::AppConfig) -> ServerLoop {
    server_as(config, ServerIdentity::claim_in(kind, Some(root.to_owned())), ServerWaker::new(|| {}))
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions()) & 0o777
}

/// **A service's folder exists before its factory runs, only its owner can use it, and no other
/// service shares it.** The service sees the same folder from an action as from its factory.
#[test]
fn each_service_gets_a_folder_that_exists_is_private_and_is_its_own() {
    let starts = declare(&[("demo", &["one", "two"])]);
    let mut server = server(&config(""));
    let (one, two) = (starts.dir("demo.one"), starts.dir("demo.two"));
    assert!(one.is_dir() && two.is_dir());
    assert_ne!(one, two);
    assert_eq!(one.parent(), two.parent(), "both are in this server's folder");
    assert!(one.ends_with("default/demo.one"), "{one:?}");
    #[cfg(unix)]
    for folder in [&one, &two, one.parent().unwrap()] {
        assert_eq!(mode(folder), 0o700, "{folder:?} is the owner's alone");
    }
    assert_eq!(rpc(&mut server, "demo.one_dir").try_recv().unwrap(), one.display().to_string());
    assert_eq!(rpc(&mut server, "demo.two_dir").try_recv().unwrap(), two.display().to_string());
}

/// **The folder is the same after the server restarts** — what the service wrote is still there.
#[test]
fn a_restarted_server_gives_a_service_its_folder_back() {
    let root = tempfile::tempdir().unwrap();
    let starts = declare(&[("demo", &["one"])]);
    let mut first = start_under(root.path(), ServerKind::Background, &config(""));
    let before = starts.dir("demo.one");
    std::fs::write(before.join("archive"), "kept").unwrap();
    first.shutdown();
    drop(first);
    let _second = start_under(root.path(), ServerKind::Background, &config(""));
    assert_eq!(starts.dir("demo.one"), before);
    assert_eq!(std::fs::read_to_string(before.join("archive")).unwrap(), "kept");
}

/// **Two standalone windows at once never share a folder; a restarted one gets its slot back.**
#[test]
fn standalone_servers_at_once_have_folders_of_their_own_and_a_restart_gets_its_slot_back() {
    let root = tempfile::tempdir().unwrap();
    let starts = declare(&[("demo", &["one"])]);
    let first = start_under(root.path(), ServerKind::Standalone, &config(""));
    let slot_1 = starts.dir("demo.one");
    let _second = start_under(root.path(), ServerKind::Standalone, &config(""));
    let slot_2 = starts.dir("demo.one");
    assert!(slot_1.ends_with("standalone-1/demo.one") && slot_2.ends_with("standalone-2/demo.one"), "{slot_1:?} {slot_2:?}");
    drop(first);
    let _restarted = start_under(root.path(), ServerKind::Standalone, &config(""));
    assert_eq!(starts.dir("demo.one"), slot_1, "the lowest free slot, which is the one that was freed");
}

/// **No data folder: each service says why it did not start, and the server runs.**
#[test]
fn a_server_with_no_data_folder_reports_each_service_and_still_runs() {
    let starts = declare(&[("demo", &["one", "two"])]);
    let nowhere = ServerIdentity::claim_in(ServerKind::Background, None);
    let mut server = server_as(&config(""), nowhere, ServerWaker::new(|| {}));
    assert_eq!(starts.count(), 0, "no factory ran");
    let window = attached(&mut server);
    let messages = drain(&window);
    assert!(messages.iter().any(|message| matches!(message, ServerMsg::Attached { .. })), "the server still serves a window");
    for service in ["demo.one", "demo.two"] {
        assert!(messages.iter().any(|message| matches!(message, ServerMsg::Notifications(views)
            if views.iter().any(|view| view.title.contains(service)
                && view.body.as_deref().is_some_and(|body| body.contains("no data folder"))))), "{service} is reported");
    }
}

/// **An extension reads its own table as a typed value; no table is its defaults.**
#[test]
fn an_extension_reads_its_table_as_a_typed_value_and_a_missing_one_is_its_defaults() {
    let starts = declare(&[("demo", &["one"])]);
    let _set = server(&config("[extensions.demo]\nmax = 3"));
    assert_eq!(starts.latest("demo.one").1, Ok(Limits { max: 3 }));
    let _unset = server(&config(""));
    assert_eq!(starts.latest("demo.one").1, Ok(Limits { max: 7 }), "a missing table is an empty one");
}

/// **A table that does not fit is an explicit error that names the table and what is wrong** — and
/// a factory that passes it on with `?` is reported, not started with a default.
#[test]
fn a_table_that_does_not_fit_is_an_error_the_user_is_told_about() {
    let starts = declare(&[("demo", &["one"])]);
    let _server = server(&config("[extensions.demo]\nmax = \"lots\""));
    let error = starts.latest("demo.one").1.unwrap_err();
    assert!(error.contains("[extensions.demo]") && error.contains("lots"), "{error}");

    crate::entry::service::reset();
    let demo = crate::entry::extension("demo");
    demo.service::<Probe>("strict", |cx| {
        let _limits: Limits = cx.extension_config()?;
        Ok(Probe)
    });
    let mut server = server(&config("[extensions.demo]\nmax = \"lots\""));
    let messages = drain(&attached(&mut server));
    assert!(messages.iter().any(|message| matches!(message, ServerMsg::Notifications(views)
        if views.iter().any(|view| view.title.contains("demo.strict")
            && view.body.as_deref().is_some_and(|body| body.contains("[extensions.demo]"))))), "the user is told which table");
}

/// **An extension can read no table but its own.**
#[test]
fn an_extension_cannot_read_another_extensions_table() {
    let starts = declare(&[("demo", &["one"]), ("other", &["two"])]);
    let _both = server(&config("[extensions.demo]\nmax = 3\n[extensions.other]\nmax = 9"));
    assert_eq!(starts.latest("demo.one").1, Ok(Limits { max: 3 }));
    assert_eq!(starts.latest("other.two").1, Ok(Limits { max: 9 }));
    let _only_other = server(&config("[extensions.other]\nmax = 9"));
    assert_eq!(starts.latest("demo.one").1, Ok(Limits { max: 7 }), "demo has no table, and other's is not its own");
}

/// **Actions and projections read the session's effective configuration** — the table a trusted
/// project changed — not the one the server started with.
#[test]
fn actions_and_projections_read_the_sessions_table_not_the_servers() {
    let starts = declare(&[("demo", &["one"])]);
    let mut server = server(&config("[extensions.demo]\nmax = 3"));
    assert_eq!(starts.latest("demo.one").1, Ok(Limits { max: 3 }));
    assert_eq!(rpc(&mut server, "demo.one_max").try_recv().unwrap(), "3", "no session table: the server's");
    server.sessions[0].set_config(&config("[extensions.demo]\nmax = 5"));
    assert_eq!(rpc(&mut server, "demo.one_max").try_recv().unwrap(), "5", "an action reads the session's");
    let shown = |server: &mut ServerLoop| {
        let window = attached(server);
        drain(&window).into_iter().find_map(|message| match message {
            ServerMsg::Service(update) => Some(update.kind),
            _ => None,
        })
    };
    assert_eq!(shown(&mut server), Some(crate::server::wire::service::ServiceUpdateKind::Snapshot("5".into())), "so does a projection");
}
