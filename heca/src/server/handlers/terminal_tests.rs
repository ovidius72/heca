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
    WmAction::SpawnCommand {
        command: "htop".into(),
        kind: SpawnKind::Terminal,
        float,
        close_policy: Default::default(),
        cwd: None,
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
