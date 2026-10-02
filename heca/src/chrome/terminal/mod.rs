//! **The terminal surface** — a terminal is a component you place, like any other widget.
//!
//! `Terminal::new("shell")` goes in a tree anywhere: a pane, a dock, an overlay, a plugin's panel.
//! Whoever places it writes no rect, no click handling and no drawing pass; the framework gives it
//! the box it lives in, and it reports what it learns about that box itself.
//!
//! This file is the only one in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4);
//! everything below it takes plain data and is testable headless.

mod component;
mod input;
mod model;
#[cfg(test)]
pub(crate) mod testing;
mod viewport;

pub(crate) use component::Terminal;
use input::TerminalCommand;
pub(crate) use input::{Cell, Grid, TerminalInput};
pub(crate) use model::TerminalId;
pub(crate) use viewport::Viewport;

use heca_core::layout::PaneId;

use crate::app_state::AppState;

/// **The terminal a pane shows** — made the first time it is asked for, the same one every time
/// after, so the widget in a rebuilt tree and the one the app keeps agree.
///
/// Which terminal *process* it shows is the store's to say: it is attached here each time, so a
/// pane whose process was started after its first tree still ends up showing it.
pub(crate) fn view_of(state: &mut AppState, pane_id: PaneId) -> Terminal {
    let proxy = state.event_proxy.clone();
    let process = state.server.backends.terminal_of(pane_id);
    let terminal = state
        .terminals
        .entry(pane_id)
        .or_insert_with(|| {
            let terminal = Terminal::new();
            terminal.bind(seams(proxy, pane_id));
            terminal
        })
        .clone();
    if let Some(id) = process {
        terminal.attach(id);
    }
    terminal
}

/// **Everything a pane's terminal says to its owner**: what its scrollback controls mean, and where
/// its input goes — to the one host handler that holds the policy needing state.
fn seams(
    proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    pane_id: PaneId,
) -> input::Seams {
    let input_proxy = proxy.clone();
    let command_proxy = proxy.clone();
    input::Seams {
        scroll: scroll_intents(proxy, pane_id),
        input: Box::new(move |input| {
            let _ = input_proxy
                .send_event(crate::app::events::AppEvent::TerminalInput { pane_id, input });
        }),
        command: Box::new(move |id, command| {
            use crate::app::events::AppEvent;
            use crate::app::interaction::{InteractionIntent, InteractionSource};
            use crate::input::WmAction;
            let action = match command {
                TerminalCommand::Run { text, enter } => WmAction::TerminalRun {
                    pane_id: None,
                    terminal: Some(id.0),
                    text,
                    enter,
                },
                TerminalCommand::Kill => WmAction::TerminalKill {
                    pane_id: None,
                    terminal: Some(id.0),
                },
            };
            let _ = command_proxy.send_event(AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::ActivateAction(action),
            });
        }),
    }
}

/// **What a click on a pane's scrollback controls means**: focus the pane, then scroll it. The
/// same two actions the keyboard and RPC reach, sent the way every click on the chrome is.
fn scroll_intents(
    proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    pane_id: PaneId,
) -> viewport::ScrollIntents {
    use crate::app::events::AppEvent;
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    use crate::input::WmAction;
    let send = move |action: WmAction| {
        let _ = proxy.send_event(AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::FocusPane { pane_id },
        });
        let _ = proxy.send_event(AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::ActivateAction(action),
        });
    };
    let to_offset = send.clone();
    viewport::ScrollIntents {
        to_bottom: Box::new({
            let send = send.clone();
            move || send(WmAction::ScrollToBottom)
        }),
        to_offset: Box::new(move |rows| to_offset(WmAction::ScrollToOffset { rows })),
    }
}

/// Forget the terminals of panes that are no longer shown.
pub(crate) fn retain_only(state: &mut AppState, panes: &std::collections::HashSet<PaneId>) {
    state.terminals.retain(|id, _| panes.contains(id));
}

/// **Where each terminal was drawn this frame**, read from the scenes the frame painted: a terminal
/// paints one surface request at its own box, and the scene says where that box ended up — scrolled,
/// clipped and placed like anything else. A terminal with no request was not on screen.
///
/// Said once a frame, straight after the scene is painted, so nothing reads a position from an
/// earlier frame.
pub(crate) fn place_from(state: &AppState, scenes: &[&heca_grid_ui::Scene]) {
    let mut drawn: std::collections::HashMap<u64, heca_core::layout::Rectangle> =
        std::collections::HashMap::new();
    for scene in scenes {
        for request in heca_renderer::scene::host_requests(scene) {
            if let heca_grid_ui::scene::HostDraw::Surface { id } = request.draw {
                drawn.insert(id, request.rect);
            }
        }
    }
    for terminal in state.terminals.values() {
        terminal.place(terminal.id().and_then(|id| drawn.get(&id.0).copied()));
    }
}

/// **The box a pane's terminal fills**, and whether it was drawn: how much room the layout gave it
/// (known for every terminal, on screen or not, so one scrolled out of view keeps its size), and
/// where the last frame drew it.
///
/// A terminal that was not drawn has a box at the origin: enough to size its grid, and the caller
/// is told not to draw it there.
pub(crate) fn content_box(
    state: &AppState,
    pane_id: PaneId,
) -> (Option<heca_core::layout::Rectangle>, bool) {
    let Some(terminal) = state.terminals.get(&pane_id) else {
        return (None, false);
    };
    let Some(room) = terminal.room() else {
        return (None, false);
    };
    let placed = terminal.placed();
    let origin = placed.map_or(heca_core::layout::Point::new(0.0, 0.0), |r| r.loc);
    (
        Some(heca_core::layout::Rectangle::new(origin, room)),
        placed.is_some(),
    )
}

/// **Show each terminal how its viewport looks**, from the snapshots this frame prepared. Returns
/// whether any chip or scrollbar changed, so the caller can ask for a frame — they were painted
/// before the snapshot was read, so a change shows one frame late unless one is asked for.
pub(crate) fn show_viewports<'a>(
    state: &AppState,
    snapshots: impl Iterator<Item = (PaneId, &'a heca_core::backend::TerminalSnapshot)>,
) -> bool {
    let appearance = &state.appearance.terminal;
    let mut changed = false;
    for (pane_id, snapshot) in snapshots {
        let Some(terminal) = state.terminals.get(&pane_id) else {
            continue;
        };
        changed |= terminal.show(&Viewport {
            rows: snapshot.rows,
            scrollback_rows: snapshot.scrollback_rows,
            offset: snapshot.viewport_offset,
            scrollbar: appearance.show_scrollbar,
            badge: appearance.show_scrolled_up_badge,
            cell: (snapshot.cell_w, snapshot.cell_h),
            nominal_cell: state.pane_base_cell_size(pane_id),
        });
    }
    changed
}
