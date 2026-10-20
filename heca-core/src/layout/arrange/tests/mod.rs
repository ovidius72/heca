//! The changes to the session, made on the content alone, and what each says it did.

use crate::layout::testing::Windowed;
use crate::layout::{Pane, PaneId, Session};

mod moves;
mod names;
mod remove;
mod swap;

/// The window a pane is given room in.
const HEIGHT: f64 = 800.0;

/// A session of these workspaces, columns and panes, and no window looking at it.
fn session(shape: &[&[&[u64]]]) -> Session {
    Windowed::with_shape(shape).session
}

/// The panes of every column of workspace `ws`, as ids.
fn columns(session: &Session, ws: usize) -> Vec<Vec<u64>> {
    session.workspaces[ws]
        .scrolling
        .columns
        .iter()
        .map(|c| c.panes.iter().map(|p| p.id.0).collect())
        .collect()
}

/// Float a pane that never tiled, `id`, in workspace `ws`.
fn float(session: &mut Session, ws: usize, id: u64) {
    session.workspaces[ws].add_floating_pane(Pane::new(PaneId(id), "f"), None);
}
