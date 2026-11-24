//! Making panes with a program running in them, typing into terminals and ending them — on the
//! server, which starts and ends the processes itself.

use heca_core::layout::PaneId;
use heca_core::layout::testing::Windowed;

use super::testing::{facts, run_on, server, two_workspaces_of_two_columns};
use crate::input::{SpawnKind, WmAction};
use crate::server::{Change, ServerState};

/// The panes of every column of workspace `ws`, as ids.
fn columns(w: &Windowed, ws: usize) -> Vec<Vec<u64>> {
    w.session.workspaces[ws]
        .scrolling
        .columns
        .iter()
        .map(|c| c.panes.iter().map(|p| p.id.0).collect())
        .collect()
}

#[test]
fn renaming_a_named_terminal_changes_only_metadata_and_clear_restores_its_default() {
    use crate::app::backend_store::{Program, TerminalOwner, TerminalSpec};
    let (mut state, mut window) = (server(), two_workspaces_of_two_columns());
    let owner = TerminalOwner::Named("demo.logs".into());
    let spec = TerminalSpec {
        program: Program::Shell,
        cwd: None,
        grid: (80, 24),
    };
    let id = state
        .backends
        .ensure(owner.clone(), spec.clone(), |_| {
            Ok(Box::new(heca_core::backend::FakeBackend::new(80, 24)))
        })
        .unwrap()
        .id();
    let rename = |name: &str| WmAction::TerminalRenameTo {
        terminal: "demo.logs".into(),
        name: name.into(),
    };
    assert_eq!(
        run_on(&mut state, &mut window, None, rename("Logs")),
        vec![Change::TerminalTitleChanged {
            owner: owner.clone(),
            title: Some("Logs".into())
        }]
    );
    assert_eq!(state.backends.find_owner(&owner), Some(id));
    assert_eq!(state.backends.spec_of(id), Some(&spec));
    assert_eq!(state.backends.custom_title(id), Some("Logs"));
    assert!(run_on(&mut state, &mut window, None, rename("Logs")).is_empty());
    assert_eq!(
        run_on(&mut state, &mut window, None, rename("")),
        vec![Change::TerminalTitleChanged { owner, title: None }]
    );
    assert_eq!(state.backends.custom_title(id), None);
}

#[test]
fn renaming_a_panes_terminal_reuses_its_existing_pane_name() {
    let (mut state, mut window) = (server(), two_workspaces_of_two_columns());
    let pane = PaneId(1);
    state
        .terminals()
        .start_shell(pane, None, (80, 24), (10.0, 20.0));
    let id = state.backends.identity_of(pane).unwrap();
    assert_eq!(
        run_on(
            &mut state,
            &mut window,
            Some(pane),
            WmAction::TerminalRenameTo {
                terminal: id.0.to_string(),
                name: "Build".into()
            }
        ),
        vec![Change::NamesChanged]
    );
    assert_eq!(
        window.session.workspaces[0]
            .find_pane(pane)
            .unwrap()
            .custom_name
            .as_deref(),
        Some("Build")
    );
    assert_eq!(
        state.backends.custom_title(id),
        None,
        "there is no second competing terminal label"
    );
}

/// The pane a change says was made — tiled ([`Change::PaneAdded`]) or floating
/// ([`Change::Floated`]).
fn made(changes: &[Change]) -> Option<PaneId> {
    changes.iter().find_map(|change| match change {
        Change::PaneAdded { pane, .. } | Change::Floated { pane, .. } => Some(*pane),
        _ => None,
    })
}

/// Whether `pane` has a process the server keeps.
fn runs(server: &ServerState, pane: PaneId) -> bool {
    server.backends.get(pane).is_some()
}

#[test]
fn a_split_makes_a_column_right_of_the_askers_with_a_shell_in_it() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    w.ws().scroll_mut().activate_column(0);
    let changes = run_on(
        &mut server,
        &mut w,
        Some(PaneId(1)),
        WmAction::SplitHorizontal,
    );
    let pane = made(&changes).expect("a pane");
    assert_eq!(columns(&w, 0), [vec![1], vec![pane.0], vec![2]]);
    assert!(runs(&server, pane), "its shell runs");
}

#[test]
fn a_split_down_makes_a_pane_under_the_askers_with_a_shell_in_it() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    w.ws().scroll_mut().activate_column(1);
    let changes = run_on(
        &mut server,
        &mut w,
        Some(PaneId(2)),
        WmAction::SplitVertical,
    );
    let pane = made(&changes).expect("a pane");
    assert_eq!(columns(&w, 0), [vec![1], vec![2, pane.0]]);
    assert!(runs(&server, pane));
}

/// The workspace is said to be there before the pane made in it.
#[test]
fn a_new_workspace_comes_with_a_pane_and_a_shell() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    let changes = run_on(&mut server, &mut w, None, WmAction::CreateWorkspace);
    let pane = made(&changes).expect("a pane");
    assert_eq!(changes.first(), Some(&Change::WorkspaceAdded { index: 2 }));
    assert_eq!(columns(&w, 2), [[pane.0]]);
    assert!(runs(&server, pane));
}

fn spawn(float: bool) -> WmAction {
    spawn_in(float, None)
}

/// `htop` started with `cwd` as the user wrote it.
fn spawn_in(float: bool, cwd: Option<&str>) -> WmAction {
    WmAction::SpawnCommand {
        command: "htop".into(),
        kind: SpawnKind::Terminal,
        float,
        close_policy: Default::default(),
        cwd: cwd.map(str::to_string),
    }
}

#[test]
fn a_command_runs_in_a_column_of_its_own_named_after_it() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    w.ws().scroll_mut().activate_column(0);
    let changes = run_on(&mut server, &mut w, Some(PaneId(1)), spawn(false));
    let pane = made(&changes).expect("a pane");
    assert_eq!(columns(&w, 0), [vec![1], vec![pane.0], vec![2]]);
    let name = w.session.workspaces[0]
        .find_pane(pane)
        .map(|p| p.title.clone());
    assert_eq!(name.as_deref(), Some("htop"));
    assert!(runs(&server, pane));
}

#[test]
fn a_floating_command_floats_and_leaves_the_columns_alone() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    let changes = run_on(&mut server, &mut w, None, spawn(true));
    let pane = made(&changes).expect("a pane");
    assert_eq!(columns(&w, 0), [vec![1], vec![2]]);
    assert!(
        w.session.workspaces[0]
            .floating_panes
            .iter()
            .any(|f| f.pane.id == pane)
    );
    assert!(
        facts(&changes).len() == changes.len(),
        "nothing in the columns to show"
    );
}

/// A command that cannot start leaves no empty pane behind.
#[test]
fn a_command_that_cannot_start_makes_no_pane() {
    fn fails(
        _: &crate::app::backend_factory::LaunchSettings<'_>,
        _: &crate::app::backend_store::TerminalSpec,
    ) -> Result<Box<dyn heca_core::backend::PaneBackend>, crate::app::backend_store::SpawnError>
    {
        Err(crate::app::backend_store::SpawnError(
            "no such program".into(),
        ))
    }
    let mut server = server().launching_with(fails);
    let mut w = two_workspaces_of_two_columns();
    assert!(run_on(&mut server, &mut w, None, spawn(false)).is_empty());
    assert_eq!(columns(&w, 0), [vec![1], vec![2]]);
}

/// Closing a pane ends what ran in it; so does ending its terminal, which closes the pane.
#[test]
fn a_closed_pane_has_its_process_ended() {
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    let first = made(&run_on(
        &mut server,
        &mut w,
        None,
        WmAction::SplitHorizontal,
    ))
    .expect("a pane");
    let second = made(&run_on(
        &mut server,
        &mut w,
        None,
        WmAction::SplitHorizontal,
    ))
    .expect("a pane");
    run_on(
        &mut server,
        &mut w,
        None,
        WmAction::ClosePaneById { pane_id: first },
    );
    assert!(!runs(&server, first));
    let changes = run_on(
        &mut server,
        &mut w,
        Some(second),
        WmAction::TerminalKill {
            pane_id: None,
            terminal: None,
        },
    );
    assert!(changes.contains(&Change::PaneRemoved { pane: second }));
    assert!(!runs(&server, second));
    assert!(
        w.session.workspace_holding(second).is_none(),
        "its pane went with it"
    );
}

/// A server whose asking pane (pane 1, in column 0) reports that its program is in `folder`, and
/// the folder a pane made by `action` from there started in.
fn started_from_pane_in(folder: &std::path::Path, action: WmAction) -> Option<std::path::PathBuf> {
    use crate::app::backend_store::{Program, TerminalOwner, TerminalSpec};
    let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
    let mut asker = heca_core::backend::FakeBackend::new(80, 24);
    asker.set_runtime(heca_core::runtime::PaneRuntime {
        cwd: Some(folder.to_path_buf()),
        ..Default::default()
    });
    let spec = TerminalSpec {
        program: Program::Shell,
        cwd: None,
        grid: (80, 24),
    };
    server
        .backends
        .ensure(TerminalOwner::Pane(PaneId(1)), spec, move |_| {
            Ok(Box::new(asker))
        })
        .unwrap();
    w.ws().scroll_mut().activate_column(0);
    let pane = made(&run_on(&mut server, &mut w, Some(PaneId(1)), action)).expect("a pane");
    let id = server.backends.identity_of(pane).expect("its terminal");
    server
        .backends
        .spec_of(id)
        .and_then(|spec| spec.cwd.clone())
}

/// **A pane made from a pane starts where that pane is**: the new shell opens in the folder the
/// asker's focused pane reports — for a split, a pane under it, a new column. A new workspace is
/// not made from a pane, so it starts in `[settings] new_workspace_cwd` (the test server's is `~`).
#[test]
fn a_pane_made_from_a_pane_starts_in_that_panes_folder() {
    let folder = std::env::temp_dir();
    let started_in = |action| started_from_pane_in(&folder, action);
    assert_eq!(
        started_in(WmAction::SplitHorizontal),
        Some(folder.clone()),
        "a split"
    );
    assert_eq!(
        started_in(WmAction::SplitVertical),
        Some(folder.clone()),
        "a pane under it"
    );
    assert_eq!(
        started_in(WmAction::AddColumnToWorkspace { ws_idx: 0 }),
        Some(folder.clone()),
        "a new column"
    );
    assert_eq!(
        started_in(WmAction::CreateWorkspace),
        dirs::home_dir(),
        "a new workspace: `~`, the home folder"
    );
}

/// **A program started from a key starts in the folder its `cwd` names** — the pane it was started
/// from when nothing is written, and every other value read the one way, from that pane.
#[test]
fn a_started_program_starts_in_the_folder_its_cwd_names() {
    let folder = tempfile::tempdir().unwrap();
    let pane = folder.path().join("project/src");
    std::fs::create_dir_all(&pane).unwrap();
    git2::Repository::init(folder.path().join("project")).unwrap();
    let with_cwd = |cwd: Option<&str>| spawn_in(false, cwd);
    let started_in = |cwd| started_from_pane_in(&pane, with_cwd(cwd));
    let canonical = |p: Option<std::path::PathBuf>| p.map(|p| p.canonicalize().unwrap());

    assert_eq!(
        started_in(None),
        Some(pane.clone()),
        "nothing written: the pane's folder"
    );
    assert_eq!(started_in(Some("pane")), Some(pane.clone()), "\"pane\"");
    assert_eq!(
        canonical(started_in(Some("git"))),
        canonical(Some(folder.path().join("project"))),
        "\"git\": the repository's top folder"
    );
    assert_eq!(started_in(Some("~")), dirs::home_dir(), "\"~\"");
    // Absolute on every system: `/tmp` is not, on Windows (no drive letter).
    let absolute = std::env::temp_dir();
    assert_eq!(
        started_in(absolute.to_str()),
        Some(absolute.clone()),
        "an absolute path"
    );
    assert_eq!(
        started_in(Some("../docs")),
        Some(pane.join("../docs")),
        "a relative path: read from the pane's folder"
    );
}

/// **When the pane's folder is not known, the home folder stands in for it**, and `"git"` outside
/// a repository is the pane's folder itself.
#[test]
fn a_started_programs_folder_falls_back_to_home_and_to_the_pane() {
    use super::terminal::command_folder;
    let home = dirs::home_dir();
    assert_eq!(command_folder(None, None), home, "no pane: home");
    assert_eq!(
        command_folder(Some("git"), None).is_some(),
        home.is_some(),
        "no pane, git"
    );
    assert_eq!(
        command_folder(Some("src"), None),
        home.as_ref().map(|h| h.join("src")),
        "no pane, relative: from home"
    );
    let outside = tempfile::tempdir().unwrap();
    assert_eq!(
        command_folder(Some("git"), Some(outside.path())),
        Some(outside.path().to_path_buf()),
        "git outside a repository: the pane's folder"
    );
}

/// **A new workspace starts in `[settings] new_workspace_cwd`** — a folder that exists is used as
/// written; one that does not starts the shell in the home folder.
#[test]
fn a_new_workspace_starts_in_the_configured_folder_or_home() {
    let started_in = |cwd: &str| {
        let (mut server, mut w) = (server(), two_workspaces_of_two_columns());
        server.terminal_settings.new_workspace_cwd = cwd.into();
        let pane = made(&run_on(
            &mut server,
            &mut w,
            None,
            WmAction::CreateWorkspace,
        ))
        .expect("a pane");
        let id = server.backends.identity_of(pane).expect("its terminal");
        server
            .backends
            .spec_of(id)
            .and_then(|spec| spec.cwd.clone())
    };
    let folder = std::env::temp_dir();
    assert_eq!(
        started_in(folder.to_str().unwrap()),
        Some(folder.clone()),
        "a folder that exists"
    );
    assert_eq!(
        started_in("/no/such/folder/for/heca"),
        dirs::home_dir(),
        "one that does not: home"
    );
}
