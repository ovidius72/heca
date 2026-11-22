//! **What each terminal starts as when its session starts again** — by the two switches, through a
//! harness's own driver, and when its folder is gone — and what a bad saved file does to the server.

use super::restore_tests::*;
use super::*;
use crate::app::backend_store::Program;
use heca_core::backend::ProcessLaunch;
use std::path::Path;

/// The harness the tests run, started in `cwd`, as the original launch says.
fn harness(cwd: &Path) -> ProcessLaunch {
    ProcessLaunch {
        executable: "/opt/tools/harness".into(),
        arguments: vec!["--label".into(), "two words".into()],
        cwd: Some(cwd.to_owned()),
    }
}

fn program(cwd: &Path) -> ProcessLaunch {
    ProcessLaunch {
        executable: "/opt/tools/serve".into(),
        arguments: vec!["--port".into(), "80".into()],
        cwd: Some(cwd.to_owned()),
    }
}

/// A server with one session, `Work`, holding a plain program and a harness — each in a folder of its
/// own, which is how a test tells their panes apart — then stopped and started again under `text`.
/// Returns the new server and the two folders.
fn restarted_with(
    text: &str,
    native: Option<&str>,
    driver: &str,
) -> (
    ServerLoop,
    std::path::PathBuf,
    std::path::PathBuf,
    tempfile::TempDir,
) {
    declare_drivers();
    set_harness(native, driver);
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let (serve, agent) = (
        made(folders.path().join("serve")),
        made(folders.path().join("agent")),
    );
    let mut server = boot(root.path(), text);
    make(&mut server, folders.path(), "Work");
    session(&mut server, "Work").arrange(|session, state| {
        let first = session.add_column(0, 1).unwrap().pane;
        let second = session.add_column(0, 2).unwrap().pane;
        start_terminal(state, first, Program::Recorded(program(&serve)), &serve);
        start_terminal(state, second, Program::Recorded(harness(&agent)), &agent);
    });
    forget_started();
    let server = restart(server, root.path(), text);
    (server, serve, agent, root)
}

/// A folder that exists, spelled the way the system does.
fn made(folder: std::path::PathBuf) -> std::path::PathBuf {
    std::fs::create_dir_all(&folder).unwrap();
    folder.canonicalize().unwrap()
}

/// **The two switches are independent**, each over its own kind of terminal: a program comes back as
/// it was only when `resume_programs` is on, a harness only when `resume_harness` is, and a terminal
/// that is not brought back has the user's shell.
#[test]
fn programs_and_harnesses_are_resumed_by_their_own_switch() {
    for programs in [false, true] {
        for harnesses in [false, true] {
            let text = format!(
                "{DRIVERS_ON}[settings]\nresume_programs = {programs}\nresume_harness = {harnesses}"
            );
            let (_server, serve, agent, _root) =
                restarted_with(&text, Some("native-1"), "demo.resuming");
            let case = format!("programs={programs} harness={harnesses}");
            match started_in(&serve).as_slice() {
                [Program::Recorded(launch)] if programs => {
                    assert_eq!(*launch, program(&serve), "{case}")
                }
                [Program::Shell] if !programs => {}
                other => panic!("{case}: the program started as {other:?}"),
            }
            match started_in(&agent).as_slice() {
                [Program::Recorded(launch)] if harnesses => {
                    assert_eq!(
                        launch.arguments,
                        ["--label", "two words", "--resume", "native-1"],
                        "{case}"
                    );
                }
                [Program::Shell] if !harnesses => {}
                other => panic!("{case}: the harness started as {other:?}"),
            }
        }
    }
}

/// **A harness with a native session identity is resumed through its own driver**, in its own folder,
/// with nothing said to the user — it came back, as a conversation, not a new one.
#[test]
fn a_harness_with_a_native_id_is_resumed_by_its_driver() {
    let (mut server, _, agent, _root) =
        restarted_with(DRIVERS_ON, Some("native-77"), "demo.resuming");
    let launches = started_in(&agent);
    let [Program::Recorded(launch)] = launches.as_slice() else {
        panic!("started as {launches:?}")
    };
    assert_eq!(launch.executable, harness(&agent).executable);
    assert_eq!(
        launch
            .arguments
            .last()
            .map(|argument| argument.to_str().unwrap()),
        Some("native-77")
    );
    assert_eq!(launch.cwd.as_deref(), Some(agent.as_path()));
    assert_eq!(status(&server, "Work"), SessionStatus::Active);
    assert!(notices(&mut server, "Work").is_empty(), "nothing to report");
}

/// **A harness that cannot be resumed is never replaced by a new conversation**: no native identity,
/// a driver with no way to resume, or a driver that is not there each give the pane the user's shell
/// in the harness's folder, and say so.
#[test]
fn a_harness_that_cannot_be_resumed_gets_a_shell_and_a_report_never_a_new_conversation() {
    let cases = [
        (
            "no native session identity",
            DRIVERS_ON,
            None,
            "demo.resuming",
            "Demo",
            "never reported its own session identity",
        ),
        (
            "a driver that cannot resume",
            DRIVERS_ON,
            Some("native-1"),
            "demo.plain",
            "Demo",
            "cannot continue a conversation",
        ),
        (
            "a driver that is not switched on",
            "[settings.agents]\nenabled = false",
            Some("native-1"),
            "demo.resuming",
            "Demo",
            "is not registered",
        ),
    ];
    for (case, text, native, driver, name, reason) in cases {
        let (mut server, _, agent, _root) = restarted_with(text, native, driver);
        let launches = started_in(&agent);
        assert!(
            matches!(launches.as_slice(), [Program::Shell]),
            "{case}: started as {launches:?}"
        );
        assert!(
            started().iter().all(|spec| !matches!(&spec.program, Program::Recorded(launch) if launch.executable == harness(&agent).executable)),
            "{case}: the harness was started again as a new conversation",
        );
        assert_eq!(
            status(&server, "Work"),
            SessionStatus::Active,
            "{case}: the session itself is back"
        );
        let shown = notices(&mut server, "Work");
        let [(title, Some(body))] = shown.as_slice() else {
            panic!("{case}: reported {shown:?}")
        };
        assert_eq!(*title, format!("{name} was not resumed"), "{case}");
        assert!(body.contains(reason), "{case}: {body}");
        assert!(
            body.contains("new conversation was not started"),
            "{case}: {body}"
        );
        assert_eq!(
            lasting(&mut server, "Work"),
            std::slice::from_ref(title),
            "{case}: it stays until dismissed"
        );
    }
}

/// **A session whose folder is gone still comes back**: each terminal that was in it has the user's
/// shell, started in the home folder, and the user is told.
#[test]
fn a_missing_folder_gives_a_shell_at_home_and_a_notification() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let folder = folders.path().join("project");
    let mut server = boot(root.path(), "");
    make(&mut server, &folder, "Project");
    let folder = folder.canonicalize().unwrap();
    session(&mut server, "Project").arrange(|session, state| {
        let pane = session.add_column(0, 1).unwrap().pane;
        start_terminal(state, pane, Program::Recorded(program(&folder)), &folder);
    });
    server.shutdown();
    drop(server);
    std::fs::remove_dir_all(&folder).unwrap();
    forget_started();
    let mut server = boot(root.path(), "");
    let home = dirs::home_dir();
    let specs = started();
    assert_eq!(specs.len(), 2);
    for spec in &specs {
        assert_eq!(
            spec.program,
            Program::Shell,
            "a program is not run in the wrong folder"
        );
        assert_eq!(spec.cwd, home, "the shell starts at home");
    }
    assert_eq!(status(&server, "Project"), SessionStatus::Active);
    let shown = notices(&mut server, "Project");
    assert!(
        shown
            .iter()
            .any(|(title, _)| title.contains("doesn't exist, starting in your home folder")),
        "{shown:?}"
    );
    assert!(
        lasting(&mut server, "Project")
            .iter()
            .any(|title| title.contains("doesn't exist")),
        "it stays until dismissed"
    );
}

/// **A file nobody can read never stops the server**: a missing one is a first start; a corrupt one and
/// one from a newer heca each start the server with no sessions, say why once, and are kept aside so
/// the next write cannot destroy them.
#[test]
fn a_missing_corrupt_or_newer_file_starts_the_server_and_reports_once() {
    let folders = tempfile::tempdir().unwrap();
    let missing = tempfile::tempdir().unwrap();
    let server = boot(missing.path(), "");
    assert!(
        server.sessions.is_empty() && server.notices.is_empty(),
        "a missing file is a first start, with nothing to say"
    );

    let cases = [
        ("corrupt", "this is {not json".to_owned(), "not readable"),
        (
            "newer",
            r#"{"version": 99, "next_session": 5, "sessions": []}"#.to_owned(),
            "version 99",
        ),
    ];
    for (case, text, why) in cases {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(server_folder(root.path())).unwrap();
        std::fs::write(saved_file(root.path()), &text).unwrap();
        let mut server = boot(root.path(), "");
        assert!(server.sessions.is_empty(), "{case}: no sessions");
        assert_eq!(
            server.notices.len(),
            1,
            "{case}: one report, held until a session can show it"
        );
        make(&mut server, &folders.path().join(case), "First");
        server.pump(Instant::now());
        let shown = notices(&mut server, "First");
        let reports: Vec<_> = shown
            .iter()
            .filter(|(title, _)| title == "Some saved sessions could not be restored")
            .collect();
        assert_eq!(
            reports.len(),
            1,
            "{case}: shown once, not at every turn: {shown:?}"
        );
        assert!(
            reports[0]
                .1
                .as_deref()
                .is_some_and(|body| body.contains(why)),
            "{case}: {shown:?}"
        );
        assert_eq!(
            lasting(&mut server, "First"),
            ["Some saved sessions could not be restored"],
            "{case}: it stays until dismissed"
        );
        assert!(server.notices.is_empty());
        let aside = server_folder(root.path()).join("heca.sessions.json.unreadable");
        assert_eq!(
            std::fs::read_to_string(aside).unwrap(),
            text,
            "{case}: kept, not overwritten"
        );

        // The next start reads what this one wrote, and has nothing to report.
        let server = restart(server, root.path(), "");
        assert!(server.notices.is_empty(), "{case}: not reported again");
        assert_eq!(server.sessions.len(), 1);
    }
}

/// **One session that cannot be rebuilt is left out and reported; the others come back.**
#[test]
fn one_bad_session_does_not_cost_the_others() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    make(&mut server, &folders.path().join("a"), "Good");
    make(&mut server, &folders.path().join("b"), "Bad");
    server.shutdown();
    drop(server);
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(saved_file(root.path())).unwrap()).unwrap();
    let bad = file["sessions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|s| s["name"] == "Bad")
        .unwrap();
    bad["content"]["workspaces"] = serde_json::json!([]);
    std::fs::write(saved_file(root.path()), file.to_string()).unwrap();
    let server = boot(root.path(), "");
    assert_eq!(server.sessions.len(), 1);
    assert!(server.named("Good").is_ok());
    assert_eq!(
        server.notices.len(),
        1,
        "the one that could not be rebuilt is reported once"
    );
}

/// **A harness heca was never told about is still not started as a new conversation.** Agent reporting
/// is opt-in per project, so a pane can hold `claude` with no driver identity and no native session
/// id recorded — only the command the shell was handed. The record from a real restore is replayed
/// here: `agent: null`, launch `/bin/zsh -ic "claude "`. It is a harness by what it runs, so it is
/// governed by `resume_harness` and never by `resume_programs`, and a harness with no session id gets
/// the user's shell and a report that says how to turn reporting on.
#[test]
fn a_harness_recorded_without_reporting_gets_a_shell_and_a_report_not_a_new_conversation() {
    for (programs, harnesses) in [(true, true), (false, true), (true, false), (false, false)] {
        let root = tempfile::tempdir().unwrap();
        let folders = tempfile::tempdir().unwrap();
        let folder = made(folders.path().join("prova-a"));
        let text =
            format!("[settings]\nresume_programs = {programs}\nresume_harness = {harnesses}");
        let mut server = boot(root.path(), &text);
        make(&mut server, folders.path(), "A");
        let typed = ProcessLaunch {
            executable: "/bin/zsh".into(),
            arguments: vec!["-ic".into(), "claude ".into()],
            cwd: Some(folder.clone()),
        };
        session(&mut server, "A").arrange(|session, state| {
            let pane = session.add_column(0, 1).unwrap().pane;
            start_terminal(state, pane, Program::Recorded(typed.clone()), &folder);
        });
        let record = session(&mut server, "A")
            .recorded_terminals()
            .into_iter()
            .find(|terminal| terminal.launch.as_ref() == Some(&typed))
            .unwrap();
        assert!(
            serde_json::to_value(&record).unwrap()["agent"].is_null(),
            "the record is the one that was seen: no driver, no native id"
        );
        forget_started();
        let mut server = restart(server, root.path(), &text);
        let case = format!("programs={programs} harness={harnesses}");
        let in_folder = started_in(&folder);
        assert!(
            !in_folder
                .iter()
                .any(|program| matches!(program, Program::Recorded(launch) if *launch == typed)),
            "{case}: claude was started again as a new conversation: {in_folder:?}"
        );
        assert!(
            in_folder.contains(&Program::Shell),
            "{case}: the pane has the user's shell: {in_folder:?}"
        );
        let shown = notices(&mut server, "A");
        if harnesses {
            let [(title, Some(body))] = shown.as_slice() else {
                panic!("{case}: reported {shown:?}")
            };
            assert_eq!(title, "Claude Code was not resumed", "{case}");
            assert!(
                body.contains("new conversation was not started")
                    && body.contains("Agent reporting"),
                "{case}: {body}"
            );
            assert!(
                body.contains("settings.agents.programs.claude"),
                "{case}: {body}"
            );
            assert_eq!(
                lasting(&mut server, "A"),
                std::slice::from_ref(title),
                "{case}: it stays until dismissed"
            );
        } else {
            assert!(
                shown.is_empty(),
                "{case}: switched off by the user, nothing to report: {shown:?}"
            );
        }
    }
}

/// An ordinary typed command is still replayed exactly as recorded under `resume_programs`: telling
/// harnesses apart must not take it away.
#[test]
fn an_ordinary_typed_command_is_still_replayed_as_recorded() {
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let folder = made(folders.path().join("web"));
    let mut server = boot(root.path(), "");
    make(&mut server, folders.path(), "A");
    let typed = ProcessLaunch {
        executable: "/bin/zsh".into(),
        arguments: vec!["-ic".into(), "npm run dev".into()],
        cwd: Some(folder.clone()),
    };
    session(&mut server, "A").arrange(|session, state| {
        let pane = session.add_column(0, 1).unwrap().pane;
        start_terminal(state, pane, Program::Recorded(typed.clone()), &folder);
    });
    forget_started();
    let _server = restart(server, root.path(), "");
    assert_eq!(started_in(&folder), [Program::Recorded(typed)]);
}
