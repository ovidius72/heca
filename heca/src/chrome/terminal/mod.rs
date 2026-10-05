//! **The terminal surface** — a terminal is a component you place, like any other widget.
//!
//! `Terminal::new("shell")` goes in a tree anywhere: a pane, a dock, an overlay, a plugin's panel.
//! Whoever places it writes no rect, no click handling and no drawing pass; the framework gives it
//! the box it lives in, and it reports what it learns about that box itself.
//!
//! This file is the only one in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4);
//! everything below it takes plain data and is testable headless.

mod component;
mod declared;
mod input;
mod model;
mod search;
mod shared;
#[cfg(test)]
pub(crate) mod testing;
mod viewport;

pub use component::Terminal;
use input::TerminalCommand;
pub(crate) use input::{Cell, Grid, Search, TerminalInput};
pub(crate) use model::TerminalId;
pub(crate) use viewport::Viewport;

use heca_core::layout::PaneId;

use crate::app_state::AppState;

/// **The terminal a pane shows** — made the first time it is asked for, the same one every time
/// after, so the widget in a rebuilt tree and the one the app keeps agree.
///
/// It is found by the terminal's id, which the store issues to the pane the first time it is asked,
/// before any process runs: a pane whose shell starts after its first tree still shows it.
pub(crate) fn view_of(state: &mut AppState, pane_id: PaneId) -> Terminal {
    let id = state.backends.id_for(pane_id);
    let proxy = state.event_proxy.clone();
    state
        .terminals
        .entry(id)
        .or_insert_with(|| {
            let terminal = Terminal::new();
            terminal.attach(id);
            terminal.bind(seams(proxy, Some(pane_id), id));
            terminal
        })
        .clone()
}

/// **Start the terminals extensions declared**, once each, and show each the process it got.
///
/// A declared terminal is found by its qualified name (`demo.shell`): the store's one door starts
/// it or keeps the one already running. The widget is then known by its id like any other, and is
/// drawn where its scene puts it. A command that cannot start is reported once and not tried again.
pub(crate) fn start_declared(state: &mut AppState) {
    for terminal in declared::waiting() {
        let Some((name, program, cwd)) = terminal.to_start() else {
            continue;
        };
        terminal.resolved();
        let owner = crate::app::backend_store::TerminalOwner::Named(name.clone());
        let id = state.backends.id_for(owner.clone());
        let spec = crate::app::backend_store::TerminalSpec {
            program,
            cwd,
            grid: crate::app::backend_factory::FALLBACK_TERMINAL_GRID,
        };
        if let Err(err) = state.start_terminal(owner, spec) {
            crate::notification::Notification::warning(format!(
                "Could not start the terminal {name}: {err}"
            ))
            .dedup_key(format!("terminal.start:{name}"))
            .send();
            continue;
        }
        terminal.attach(id);
        terminal.bind(seams(state.event_proxy.clone(), None, id));
        state.terminals.insert(id, terminal);
    }
}

/// Whether an extension declared a terminal that has not been started yet — what a frame that built
/// a tree asks, to know it needs one more.
pub(crate) fn declared_waiting() -> bool {
    declared::any_waiting()
}

/// The terminal widget a pane shows, if its tree has asked for one yet.
pub(crate) fn of_pane(state: &AppState, pane_id: PaneId) -> Option<&Terminal> {
    let id = state.backends.identity_of(pane_id)?;
    state.terminals.get(&id)
}

/// **Everything a terminal says to its owner**: what its scrollback controls mean, and where its
/// input goes — to the one host handler that holds the policy needing state. A terminal in a pane
/// also focuses that pane when its controls are used; one no pane owns has no pane to focus.
fn seams(
    proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    pane: Option<PaneId>,
    terminal: TerminalId,
) -> input::Seams {
    let input_proxy = proxy.clone();
    let command_proxy = proxy.clone();
    input::Seams {
        scroll: scroll_intents(proxy, pane, terminal),
        input: Box::new(move |input| {
            let _ = input_proxy
                .send_event(crate::app::events::AppEvent::TerminalInput { terminal, input });
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

/// **What a click on a terminal's scrollback controls means**: scroll that terminal — by its id, so
/// it is the one clicked whether or not any pane owns it — after focusing its pane, if it has one.
fn scroll_intents(
    proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    pane: Option<PaneId>,
    terminal: TerminalId,
) -> viewport::ScrollIntents {
    use crate::app::events::AppEvent;
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    let send = move |input: TerminalInput| {
        if let Some(pane_id) = pane {
            let _ = proxy.send_event(AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::FocusPane { pane_id },
            });
        }
        let _ = proxy.send_event(AppEvent::TerminalInput { terminal, input });
    };
    let to_offset = send.clone();
    viewport::ScrollIntents {
        to_bottom: Box::new({
            let send = send.clone();
            move || send(TerminalInput::ScrollToBottom)
        }),
        to_offset: Box::new(move |rows| to_offset(TerminalInput::ScrollTo { rows })),
    }
}

/// Forget the terminals nothing owns any more: a pane that was closed, a named terminal that was
/// killed. A terminal whose process has not started yet is kept — its owner still has its id.
pub(crate) fn retain_owned(state: &mut AppState) {
    let backends = &state.backends;
    state.terminals.retain(|id, _| backends.is_issued(*id));
}

/// **Where each terminal was drawn this frame**, as the flush met them: every scene the frame
/// flushes — columns, floats, chrome, overlays — hands each terminal surface it reaches to
/// [`record`](Self::record), with the rect the scene put it at (scrolled, clipped and placed like
/// anything else). One funnel for every scene, so no scene can be missed.
#[derive(Default)]
pub(crate) struct Drawn(std::collections::HashMap<u64, heca_core::layout::Rectangle>);

impl Drawn {
    /// Note that the flush drew this surface.
    pub(crate) fn record(&mut self, surface: &heca_grid_ui::SurfaceAt) {
        self.0.insert(surface.id, surface.rect);
    }

    /// **Tell every terminal where the frame drew it**, or that it did not: said once, after the
    /// frame's last flush. What a pointer or a key is asked between frames reads this.
    pub(crate) fn settle(self, state: &AppState) {
        settle(state.terminals.values(), &self);
    }
}

fn settle<'a>(terminals: impl Iterator<Item = &'a Terminal>, drawn: &Drawn) {
    for terminal in terminals {
        terminal.place(terminal.id().and_then(|id| drawn.0.get(&id.0).copied()));
    }
}

/// **The box a terminal fills**: how much room the layout gave it (known for every terminal, on
/// screen or not, so one scrolled out of view keeps its size). Its size only — where it is drawn
/// comes from the scene that draws it, at the moment it is drawn.
pub(crate) fn room_of(state: &AppState, id: TerminalId) -> Option<heca_core::layout::Size> {
    state.terminals.get(&id)?.room()
}

/// **Show each terminal how its viewport looks**, from the snapshots this frame prepared. Returns
/// whether any chip or scrollbar changed, so the caller can ask for a frame — they were painted
/// before the snapshot was read, so a change shows one frame late unless one is asked for.
pub(crate) fn show_viewports<'a>(
    state: &AppState,
    snapshots: impl Iterator<
        Item = (
            TerminalId,
            Option<PaneId>,
            &'a heca_core::backend::TerminalSnapshot,
        ),
    >,
) -> bool {
    let appearance = &state.appearance.terminal;
    let mut changed = false;
    for (id, pane, snapshot) in snapshots {
        let Some(terminal) = state.terminals.get(&id) else {
            continue;
        };
        changed |= terminal.show(&Viewport {
            rows: snapshot.rows,
            scrollback_rows: snapshot.scrollback_rows,
            offset: snapshot.viewport_offset,
            scrollbar: appearance.show_scrollbar,
            badge: appearance.show_scrolled_up_badge,
            cell: (snapshot.cell_w, snapshot.cell_h),
            top_stable_row: snapshot.viewport_top_stable_row,
            match_alpha: appearance.search_match_alpha,
            current_match_alpha: appearance.search_current_match_alpha,
            nominal_cell: pane.map_or(state.terminal_cell_size, |pane| {
                state.pane_base_cell_size(pane)
            }),
        });
    }
    changed
}
