//! **Running programs in terminals**: a command in a pane of its own, a line typed into a running
//! terminal, a terminal ended. Each names its terminal by the store's one rule
//! ([`BackendStore::target`](crate::app::backend_store::BackendStore::target)).

use std::path::PathBuf;

use heca_core::layout::{Pane, PaneId};

use crate::app::backend_store::{Program, TerminalSpec, TerminalTarget};
use crate::input::{SpawnKind, WmAction};
use crate::server::{Change, ServerCx};

/// Run a command in a new pane — floating in the middle of the asker's area, or in a column right
/// of the asker's. The process starts first: a command that cannot start leaves no empty pane, and
/// the user is told why.
pub(super) fn spawn_command(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::SpawnCommand {
        command,
        kind,
        float,
        close_policy,
        cwd,
    } = action
    else {
        return Vec::new();
    };
    if !matches!(kind, SpawnKind::Terminal) {
        eprintln!("[heca] spawn kind '{kind:?}' not yet implemented");
        return Vec::new();
    }
    let pane = PaneId(cx.layout.next_id());
    let spec = TerminalSpec {
        program: Program::Command(command.clone()),
        cwd: cwd.as_deref().map(PathBuf::from),
        grid: cx.asker.grid(),
    };
    if let Err(e) = cx.terminals.start(pane, spec, cx.asker.cell) {
        crate::notification::Notification::danger(format!("Couldn't run '{command}'"))
            .body(e.to_string())
            .dedup_key(format!("spawn.failed:{command}"))
            .sticky()
            .send();
        return Vec::new();
    }
    let mut made = Pane::new(pane, command.clone());
    made.close_policy = *close_policy;
    let ws = cx.asker.workspace;
    let added = match float {
        true => {
            let rect = cx
                .layout
                .workspaces
                .get(ws)
                .map(|w| w.float_rect_in(cx.asker.area));
            rect.and_then(|rect| cx.layout.add_floating(ws, made, rect))
        }
        false => cx.layout.add_column_with(ws, cx.asker.column + 1, made),
    };
    let Some(a) = added else {
        cx.terminals.end([pane]);
        return Vec::new();
    };
    Change::after_adding(a)
}

/// Type `text` into a terminal, and press Enter after it unless told not to. Nothing changes that
/// a window shows until the terminal's own output does.
pub(super) fn terminal_run(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::TerminalRun {
        pane_id,
        terminal,
        text,
        enter,
    } = action
    else {
        return Vec::new();
    };
    let backends = &mut *cx.terminals.backends;
    let Some(target) = backends.target(*pane_id, *terminal, cx.asker.focused_pane) else {
        return Vec::new();
    };
    let mut bytes = text.as_bytes().to_vec();
    if *enter {
        bytes.push(b'\r');
    }
    let backend = match target {
        TerminalTarget::Pane(pane) => backends.get_mut(pane),
        TerminalTarget::Unowned(id) => backends.get_mut_by_id(id),
    };
    if let Some(backend) = backend {
        backend.process_input(&bytes);
    }
    Vec::new()
}

/// End a terminal. A pane's ends by closing its pane, so there is one way a pane's process ends;
/// one no pane owns (an extension's) is ended here, and this is the only thing that ends it.
pub(super) fn terminal_kill(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::TerminalKill { pane_id, terminal } = action else {
        return Vec::new();
    };
    match cx
        .terminals
        .backends
        .target(*pane_id, *terminal, cx.asker.focused_pane)
    {
        Some(TerminalTarget::Pane(pane)) => super::remove::close_pane(cx, pane),
        Some(TerminalTarget::Unowned(id)) => match cx.terminals.backends.kill(id) {
            true => vec![Change::LayoutChanged],
            false => Vec::new(),
        },
        None => Vec::new(),
    }
}
